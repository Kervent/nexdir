//! Read-only background tasks (storage analysis, duplicate search, file search, index
//! builds). Unlike copy/move jobs they never modify user files, so several may run at
//! once. Every task is cancellable, reports throttled progress, and keeps the (partial)
//! result when it is cancelled.

use serde::Serialize;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const EMIT_INTERVAL: Duration = Duration::from_millis(150);
const HISTORY_LIMIT: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TaskKind {
    StorageScan,
    Duplicates,
    Search,
    IndexBuild,
    NetworkDiscovery,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TaskState {
    Running,
    Completed,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskProgress {
    pub id: u64,
    pub kind: TaskKind,
    pub state: TaskState,
    /// Short description of the current phase ("Scanning", "Hashing 12 of 40 groups", …).
    pub phase: String,
    pub items: u64,
    pub bytes: u64,
    /// Total work for the current phase when known (enables a determinate progress bar).
    pub total: Option<u64>,
    pub done: u64,
    pub current: Option<String>,
    pub error: Option<String>,
    pub started_at: i64,
    pub finished_at: Option<i64>,
}

pub fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Handle given to a running task.
pub struct TaskContext<'a> {
    cancel: &'a AtomicBool,
    progress: TaskProgress,
    emit: &'a dyn Fn(&TaskProgress),
    last_emit: Instant,
}

impl<'a> TaskContext<'a> {
    pub fn new(id: u64, kind: TaskKind, cancel: &'a AtomicBool, emit: &'a dyn Fn(&TaskProgress)) -> Self {
        let progress = TaskProgress {
            id,
            kind,
            state: TaskState::Running,
            phase: "Starting".into(),
            items: 0,
            bytes: 0,
            total: None,
            done: 0,
            current: None,
            error: None,
            started_at: now_millis(),
            finished_at: None,
        };
        Self {
            cancel,
            progress,
            emit,
            last_emit: Instant::now() - EMIT_INTERVAL,
        }
    }

    pub fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    pub fn progress(&mut self) -> &mut TaskProgress {
        &mut self.progress
    }

    pub fn set_phase(&mut self, phase: impl Into<String>, total: Option<u64>) {
        self.progress.phase = phase.into();
        self.progress.total = total;
        self.progress.done = 0;
        self.flush();
    }

    /// Emits progress at most every 150 ms.
    pub fn tick(&mut self) {
        if self.last_emit.elapsed() >= EMIT_INTERVAL {
            self.flush();
        }
    }

    pub fn flush(&mut self) {
        self.last_emit = Instant::now();
        (self.emit)(&self.progress);
    }

    fn finish(mut self, error: Option<String>) -> TaskProgress {
        self.progress.state = match (&error, self.cancelled()) {
            (Some(_), _) => TaskState::Failed,
            (None, true) => TaskState::Cancelled,
            (None, false) => TaskState::Completed,
        };
        self.progress.error = error;
        self.progress.current = None;
        self.progress.finished_at = Some(now_millis());
        self.flush();
        self.progress
    }
}

struct Task {
    id: u64,
    cancel: AtomicBool,
    progress: Mutex<TaskProgress>,
    result: Mutex<Option<serde_json::Value>>,
}

pub type Emitter = Arc<dyn Fn(&TaskProgress) + Send + Sync>;

pub struct TaskManager {
    tasks: Arc<Mutex<VecDeque<Arc<Task>>>>,
    next_id: AtomicU64,
    emit: Emitter,
}

impl TaskManager {
    pub fn new(emit: Emitter) -> Self {
        Self {
            tasks: Arc::new(Mutex::new(VecDeque::new())),
            next_id: AtomicU64::new(1),
            emit,
        }
    }

    /// Starts `run` on its own thread. Its return value (also on cancellation) becomes the
    /// task result; an `Err` marks the task as failed.
    pub fn spawn<R, F>(&self, kind: TaskKind, run: F) -> u64
    where
        R: Serialize,
        F: FnOnce(&mut TaskContext) -> Result<R, String> + Send + 'static,
    {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let cancel = AtomicBool::new(false);
        let initial = TaskContext::new(id, kind, &cancel, &|_| {}).progress;
        let task = Arc::new(Task {
            id,
            cancel,
            progress: Mutex::new(initial),
            result: Mutex::new(None),
        });
        {
            let mut tasks = self.tasks.lock().unwrap();
            tasks.push_back(task.clone());
            while tasks.len() > HISTORY_LIMIT {
                match tasks.iter().position(|t| t.progress.lock().unwrap().state != TaskState::Running) {
                    Some(i) => {
                        tasks.remove(i);
                    }
                    None => break,
                }
            }
        }
        let emit = self.emit.clone();
        std::thread::Builder::new()
            .name(format!("nexdir-task-{id}"))
            .spawn(move || {
                let forward = |p: &TaskProgress| {
                    *task.progress.lock().unwrap() = p.clone();
                    emit(p);
                };
                let mut ctx = TaskContext::new(id, kind, &task.cancel, &forward);
                ctx.flush();
                let outcome = run(&mut ctx);
                let (value, error) = match outcome {
                    Ok(r) => (serde_json::to_value(r).ok(), None),
                    Err(e) => (None, Some(e)),
                };
                // Store the result before announcing completion so the UI can fetch it.
                *task.result.lock().unwrap() = value;
                let final_progress = ctx.finish(error);
                *task.progress.lock().unwrap() = final_progress;
            })
            .expect("failed to start task thread");
        id
    }

    pub fn cancel(&self, id: u64) -> bool {
        let tasks = self.tasks.lock().unwrap();
        tasks
            .iter()
            .find(|t| t.id == id)
            .map(|t| t.cancel.store(true, Ordering::Relaxed))
            .is_some()
    }

    pub fn result(&self, id: u64) -> Option<serde_json::Value> {
        let tasks = self.tasks.lock().unwrap();
        tasks.iter().find(|t| t.id == id).and_then(|t| t.result.lock().unwrap().clone())
    }

    pub fn is_running(&self, kind: TaskKind) -> bool {
        let tasks = self.tasks.lock().unwrap();
        tasks.iter().any(|t| {
            let p = t.progress.lock().unwrap();
            p.kind == kind && p.state == TaskState::Running
        })
    }

    pub fn progress(&self, id: u64) -> Option<TaskProgress> {
        let tasks = self.tasks.lock().unwrap();
        tasks.iter().find(|t| t.id == id).map(|t| t.progress.lock().unwrap().clone())
    }
}

#[cfg(test)]
pub mod testing {
    use super::*;

    /// Runs a task body synchronously for unit tests.
    pub fn run_now<R>(kind: TaskKind, body: impl FnOnce(&mut TaskContext) -> R) -> (R, TaskProgress) {
        let cancel = AtomicBool::new(false);
        let mut ctx = TaskContext::new(1, kind, &cancel, &|_| {});
        let r = body(&mut ctx);
        (r, ctx.finish(None))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wait(m: &TaskManager, id: u64) -> TaskProgress {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let p = m.progress(id).unwrap();
            if p.state != TaskState::Running {
                return p;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn completes_fails_and_cancels() {
        let m = TaskManager::new(Arc::new(|_| {}));
        let ok = m.spawn(TaskKind::Search, |_| Ok::<_, String>(vec![1, 2, 3]));
        assert_eq!(wait(&m, ok).state, TaskState::Completed);
        assert_eq!(m.result(ok), Some(serde_json::json!([1, 2, 3])));

        let bad = m.spawn(TaskKind::Search, |_| Err::<(), _>("boom".to_string()));
        let p = wait(&m, bad);
        assert_eq!(p.state, TaskState::Failed);
        assert_eq!(p.error.as_deref(), Some("boom"));

        let slow = m.spawn(TaskKind::Search, |ctx| {
            let mut n = 0u32;
            while !ctx.cancelled() {
                n += 1;
                std::thread::sleep(Duration::from_millis(1));
            }
            Ok::<_, String>(n)
        });
        std::thread::sleep(Duration::from_millis(20));
        assert!(m.cancel(slow));
        assert_eq!(wait(&m, slow).state, TaskState::Cancelled);
        assert!(m.result(slow).is_some(), "partial result is kept");
    }
}
