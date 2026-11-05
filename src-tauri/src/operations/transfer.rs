//! Copy and move engine.
//!
//! Safety rules:
//! * Existing destination items are only replaced when the user chose `Replace`.
//!   Folders are then merged; nothing in the destination that is not part of the
//!   transfer is ever removed.
//! * Links and junctions are never followed while recursing. On the same volume a move
//!   renames the link itself; otherwise links are skipped and reported.
//! * Files are copied through `safe_copy`: into a temporary file first, verified (size and
//!   BLAKE3 hash), then renamed into place or swapped in with `ReplaceFileW` and a backup. An
//!   existing destination file survives cancellation and every failure.
//! * A move deletes a source file only after its verified copy is in place, and removes
//!   source folders only when they ended up empty.
//! * Every processed, skipped and failed item is recorded, also on cancellation.

use super::safe_copy;
use crate::error::AppError;
use crate::filesystem::EntryKind;
use crate::security;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const ERROR_NOT_SAME_DEVICE: i32 = 17;
/// Moves that would copy and then delete the original are not done on network shares yet:
/// there is no Recycle Bin there, and the client cannot prove that the server has stored the
/// copy durably. A rename within the same share is still allowed (nothing is deleted).
pub const NETWORK_MOVE_REFUSED: &str = "Moving between a network share and another location (or merging into an existing folder on a share) is not available yet. Nothing was changed. Copy the item instead and delete the original yourself once you have checked the copy.";
const EMIT_INTERVAL: Duration = Duration::from_millis(120);
/// Upper bound for per-item lists in a report, to keep memory bounded on huge trees.
const REPORT_LIMIT: usize = 20_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TransferKind {
    Copy,
    Move,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConflictPolicy {
    /// Leave existing destination items untouched and skip the source item.
    Skip,
    /// Give the transferred item a free name such as "report (2).pdf".
    KeepBoth,
    /// Replace existing files and merge into existing folders.
    Replace,
}

#[derive(Debug, Clone)]
pub struct TransferRequest {
    pub kind: TransferKind,
    pub sources: Vec<PathBuf>,
    pub destination: PathBuf,
    pub policy: ConflictPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum OperationState {
    Queued,
    Scanning,
    Running,
    Completed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationProgress {
    pub id: u64,
    pub kind: TransferKind,
    pub state: OperationState,
    pub destination: String,
    pub source_count: usize,
    pub total_bytes: u64,
    pub done_bytes: u64,
    pub total_files: u64,
    pub done_files: u64,
    pub current: Option<String>,
    pub failure_count: usize,
    pub skipped_count: usize,
    pub started_at: i64,
    pub finished_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemNote {
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationReport {
    /// Source files that were fully transferred.
    pub completed: Vec<String>,
    pub failures: Vec<ItemNote>,
    pub skipped: Vec<ItemNote>,
    /// True when one of the lists hit the size limit.
    pub truncated: bool,
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

impl OperationProgress {
    pub fn queued(id: u64, req: &TransferRequest) -> Self {
        Self {
            id,
            kind: req.kind,
            state: OperationState::Queued,
            destination: req.destination.display().to_string(),
            source_count: req.sources.len(),
            total_bytes: 0,
            done_bytes: 0,
            total_files: 0,
            done_files: 0,
            current: None,
            failure_count: 0,
            skipped_count: 0,
            started_at: now_ms(),
            finished_at: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictCheck {
    /// Names that already exist in the destination.
    pub conflicts: Vec<String>,
    /// True when every source already lives in the destination folder.
    pub same_folder: bool,
}

pub fn find_conflicts(sources: &[PathBuf], destination: &Path) -> ConflictCheck {
    let conflicts = sources
        .iter()
        .filter_map(|s| s.file_name())
        .filter(|name| fs::symlink_metadata(destination.join(name)).is_ok())
        .map(|n| n.to_string_lossy().into_owned())
        .collect();
    let same_folder = !sources.is_empty() && sources.iter().all(|s| s.parent().is_some_and(|p| paths_equal(p, destination)));
    ConflictCheck { conflicts, same_folder }
}

fn paths_equal(a: &Path, b: &Path) -> bool {
    a.as_os_str().to_string_lossy().to_lowercase() == b.as_os_str().to_string_lossy().to_lowercase()
}

fn link_kind(path: &Path, meta: &fs::Metadata) -> EntryKind {
    crate::filesystem::entry_from_metadata(path, meta).kind
}

/// Returns a free name in `dir` based on `name`: "a.txt" â†’ "a (2).txt", "dir" â†’ "dir (2)".
pub fn unique_name(dir: &Path, name: &str, is_dir: bool) -> PathBuf {
    let (stem, ext) = match (is_dir, name.rfind('.')) {
        (false, Some(i)) if i > 0 => (&name[..i], &name[i..]),
        _ => (name, ""),
    };
    (2u32..)
        .map(|n| dir.join(format!("{stem} ({n}){ext}")))
        .find(|candidate| fs::symlink_metadata(candidate).is_err())
        .expect("unbounded search finds a free name")
}

/// Sums sizes and file counts below `path` without following links.
fn measure(path: &Path, cancel: &AtomicBool) -> (u64, u64) {
    let Ok(meta) = fs::symlink_metadata(path) else { return (0, 0) };
    if meta.file_type().is_symlink() {
        return (0, 0);
    }
    if !meta.is_dir() {
        return (meta.len(), 1);
    }
    let (mut bytes, mut files) = (0, 0);
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let Ok(reader) = fs::read_dir(&dir) else { continue };
        for entry in reader.flatten() {
            let Ok(m) = entry.metadata() else { continue };
            if m.file_type().is_symlink() {
                continue;
            } else if m.is_dir() {
                stack.push(entry.path());
            } else {
                bytes += m.len();
                files += 1;
            }
        }
    }
    (bytes, files)
}

pub struct Engine<'a> {
    req: &'a TransferRequest,
    /// Test hooks for file copies and moves; `NoHooks` in the app.
    hooks: &'a dyn safe_copy::Hooks,
    cancel: &'a AtomicBool,
    on_update: &'a mut dyn FnMut(&OperationProgress),
    pub progress: OperationProgress,
    pub report: OperationReport,
    last_emit: Instant,
    emit_interval: Duration,
}

/// Runs a transfer synchronously (tests in other modules, e.g. the real SMB tests).
#[cfg(test)]
pub(crate) fn run_for_test(req: &TransferRequest, cancel: &AtomicBool, on_update: &mut dyn FnMut(&OperationProgress)) -> OperationReport {
    Engine::new(1, req, cancel, on_update).run().1
}

impl<'a> Engine<'a> {
    pub fn new(id: u64, req: &'a TransferRequest, cancel: &'a AtomicBool, on_update: &'a mut dyn FnMut(&OperationProgress)) -> Self {
        Self {
            progress: OperationProgress::queued(id, req),
            report: OperationReport::default(),
            req,
            hooks: &safe_copy::NoHooks,
            cancel,
            on_update,
            last_emit: Instant::now(),
            emit_interval: EMIT_INTERVAL,
        }
    }

    fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    fn emit(&mut self, force: bool) {
        if force || self.last_emit.elapsed() >= self.emit_interval {
            self.last_emit = Instant::now();
            (self.on_update)(&self.progress);
        }
    }

    fn push(list: &mut Vec<ItemNote>, truncated: &mut bool, path: &Path, message: impl Into<String>) {
        if list.len() < REPORT_LIMIT {
            list.push(ItemNote {
                path: path.display().to_string(),
                message: message.into(),
            });
        } else {
            *truncated = true;
        }
    }

    fn fail(&mut self, path: &Path, message: impl Into<String>) {
        self.progress.failure_count += 1;
        Self::push(&mut self.report.failures, &mut self.report.truncated, path, message);
    }

    fn skip(&mut self, path: &Path, message: impl Into<String>) {
        self.progress.skipped_count += 1;
        Self::push(&mut self.report.skipped, &mut self.report.truncated, path, message);
    }

    fn complete_file(&mut self, path: &Path) {
        self.progress.done_files += 1;
        self.record_completed(path);
    }

    fn record_completed(&mut self, path: &Path) {
        if self.report.completed.len() < REPORT_LIMIT {
            self.report.completed.push(path.display().to_string());
        } else {
            self.report.truncated = true;
        }
    }

    /// Runs the whole transfer. Returns when finished or cancelled.
    pub fn run(mut self) -> (OperationProgress, OperationReport) {
        self.progress.state = OperationState::Scanning;
        self.emit(true);
        for src in &self.req.sources {
            let (bytes, files) = measure(src, self.cancel);
            self.progress.total_bytes += bytes;
            self.progress.total_files += files;
        }

        if !self.cancelled() {
            self.progress.state = OperationState::Running;
            self.emit(true);
            let dest_canonical = fs::canonicalize(&self.req.destination);
            match dest_canonical {
                Err(e) => {
                    let err = AppError::from_io(&e, &self.req.destination);
                    let dest = self.req.destination.clone();
                    self.fail(&dest, err.message);
                }
                Ok(dest_canonical) => {
                    for src in self.req.sources.clone() {
                        if self.cancelled() {
                            break;
                        }
                        self.transfer_top_level(&src, &dest_canonical);
                    }
                }
            }
        }

        self.progress.state = if self.cancelled() {
            OperationState::Cancelled
        } else {
            OperationState::Completed
        };
        self.progress.current = None;
        self.progress.finished_at = Some(now_ms());
        self.emit(true);
        (self.progress, self.report)
    }

    fn transfer_top_level(&mut self, src: &Path, dest_canonical: &Path) {
        let meta = match fs::symlink_metadata(src) {
            Ok(m) => m,
            Err(e) => return self.fail(src, AppError::from_io(&e, src).message),
        };
        let Some(name) = src.file_name().map(|n| n.to_string_lossy().into_owned()) else {
            return self.fail(src, "Drives and root folders cannot be copied or moved as a whole.");
        };
        let kind = link_kind(src, &meta);

        if kind == EntryKind::Dir {
            if let Ok(src_canonical) = fs::canonicalize(src) {
                if security::is_same_or_inside(dest_canonical, &src_canonical) {
                    return self.fail(src, "A folder cannot be copied or moved into itself.");
                }
            }
        }
        let destination = self.req.destination.clone();
        if self.req.kind == TransferKind::Move && src.parent().is_some_and(|p| paths_equal(p, &destination)) {
            return self.skip(src, "The item is already in this folder.");
        }

        let mut target = destination.join(&name);
        if fs::symlink_metadata(&target).is_ok() {
            match self.req.policy {
                ConflictPolicy::Skip => return self.skip(src, "An item with this name already exists in the destination."),
                ConflictPolicy::KeepBoth => target = unique_name(&destination, &name, meta.is_dir()),
                ConflictPolicy::Replace => {}
            }
        }
        self.transfer(src, &target, true);
    }

    fn involves_network(&self, src: &Path, target: &Path) -> bool {
        self.hooks.is_network(src) || self.hooks.is_network(target)
    }

    fn transfer(&mut self, src: &Path, target: &Path, top_level: bool) {
        if self.cancelled() {
            return;
        }
        let meta = match fs::symlink_metadata(src) {
            Ok(m) => m,
            Err(e) => return self.fail(src, AppError::from_io(&e, src).message),
        };
        let kind = link_kind(src, &meta);
        let target_meta = fs::symlink_metadata(target).ok();

        // Same-volume move: a rename moves the item (or the link itself) atomically.
        if self.req.kind == TransferKind::Move && target_meta.is_none() {
            let (bytes, files) = measure(src, self.cancel);
            match self.hooks.rename_item(src, target) {
                Ok(()) => {
                    self.progress.done_bytes += bytes;
                    self.progress.done_files += files;
                    self.record_completed(src);
                    self.emit(false);
                    return;
                }
                Err(e) if e.raw_os_error() == Some(ERROR_NOT_SAME_DEVICE) => {
                    // Fall back to copy + remove, except across network shares.
                    if self.involves_network(src, target) {
                        return self.fail(src, NETWORK_MOVE_REFUSED);
                    }
                }
                Err(e) => return self.fail(src, AppError::from_io(&e, src).message),
            }
        }
        // Merging into an existing destination copies and removes item by item.
        if self.req.kind == TransferKind::Move && top_level && target_meta.is_some() && self.involves_network(src, target) {
            return self.fail(src, NETWORK_MOVE_REFUSED);
        }

        match kind {
            EntryKind::Dir => self.transfer_dir(src, target, target_meta),
            EntryKind::File => self.transfer_file(src, target, target_meta, meta.len(), false),
            // Copying a selected file link copies the file it points to, like Explorer.
            EntryKind::FileLink if top_level && self.req.kind == TransferKind::Copy => {
                let len = fs::metadata(src).map(|m| m.len()).unwrap_or(0);
                self.transfer_file(src, target, target_meta, len, true)
            }
            _ => self.skip(src, "Links and junctions are not followed. This item was left unchanged."),
        }
    }

    fn transfer_dir(&mut self, src: &Path, target: &Path, target_meta: Option<fs::Metadata>) {
        match target_meta {
            Some(m) if m.is_dir() && !m.file_type().is_symlink() => {
                if self.req.policy != ConflictPolicy::Replace {
                    return self.skip(src, "A folder with this name already exists in the destination.");
                }
                // Replace policy: merge into the existing folder.
            }
            Some(_) => return self.fail(src, "A file or link with this name exists in the destination."),
            None => {
                if let Err(e) = fs::create_dir(target) {
                    return self.fail(target, AppError::from_io(&e, target).message);
                }
            }
        }
        let reader = match fs::read_dir(src) {
            Ok(r) => r,
            Err(e) => return self.fail(src, AppError::from_io(&e, src).message),
        };
        for entry in reader {
            if self.cancelled() {
                return;
            }
            match entry {
                Ok(entry) => self.transfer(&entry.path(), &target.join(entry.file_name()), false),
                Err(e) => self.fail(src, AppError::from_io(&e, src).message),
            }
        }
        if self.req.kind == TransferKind::Move && !self.cancelled() {
            // Only succeeds when empty: anything skipped or failed stays in place.
            let _ = fs::remove_dir(src);
        }
    }

    fn transfer_file(&mut self, src: &Path, target: &Path, target_meta: Option<fs::Metadata>, len: u64, follow_link: bool) {
        let mode = match target_meta {
            None => safe_copy::Mode::CreateNew,
            Some(m) if m.file_type().is_symlink() => {
                return self.fail(src, "A link with this name exists in the destination. It was left unchanged.")
            }
            Some(m) if m.is_dir() => return self.fail(src, "A folder with this name exists in the destination."),
            Some(_) if self.req.policy == ConflictPolicy::Replace => safe_copy::Mode::Replace,
            Some(_) => return self.skip(src, "A file with this name already exists in the destination."),
        };
        self.progress.current = Some(src.display().to_string());
        self.emit(false);

        // Staged copy: an existing destination survives cancellation and every failure.
        let base = self.progress.done_bytes;
        let cancel = self.cancel;
        let hooks = self.hooks;
        let is_move = self.req.kind == TransferKind::Move;
        // Defence in depth: never delete a source after copying across a network share.
        if is_move && self.involves_network(src, target) {
            return self.fail(src, NETWORK_MOVE_REFUSED);
        }
        let opts = safe_copy::Options {
            mode,
            // Flush to disk when data disappears afterwards (the source of a move; the replaced
            // version is always flushed by safe_copy) and on network shares, so the server is
            // asked to commit the data before the copy counts as finished.
            durable: is_move || self.hooks.is_network(target),
            // The source is deleted by safe_copy through the handle it was copied from.
            delete_source: is_move,
            follow_source_link: follow_link,
            // On shares the SMB client would otherwise accept the data into its cache at once and
            // send it during the final flush, where progress and Cancel cannot follow it.
            write_through: self.hooks.is_network(target),
        };
        let result = {
            let progress = &mut self.progress;
            let on_update = &mut *self.on_update;
            let last_emit = &mut self.last_emit;
            let interval = self.emit_interval;
            safe_copy::copy(hooks, src, target, opts, cancel, &mut |done| {
                progress.done_bytes = base + done.min(len);
                if last_emit.elapsed() >= interval {
                    *last_emit = Instant::now();
                    on_update(progress);
                }
            })
        };
        match result {
            Ok(copied) => {
                self.progress.done_bytes = base + len;
                if let Some(backup) = copied.leftover {
                    self.fail(
                        target,
                        format!(
                            "Replaced. The previous version could not be removed and is kept as {}.",
                            backup.display()
                        ),
                    );
                }
                if let Some(temp) = copied.temp_left {
                    self.fail(target, format!("A temporary file could not be removed: {}", temp.display()));
                }
                // Move: safe_copy deleted the source only after the verified copy was in place.
                if let Some(reason) = copied.source_kept {
                    self.fail(src, format!("Copied, but the original was kept: {reason}"));
                    self.emit(false);
                    return;
                }
                self.complete_file(src);
            }
            Err(safe_copy::CopyError::Cancelled) => self.progress.done_bytes = base,
            Err(safe_copy::CopyError::Failed(message)) => {
                self.progress.done_bytes = base;
                self.fail(src, message);
            }
            Err(safe_copy::CopyError::BackupKept { message, backup }) => {
                // The previous version only exists under the backup name now: report that file.
                self.progress.done_bytes = base;
                self.fail(
                    &backup,
                    format!("{message} The previous version of {} is kept under this name.", target.display()),
                );
            }
        }
        self.emit(false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn run(kind: TransferKind, sources: Vec<PathBuf>, destination: &Path, policy: ConflictPolicy) -> (OperationProgress, OperationReport) {
        let req = TransferRequest {
            kind,
            sources,
            destination: destination.to_path_buf(),
            policy,
        };
        let cancel = AtomicBool::new(false);
        let mut updates = 0;
        let mut on_update = |_: &OperationProgress| updates += 1;
        let result = Engine::new(1, &req, &cancel, &mut on_update).run();
        assert!(updates >= 2);
        result
    }

    fn tree(root: &Path) {
        fs::create_dir_all(root.join("docs/deep/deeper")).unwrap();
        fs::write(root.join("docs/a.txt"), "alpha").unwrap();
        fs::write(root.join("docs/deep/b.txt"), "bravo!").unwrap();
        fs::write(root.join("docs/deep/deeper/Ã¼ æ—¥æœ¬.txt"), "unicode").unwrap();
    }

    #[test]
    fn copies_a_tree() {
        let tmp = tempfile::tempdir().unwrap();
        let (src, dst) = (tmp.path().join("src"), tmp.path().join("dst"));
        tree(&src);
        fs::create_dir(&dst).unwrap();
        let (p, r) = run(TransferKind::Copy, vec![src.join("docs")], &dst, ConflictPolicy::Skip);
        assert_eq!(p.state, OperationState::Completed);
        assert_eq!((p.total_files, p.done_files), (3, 3));
        assert_eq!(p.total_bytes, 18);
        assert_eq!(p.done_bytes, 18);
        assert!(r.failures.is_empty(), "{:?}", r.failures);
        assert_eq!(fs::read_to_string(dst.join("docs/deep/deeper/Ã¼ æ—¥æœ¬.txt")).unwrap(), "unicode");
        assert!(src.join("docs/a.txt").exists(), "copy keeps the source");
    }

    #[test]
    fn skip_policy_never_touches_existing_files() {
        let tmp = tempfile::tempdir().unwrap();
        let (src, dst) = (tmp.path().join("src"), tmp.path().join("dst"));
        fs::create_dir_all(&src).unwrap();
        fs::create_dir_all(&dst).unwrap();
        fs::write(src.join("same.txt"), "new").unwrap();
        fs::write(dst.join("same.txt"), "old").unwrap();
        let (p, r) = run(TransferKind::Copy, vec![src.join("same.txt")], &dst, ConflictPolicy::Skip);
        assert_eq!(p.skipped_count, 1);
        assert_eq!(r.skipped.len(), 1);
        assert_eq!(fs::read_to_string(dst.join("same.txt")).unwrap(), "old");
    }

    #[test]
    fn keep_both_creates_numbered_copies() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        fs::write(dir.join("report.pdf"), "1").unwrap();
        fs::create_dir(dir.join("folder")).unwrap();
        let (_, r) = run(
            TransferKind::Copy,
            vec![dir.join("report.pdf"), dir.join("folder")],
            dir,
            ConflictPolicy::KeepBoth,
        );
        assert!(r.failures.is_empty(), "{:?}", r.failures);
        assert_eq!(fs::read_to_string(dir.join("report (2).pdf")).unwrap(), "1");
        assert!(dir.join("folder (2)").is_dir());
        let (_, _) = run(TransferKind::Copy, vec![dir.join("report.pdf")], dir, ConflictPolicy::KeepBoth);
        assert!(dir.join("report (3).pdf").exists());
    }

    #[test]
    fn replace_merges_folders_without_deleting_extra_destination_files() {
        let tmp = tempfile::tempdir().unwrap();
        let (src, dst) = (tmp.path().join("src"), tmp.path().join("dst"));
        tree(&src);
        fs::create_dir_all(dst.join("docs")).unwrap();
        fs::write(dst.join("docs/a.txt"), "old alpha").unwrap();
        fs::write(dst.join("docs/only-in-dest.txt"), "keep me").unwrap();
        let (_, r) = run(TransferKind::Copy, vec![src.join("docs")], &dst, ConflictPolicy::Replace);
        assert!(r.failures.is_empty(), "{:?}", r.failures);
        assert_eq!(fs::read_to_string(dst.join("docs/a.txt")).unwrap(), "alpha");
        assert_eq!(fs::read_to_string(dst.join("docs/only-in-dest.txt")).unwrap(), "keep me");
        assert!(dst.join("docs/deep/b.txt").exists());
    }

    #[test]
    fn refuses_to_copy_a_folder_into_itself() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("src");
        tree(&src);
        let inner = src.join("docs/deep");
        let (p, r) = run(TransferKind::Copy, vec![src.join("docs")], &inner, ConflictPolicy::KeepBoth);
        assert_eq!(p.failure_count, 1);
        assert!(r.failures[0].message.contains("into itself"));
        assert!(!inner.join("docs").exists());
    }

    #[test]
    fn moves_within_a_volume() {
        let tmp = tempfile::tempdir().unwrap();
        let (src, dst) = (tmp.path().join("src"), tmp.path().join("dst"));
        tree(&src);
        fs::create_dir(&dst).unwrap();
        let (p, r) = run(TransferKind::Move, vec![src.join("docs")], &dst, ConflictPolicy::Skip);
        assert!(r.failures.is_empty(), "{:?}", r.failures);
        assert_eq!(p.done_files, 3);
        assert!(!src.join("docs").exists());
        assert_eq!(fs::read_to_string(dst.join("docs/deep/b.txt")).unwrap(), "bravo!");
    }

    #[test]
    fn move_with_replace_merges_and_keeps_skipped_sources() {
        let tmp = tempfile::tempdir().unwrap();
        let (src, dst) = (tmp.path().join("src"), tmp.path().join("dst"));
        tree(&src);
        fs::create_dir_all(dst.join("docs")).unwrap();
        fs::write(dst.join("docs/a.txt"), "old").unwrap();
        let (_, r) = run(TransferKind::Move, vec![src.join("docs")], &dst, ConflictPolicy::Replace);
        assert!(r.failures.is_empty(), "{:?}", r.failures);
        assert_eq!(fs::read_to_string(dst.join("docs/a.txt")).unwrap(), "alpha");
        assert!(!src.join("docs").exists(), "fully merged source folder is removed when empty");
    }

    #[test]
    fn move_into_same_folder_is_a_noop() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("x.txt"), "x").unwrap();
        let (p, _) = run(
            TransferKind::Move,
            vec![tmp.path().join("x.txt")],
            tmp.path(),
            ConflictPolicy::Replace,
        );
        assert_eq!(p.skipped_count, 1);
        assert_eq!(fs::read_to_string(tmp.path().join("x.txt")).unwrap(), "x");
    }

    #[test]
    fn junctions_inside_copied_folders_are_not_followed() {
        let tmp = tempfile::tempdir().unwrap();
        let outside = tmp.path().join("outside");
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("private.txt"), "must not be copied").unwrap();
        let src = tmp.path().join("src");
        fs::create_dir(&src).unwrap();
        fs::write(src.join("ok.txt"), "ok").unwrap();
        let out = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(src.join("link"))
            .arg(&outside)
            .output()
            .unwrap();
        assert!(out.status.success());
        let dst = tmp.path().join("dst");
        fs::create_dir(&dst).unwrap();

        let (p, r) = run(TransferKind::Copy, vec![src.clone()], &dst, ConflictPolicy::Skip);
        assert_eq!(p.skipped_count, 1, "{:?}", r.skipped);
        assert!(dst.join("src/ok.txt").exists());
        assert!(!dst.join("src/link").exists());
        assert!(outside.join("private.txt").exists());
    }

    #[test]
    fn cancellation_is_reported_and_keeps_completed_list() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("src");
        fs::create_dir(&src).unwrap();
        for i in 0..50 {
            fs::write(src.join(format!("f{i}.bin")), vec![0u8; 1024]).unwrap();
        }
        let dst = tmp.path().join("dst");
        fs::create_dir(&dst).unwrap();
        let req = TransferRequest {
            kind: TransferKind::Copy,
            sources: vec![src],
            destination: dst,
            policy: ConflictPolicy::Skip,
        };
        let cancel = AtomicBool::new(false);
        let mut seen = 0;
        let cancel_ref = &cancel;
        let mut on_update = |p: &OperationProgress| {
            seen += 1;
            if p.done_files >= 5 {
                cancel_ref.store(true, Ordering::Relaxed);
            }
        };
        let mut engine = Engine::new(7, &req, &cancel, &mut on_update);
        engine.emit_interval = Duration::ZERO;
        let (p, r) = engine.run();
        assert_eq!(p.state, OperationState::Cancelled);
        assert!(p.done_files < 50);
        assert_eq!(r.completed.len() as u64, p.done_files);
    }

    /// Every final swap into the destination fails (as if the file were in use).
    struct RefuseReplace;
    impl safe_copy::Hooks for RefuseReplace {
        fn at(&self, stage: safe_copy::Stage) -> std::io::Result<()> {
            match stage {
                safe_copy::Stage::BeforeBackup(_) => Err(std::io::Error::from_raw_os_error(32)),
                _ => Ok(()),
            }
        }
    }

    #[test]
    fn move_with_replace_keeps_the_source_when_the_replace_fails() {
        let tmp = tempfile::tempdir().unwrap();
        let (src, dst) = (tmp.path().join("src"), tmp.path().join("dst"));
        fs::create_dir_all(&src).unwrap();
        fs::create_dir_all(&dst).unwrap();
        fs::write(src.join("a.txt"), "new").unwrap();
        fs::write(dst.join("a.txt"), "old").unwrap();
        let req = TransferRequest {
            kind: TransferKind::Move,
            sources: vec![src.join("a.txt")],
            destination: dst.clone(),
            policy: ConflictPolicy::Replace,
        };
        let cancel = AtomicBool::new(false);
        let mut on_update = |_: &OperationProgress| {};
        let mut engine = Engine::new(1, &req, &cancel, &mut on_update);
        engine.hooks = &RefuseReplace;
        let (p, r) = engine.run();
        assert_eq!(p.failure_count, 1, "{:?}", r.failures);
        assert_eq!(fs::read_to_string(src.join("a.txt")).unwrap(), "new", "the source must be kept");
        assert_eq!(
            fs::read_to_string(dst.join("a.txt")).unwrap(),
            "old",
            "the destination must be unchanged"
        );
        assert_eq!(fs::read_dir(&dst).unwrap().count(), 1, "no temporary files may remain");
    }

    #[test]
    fn move_with_replace_onto_the_same_file_deletes_nothing() {
        // A hard link is a second name for the same file (like an 8.3 short name).
        let tmp = tempfile::tempdir().unwrap();
        let (src, dst) = (tmp.path().join("src"), tmp.path().join("dst"));
        fs::create_dir_all(&src).unwrap();
        fs::create_dir_all(&dst).unwrap();
        fs::write(src.join("a.txt"), "only copy").unwrap();
        fs::hard_link(src.join("a.txt"), dst.join("a.txt")).unwrap();
        let (p, r) = run(TransferKind::Move, vec![src.join("a.txt")], &dst, ConflictPolicy::Replace);
        assert_eq!(p.failure_count, 1, "{:?}", r.failures);
        assert!(r.failures[0].message.contains("same file"));
        assert_eq!(fs::read_to_string(src.join("a.txt")).unwrap(), "only copy");
        assert_eq!(fs::read_to_string(dst.join("a.txt")).unwrap(), "only copy");
    }

    #[test]
    fn copy_with_replace_onto_a_destination_link_leaves_it_unchanged() {
        let tmp = tempfile::tempdir().unwrap();
        let outside = tmp.path().join("outside.txt");
        fs::write(&outside, "outside").unwrap();
        let (src, dst) = (tmp.path().join("src"), tmp.path().join("dst"));
        fs::create_dir_all(&src).unwrap();
        fs::create_dir_all(&dst).unwrap();
        fs::write(src.join("a.txt"), "new").unwrap();
        if std::os::windows::fs::symlink_file(&outside, dst.join("a.txt")).is_err() {
            eprintln!("skipped: creating symbolic links needs Developer Mode or admin rights");
            return;
        }
        let (p, _) = run(TransferKind::Copy, vec![src.join("a.txt")], &dst, ConflictPolicy::Replace);
        assert_eq!(p.failure_count, 1);
        assert_eq!(fs::read_to_string(&outside).unwrap(), "outside", "must not write through the link");
    }

    /// Behaves as if source and destination were on different drives: renaming a whole item
    /// fails with ERROR_NOT_SAME_DEVICE, so the engine has to copy and then delete. Optionally
    /// every swap into an existing destination fails as well.
    struct OtherDrive {
        refuse_replace: bool,
    }
    impl safe_copy::Hooks for OtherDrive {
        fn at(&self, stage: safe_copy::Stage) -> std::io::Result<()> {
            match stage {
                safe_copy::Stage::BeforeBackup(_) if self.refuse_replace => Err(std::io::Error::from_raw_os_error(32)),
                _ => Ok(()),
            }
        }
        fn rename_item(&self, _: &Path, _: &Path) -> std::io::Result<()> {
            Err(std::io::Error::from_raw_os_error(ERROR_NOT_SAME_DEVICE))
        }
    }

    fn run_with(
        steps: &dyn safe_copy::Hooks,
        kind: TransferKind,
        sources: Vec<PathBuf>,
        destination: &Path,
        policy: ConflictPolicy,
    ) -> (OperationProgress, OperationReport) {
        let req = TransferRequest {
            kind,
            sources,
            destination: destination.to_path_buf(),
            policy,
        };
        let cancel = AtomicBool::new(false);
        let mut on_update = |_: &OperationProgress| {};
        let mut engine = Engine::new(1, &req, &cancel, &mut on_update);
        engine.hooks = steps;
        engine.run()
    }

    #[test]
    fn cross_drive_move_copies_verifies_then_deletes() {
        let tmp = tempfile::tempdir().unwrap();
        let (src, dst) = (tmp.path().join("src"), tmp.path().join("dst"));
        tree(&src);
        fs::create_dir_all(dst.join("docs")).unwrap();
        fs::write(dst.join("docs/a.txt"), "old alpha").unwrap();
        let (p, r) = run_with(
            &OtherDrive { refuse_replace: false },
            TransferKind::Move,
            vec![src.join("docs")],
            &dst,
            ConflictPolicy::Replace,
        );
        assert!(r.failures.is_empty(), "{:?}", r.failures);
        assert_eq!(p.done_files, 3);
        assert_eq!(fs::read_to_string(dst.join("docs/a.txt")).unwrap(), "alpha");
        assert_eq!(fs::read_to_string(dst.join("docs/deep/b.txt")).unwrap(), "bravo!");
        assert!(!src.join("docs").exists(), "fully moved source folder is removed");
    }

    #[test]
    fn cross_drive_move_keeps_sources_whose_copy_was_not_put_in_place() {
        let tmp = tempfile::tempdir().unwrap();
        let (src, dst) = (tmp.path().join("src"), tmp.path().join("dst"));
        tree(&src);
        fs::create_dir_all(dst.join("docs")).unwrap();
        fs::write(dst.join("docs/a.txt"), "old alpha").unwrap();
        let (p, r) = run_with(
            &OtherDrive { refuse_replace: true },
            TransferKind::Move,
            vec![src.join("docs")],
            &dst,
            ConflictPolicy::Replace,
        );
        assert_eq!(p.failure_count, 1, "{:?}", r.failures);
        assert_eq!(fs::read_to_string(src.join("docs/a.txt")).unwrap(), "alpha", "source kept");
        assert_eq!(
            fs::read_to_string(dst.join("docs/a.txt")).unwrap(),
            "old alpha",
            "destination unchanged"
        );
        assert!(!src.join("docs/deep/b.txt").exists(), "files without a conflict were moved");
        assert!(src.join("docs").is_dir(), "the source folder stays because it is not empty");
    }

    #[test]
    fn copy_with_replace_onto_a_destination_junction_leaves_it_unchanged() {
        // Junctions need no special rights, so this always runs (unlike the symlink test).
        let tmp = tempfile::tempdir().unwrap();
        let outside = tmp.path().join("outside");
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("keep.txt"), "outside").unwrap();
        let (src, dst) = (tmp.path().join("src"), tmp.path().join("dst"));
        fs::create_dir_all(&src).unwrap();
        fs::create_dir_all(&dst).unwrap();
        fs::write(src.join("a.txt"), "new").unwrap();
        let out = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(dst.join("a.txt"))
            .arg(&outside)
            .output()
            .unwrap();
        assert!(out.status.success());
        for policy in [ConflictPolicy::Replace, ConflictPolicy::Skip] {
            let (p, r) = run(TransferKind::Move, vec![src.join("a.txt")], &dst, policy);
            assert_eq!(p.done_files, 0, "{policy:?}: {:?} {:?}", r.failures, r.skipped);
        }
        assert_eq!(fs::read_to_string(src.join("a.txt")).unwrap(), "new", "the source must be kept");
        assert_eq!(fs::read_to_string(outside.join("keep.txt")).unwrap(), "outside");
        assert_eq!(
            fs::read_dir(&outside).unwrap().count(),
            1,
            "nothing may be written into the junction target"
        );
    }

    #[test]
    fn unique_names() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(unique_name(tmp.path(), "a.tar.gz", false), tmp.path().join("a.tar (2).gz"));
        assert_eq!(unique_name(tmp.path(), ".gitignore", false), tmp.path().join(".gitignore (2)"));
        assert_eq!(unique_name(tmp.path(), "my.folder", true), tmp.path().join("my.folder (2)"));
    }

    #[test]
    fn conflict_check() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("a.txt"), "").unwrap();
        let other = tempfile::tempdir().unwrap();
        fs::write(other.path().join("a.txt"), "").unwrap();
        fs::write(other.path().join("b.txt"), "").unwrap();
        let c = find_conflicts(&[other.path().join("a.txt"), other.path().join("b.txt")], tmp.path());
        assert_eq!(c.conflicts, vec!["a.txt".to_string()]);
        assert!(!c.same_folder);
        assert!(find_conflicts(&[tmp.path().join("a.txt")], tmp.path()).same_folder);
    }

    /// Treats every path as a network share. With `same_share`, whole-item renames succeed
    /// (a move inside one share); otherwise they fail like across devices.
    struct NetworkShare {
        same_share: bool,
    }
    impl safe_copy::Hooks for NetworkShare {
        fn is_network(&self, _: &Path) -> bool {
            true
        }
        fn rename_item(&self, from: &Path, to: &Path) -> std::io::Result<()> {
            if self.same_share {
                crate::platform::move_no_replace(from, to)
            } else {
                Err(std::io::Error::from_raw_os_error(ERROR_NOT_SAME_DEVICE))
            }
        }
    }

    fn snapshot(root: &Path) -> Vec<(String, Vec<u8>)> {
        let mut out = vec![];
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            for e in fs::read_dir(&dir).unwrap() {
                let p = e.unwrap().path();
                if p.is_dir() {
                    stack.push(p.clone());
                    out.push((p.strip_prefix(root).unwrap().display().to_string(), vec![]));
                } else {
                    out.push((p.strip_prefix(root).unwrap().display().to_string(), fs::read(&p).unwrap()));
                }
            }
        }
        out.sort();
        out
    }

    #[test]
    fn network_moves_that_would_delete_the_original_are_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let (src, dst) = (tmp.path().join("src"), tmp.path().join("dst"));
        tree(&src);
        fs::create_dir(&dst).unwrap();
        let before = (snapshot(&src), snapshot(&dst));
        let (p, r) = run_with(
            &NetworkShare { same_share: false },
            TransferKind::Move,
            vec![src.join("docs")],
            &dst,
            ConflictPolicy::Replace,
        );
        assert_eq!(p.state, OperationState::Completed, "{r:?}");
        assert_eq!(r.failures.len(), 1);
        assert_eq!(r.failures[0].message, NETWORK_MOVE_REFUSED);
        assert!(r.completed.is_empty());
        assert_eq!((snapshot(&src), snapshot(&dst)), before, "nothing changed on either side");
    }

    #[test]
    fn network_merge_moves_are_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let (src, dst) = (tmp.path().join("src"), tmp.path().join("dst"));
        tree(&src);
        fs::create_dir_all(dst.join("docs")).unwrap();
        fs::write(dst.join("docs/a.txt"), "old alpha").unwrap();
        let before = (snapshot(&src), snapshot(&dst));
        let (_, r) = run_with(
            &NetworkShare { same_share: true },
            TransferKind::Move,
            vec![src.join("docs")],
            &dst,
            ConflictPolicy::Replace,
        );
        assert_eq!(r.failures[0].message, NETWORK_MOVE_REFUSED);
        assert_eq!((snapshot(&src), snapshot(&dst)), before);
    }

    #[test]
    fn network_moves_inside_one_share_are_renames() {
        let tmp = tempfile::tempdir().unwrap();
        let (src, dst) = (tmp.path().join("src"), tmp.path().join("dst"));
        tree(&src);
        fs::create_dir(&dst).unwrap();
        let (p, r) = run_with(
            &NetworkShare { same_share: true },
            TransferKind::Move,
            vec![src.join("docs")],
            &dst,
            ConflictPolicy::Skip,
        );
        assert_eq!(p.state, OperationState::Completed, "{r:?}");
        assert!(!src.join("docs").exists());
        assert_eq!(fs::read_to_string(dst.join("docs/deep/b.txt")).unwrap(), "bravo!");
    }

    #[test]
    fn network_copies_work_and_never_touch_the_source() {
        let tmp = tempfile::tempdir().unwrap();
        let (src, dst) = (tmp.path().join("src"), tmp.path().join("dst"));
        tree(&src);
        fs::create_dir_all(dst.join("docs")).unwrap();
        fs::write(dst.join("docs/a.txt"), "old alpha").unwrap();
        let before = snapshot(&src);
        // Merge copy into an existing folder on the share; the existing file is replaced
        // through the staged copy, the source stays untouched.
        let (p, r) = run_with(
            &NetworkShare { same_share: false },
            TransferKind::Copy,
            vec![src.join("docs")],
            &dst,
            ConflictPolicy::Replace,
        );
        assert_eq!(p.state, OperationState::Completed, "{r:?}");
        assert!(r.failures.is_empty(), "{r:?}");
        assert_eq!(snapshot(&src), before);
        assert_eq!(fs::read_to_string(dst.join("docs/a.txt")).unwrap(), "alpha");
        assert_eq!(fs::read_to_string(dst.join("docs/deep/b.txt")).unwrap(), "bravo!");
    }
}
