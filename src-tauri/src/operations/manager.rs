//! Queue of copy/move jobs. Jobs run one after another on a background worker thread,
//! so the UI never blocks and transfers to the same disk don't compete.

use super::transfer::{Engine, OperationProgress, OperationReport, OperationState, TransferRequest};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};

/// Finished jobs kept for the status UI and reports.
const HISTORY_LIMIT: usize = 50;

pub type Emitter = Arc<dyn Fn(&OperationProgress) + Send + Sync>;

struct Job {
    id: u64,
    request: TransferRequest,
    cancel: AtomicBool,
    progress: Mutex<OperationProgress>,
    report: Mutex<Option<OperationReport>>,
}

struct Shared {
    jobs: Mutex<VecDeque<Arc<Job>>>,
    emit: Emitter,
}

pub struct OperationManager {
    shared: Arc<Shared>,
    queue: Mutex<Sender<Arc<Job>>>,
    next_id: AtomicU64,
}

impl OperationManager {
    pub fn new(emit: Emitter) -> Self {
        let shared = Arc::new(Shared {
            jobs: Mutex::new(VecDeque::new()),
            emit,
        });
        let (tx, rx) = mpsc::channel::<Arc<Job>>();
        let worker_shared = shared.clone();
        std::thread::Builder::new()
            .name("nexdir-operations".into())
            .spawn(move || {
                for job in rx {
                    run_job(&worker_shared, &job);
                }
            })
            .expect("failed to start operations worker");
        Self {
            shared,
            queue: Mutex::new(tx),
            next_id: AtomicU64::new(1),
        }
    }

    pub fn enqueue(&self, request: TransferRequest) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let progress = OperationProgress::queued(id, &request);
        let job = Arc::new(Job {
            id,
            request,
            cancel: AtomicBool::new(false),
            progress: Mutex::new(progress.clone()),
            report: Mutex::new(None),
        });
        {
            let mut jobs = self.shared.jobs.lock().unwrap();
            jobs.push_back(job.clone());
            prune(&mut jobs);
        }
        (self.shared.emit)(&progress);
        let _ = self.queue.lock().unwrap().send(job);
        id
    }

    /// Requests cancellation. Returns false if the job is unknown or already finished.
    pub fn cancel(&self, id: u64) -> bool {
        let jobs = self.shared.jobs.lock().unwrap();
        match jobs.iter().find(|j| j.id == id) {
            Some(job) if job.report.lock().unwrap().is_none() => {
                job.cancel.store(true, Ordering::Relaxed);
                true
            }
            _ => false,
        }
    }

    pub fn list(&self) -> Vec<OperationProgress> {
        self.shared
            .jobs
            .lock()
            .unwrap()
            .iter()
            .map(|j| j.progress.lock().unwrap().clone())
            .collect()
    }

    pub fn report(&self, id: u64) -> Option<OperationReport> {
        let jobs = self.shared.jobs.lock().unwrap();
        jobs.iter().find(|j| j.id == id).and_then(|j| j.report.lock().unwrap().clone())
    }
}

fn prune(jobs: &mut VecDeque<Arc<Job>>) {
    while jobs.len() > HISTORY_LIMIT {
        match jobs.iter().position(|j| j.report.lock().unwrap().is_some()) {
            Some(i) => {
                jobs.remove(i);
            }
            None => break,
        }
    }
}

fn run_job(shared: &Shared, job: &Job) {
    if job.cancel.load(Ordering::Relaxed) {
        let mut p = job.progress.lock().unwrap();
        p.state = OperationState::Cancelled;
        (shared.emit)(&p);
        *job.report.lock().unwrap() = Some(OperationReport::default());
        return;
    }
    let mut on_update = |p: &OperationProgress| {
        *job.progress.lock().unwrap() = p.clone();
        (shared.emit)(p);
    };
    let engine = Engine::new(job.id, &job.request, &job.cancel, &mut on_update);
    let (progress, report) = engine.run();
    *job.progress.lock().unwrap() = progress;
    *job.report.lock().unwrap() = Some(report);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operations::transfer::{ConflictPolicy, TransferKind};
    use std::time::{Duration, Instant};

    fn wait_for(manager: &OperationManager, id: u64) -> OperationReport {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            if let Some(r) = manager.report(id) {
                return r;
            }
            assert!(Instant::now() < deadline, "job did not finish");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn runs_queued_jobs_in_order_and_emits_progress() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("a.txt");
        std::fs::write(&src, "a").unwrap();
        let (d1, d2) = (tmp.path().join("d1"), tmp.path().join("d2"));
        std::fs::create_dir(&d1).unwrap();
        std::fs::create_dir(&d2).unwrap();

        let events = Arc::new(Mutex::new(Vec::<OperationState>::new()));
        let sink = events.clone();
        let manager = OperationManager::new(Arc::new(move |p| sink.lock().unwrap().push(p.state)));
        let req = |dest: &std::path::Path| TransferRequest {
            kind: TransferKind::Copy,
            sources: vec![src.clone()],
            destination: dest.to_path_buf(),
            policy: ConflictPolicy::Skip,
        };
        let a = manager.enqueue(req(&d1));
        let b = manager.enqueue(req(&d2));
        assert!(wait_for(&manager, a).failures.is_empty());
        assert!(wait_for(&manager, b).failures.is_empty());
        assert!(d1.join("a.txt").exists() && d2.join("a.txt").exists());
        assert!(manager.list().iter().all(|p| p.state == OperationState::Completed));
        assert!(!manager.cancel(a), "finished jobs cannot be cancelled");
        let events = events.lock().unwrap();
        assert!(events.contains(&OperationState::Queued) && events.contains(&OperationState::Completed));
    }
}
