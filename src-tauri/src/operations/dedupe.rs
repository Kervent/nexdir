//! Removing duplicate copies found by the duplicate finder. The backend enforces the rule
//! "at least one verified copy always remains", independently of the user interface:
//!
//! 1. A group must name at least one file to keep; a group that would remove every copy is
//!    refused as a whole.
//! 2. Before anything is removed, one kept file is opened with read-only sharing (nobody can
//!    write, rename or delete it while the group is processed), checked to be a regular file
//!    (not a link or junction), and its full content is hashed again. It must still have the
//!    size and BLAKE3 hash reported by the scan. Without such a copy nothing in the group is
//!    removed.
//! 3. Every file to remove is opened (writes blocked), must be a regular file, must not be
//!    the kept file under another name (hard link, 8.3 name, different case) and must still
//!    have the same size and full hash. Files that changed or disappeared are skipped.
//! 4. Only then are the verified files sent to the Recycle Bin, while all handles stay open.
//!    Removal requests run one at a time, so two requests cannot each remove "the other" copy.
//!
//! Limits: the Recycle Bin call works on paths. Between the check and the call another
//! program could rename a verified file (rename is allowed so the Recycle Bin can move it)
//! and put a different file at that path, which would then go to the Recycle Bin — from
//! where it can be restored. The kept copy cannot be affected: it is locked throughout.

use super::{check_recyclable, ItemFailure, RecycleResult};
use crate::error::{AppError, AppResult, ErrorCode};
use crate::platform;
use serde::Deserialize;
use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read};
use std::os::windows::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const CHUNK: usize = 1024 * 1024;
pub const MAX_GROUPS: usize = 5_000;

/// Serialises removal requests (see rule 4 above).
static REMOVAL: Mutex<()> = Mutex::new(());

/// One duplicate group as the user resolved it: which copies stay and which go.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemovalGroup {
    pub size: u64,
    /// Full-content BLAKE3 hash (hex) reported by the scan.
    pub hash: String,
    pub keep: Vec<PathBuf>,
    pub remove: Vec<PathBuf>,
}

/// Sends verified paths to the Recycle Bin. Production code uses the Windows shell; tests
/// substitute a stand-in so they never touch the real Recycle Bin.
pub type Recycler<'a> = &'a dyn Fn(&[PathBuf]) -> io::Result<()>;

fn fail(failed: &mut Vec<ItemFailure>, path: &Path, message: &str) {
    failed.push(ItemFailure {
        path: path.display().to_string(),
        message: message.to_string(),
    });
}

/// Opens `path` itself (never a link target) for reading.
fn open(path: &Path, share: u32) -> io::Result<File> {
    OpenOptions::new()
        .access_mode(platform::ACCESS_GENERIC_READ)
        .share_mode(share)
        .custom_flags(platform::FLAG_OPEN_REPARSE_POINT)
        .open(path)
}

fn hash_of(file: &mut File) -> io::Result<blake3::Hash> {
    let mut hasher = blake3::Hasher::new();
    let mut buf = vec![0u8; CHUNK];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            return Ok(hasher.finalize());
        }
        hasher.update(&buf[..n]);
    }
}

/// Why a file no longer matches its group, or `None` when it does.
fn mismatch(file: &mut File, info: &platform::HandleInfo, size: u64, hash: &blake3::Hash) -> Option<&'static str> {
    if info.is_reparse_point() {
        return Some("This is a link or junction, not a copy of the file. Nothing was removed.");
    }
    if info.is_dir() {
        return Some("This is a folder, not a file. Nothing was removed.");
    }
    if info.size != size {
        return Some("The file changed since the scan (different size). It was not removed.");
    }
    match hash_of(file) {
        Ok(h) if h == *hash => None,
        Ok(_) => Some("The content changed since the scan. It was not removed."),
        Err(_) => Some("The file could not be read to confirm it is still identical. It was not removed."),
    }
}

/// Opens the first kept file that is unchanged and locks it against writes, renames and
/// deletion for as long as the returned handle lives.
fn lock_keeper(group: &RemovalGroup, hash: &blake3::Hash) -> Option<(File, platform::HandleInfo)> {
    group.keep.iter().find_map(|path| {
        let mut file = open(path, platform::SHARE_READ).ok()?;
        let info = platform::handle_info(&file).ok()?;
        mismatch(&mut file, &info, group.size, hash).is_none().then_some((file, info))
    })
}

fn process_group(group: &RemovalGroup, recycle: Recycler, result: &mut RecycleResult) {
    let mut remove: Vec<&PathBuf> = Vec::new();
    let mut seen = HashSet::new();
    for p in &group.remove {
        if seen.insert(p.display().to_string().to_lowercase()) {
            remove.push(p);
        }
    }
    if remove.is_empty() {
        return;
    }
    let refuse_all = |result: &mut RecycleResult, message: &str| {
        for p in &remove {
            fail(&mut result.failed, p, message);
        }
    };

    let hash = match blake3::Hash::from_hex(group.hash.as_bytes()) {
        Ok(h) => h,
        Err(_) => return refuse_all(result, "The duplicate group is incomplete or invalid. Nothing was removed."),
    };
    if group.keep.is_empty() {
        return refuse_all(
            result,
            "Every copy of this file was selected. At least one copy must be kept, so nothing was removed.",
        );
    }
    let kept: HashSet<String> = group.keep.iter().map(|p| p.display().to_string().to_lowercase()).collect();
    if remove.iter().any(|p| kept.contains(&p.display().to_string().to_lowercase())) {
        return refuse_all(
            result,
            "A file was marked both to keep and to remove. Nothing was removed from this group.",
        );
    }

    let Some((_keeper, keeper_info)) = lock_keeper(group, &hash) else {
        return refuse_all(
            result,
            "None of the copies to keep is still unchanged, so nothing was removed from this group. Scan again.",
        );
    };

    // Handles stay open until the Recycle Bin call returns: no writes in between.
    let mut verified: Vec<PathBuf> = Vec::new();
    let mut held: Vec<File> = Vec::new();
    for path in remove {
        if let Err(message) = check_recyclable(path) {
            fail(&mut result.failed, path, &message);
            continue;
        }
        let mut file = match open(path, platform::SHARE_READ | platform::SHARE_DELETE) {
            Ok(f) => f,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                fail(&mut result.failed, path, "The file no longer exists (already removed or moved).");
                continue;
            }
            Err(_) => {
                fail(
                    &mut result.failed,
                    path,
                    "The file is in use or cannot be opened. It was not removed.",
                );
                continue;
            }
        };
        let Ok(info) = platform::handle_info(&file) else {
            fail(&mut result.failed, path, "The file could not be checked. It was not removed.");
            continue;
        };
        if info.identity == keeper_info.identity {
            fail(
                &mut result.failed,
                path,
                "This is the same file as the copy being kept (hard link or another name for it). It was not removed.",
            );
            continue;
        }
        if let Some(message) = mismatch(&mut file, &info, group.size, &hash) {
            fail(&mut result.failed, path, message);
            continue;
        }
        verified.push(path.clone());
        held.push(file);
    }
    if verified.is_empty() {
        return;
    }

    let outcome = recycle(&verified);
    drop(held);
    for path in verified {
        match (&outcome, fs::symlink_metadata(&path)) {
            (Ok(()), Err(_)) => result.recycled.push(path.display().to_string()),
            (Err(e), _) => fail(&mut result.failed, &path, &format!("The Recycle Bin operation failed: {e}")),
            (Ok(()), Ok(_)) => fail(
                &mut result.failed,
                &path,
                "Not moved to the Recycle Bin. It may be in use, or the operation was cancelled.",
            ),
        }
    }
}

/// Moves the files marked for removal to the Recycle Bin, group by group, keeping at least
/// one verified copy of every group. Never deletes permanently.
pub fn remove_duplicates(groups: &[RemovalGroup], recycle: Recycler) -> AppResult<RecycleResult> {
    if groups.is_empty() {
        return Err(AppError::new(ErrorCode::InvalidPath, "No duplicate groups were given."));
    }
    if groups.len() > MAX_GROUPS {
        return Err(AppError::new(ErrorCode::Unsupported, "Too many duplicate groups in one request."));
    }
    let _serial = REMOVAL.lock().unwrap_or_else(|e| e.into_inner());
    let mut result = RecycleResult {
        recycled: Vec::new(),
        failed: Vec::new(),
    };
    for group in groups {
        process_group(group, recycle, &mut result);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    /// Stand-in for the Recycle Bin: moves files into `bin`, like the shell does.
    fn bin_recycler(bin: PathBuf) -> impl Fn(&[PathBuf]) -> io::Result<()> {
        let n = AtomicUsize::new(0);
        move |paths: &[PathBuf]| {
            for p in paths {
                let i = n.fetch_add(1, Ordering::SeqCst);
                // Errors are ignored like the shell's per-item failures; callers check paths.
                let _ = fs::rename(p, bin.join(format!("{i}-{}", p.file_name().unwrap().to_string_lossy())));
            }
            Ok(())
        }
    }

    struct Fixture {
        _dir: tempfile::TempDir,
        root: PathBuf,
        bin: PathBuf,
    }

    fn fixture() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("data");
        let bin = dir.path().join("bin");
        fs::create_dir(&root).unwrap();
        fs::create_dir(&bin).unwrap();
        Fixture { root, bin, _dir: dir }
    }

    fn write_copies(root: &Path, names: &[&str], content: &[u8]) -> Vec<PathBuf> {
        names
            .iter()
            .map(|n| {
                let p = root.join(n);
                fs::write(&p, content).unwrap();
                p
            })
            .collect()
    }

    fn group(content: &[u8], keep: &[&PathBuf], remove: &[&PathBuf]) -> RemovalGroup {
        RemovalGroup {
            size: content.len() as u64,
            hash: blake3::hash(content).to_hex().to_string(),
            keep: keep.iter().map(|p| (*p).clone()).collect(),
            remove: remove.iter().map(|p| (*p).clone()).collect(),
        }
    }

    #[test]
    fn two_copies_remove_one_keep_one() {
        let f = fixture();
        let c = b"same content".as_slice();
        let p = write_copies(&f.root, &["a.txt", "b.txt"], c);
        let r = remove_duplicates(&[group(c, &[&p[0]], &[&p[1]])], &bin_recycler(f.bin.clone())).unwrap();
        assert_eq!(r.recycled.len(), 1, "{:?}", r.failed);
        assert!(p[0].exists() && !p[1].exists());
        assert_eq!(fs::read_dir(&f.bin).unwrap().count(), 1);
    }

    #[test]
    fn many_copies_keep_exactly_the_chosen_one() {
        let f = fixture();
        let c = vec![9u8; 300 * 1024];
        let p = write_copies(&f.root, &["1", "2", "3", "4", "5"], &c);
        let r = remove_duplicates(&[group(&c, &[&p[2]], &[&p[0], &p[1], &p[3], &p[4]])], &bin_recycler(f.bin.clone())).unwrap();
        assert_eq!(r.recycled.len(), 4, "{:?}", r.failed);
        assert!(p[2].exists());
        assert_eq!(fs::read_dir(&f.root).unwrap().count(), 1);
    }

    #[test]
    fn selecting_every_copy_is_refused_as_a_whole() {
        let f = fixture();
        let c = b"precious".as_slice();
        let p = write_copies(&f.root, &["a", "b", "c"], c);
        let calls = AtomicUsize::new(0);
        let recycler = |_: &[PathBuf]| {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        };
        let r = remove_duplicates(&[group(c, &[], &[&p[0], &p[1], &p[2]])], &recycler).unwrap();
        assert!(r.recycled.is_empty());
        assert_eq!(r.failed.len(), 3);
        assert!(r.failed[0].message.contains("At least one copy must be kept"));
        assert_eq!(calls.load(Ordering::SeqCst), 0, "the Recycle Bin must not be called");
        assert!(p.iter().all(|x| x.exists()));
    }

    #[test]
    fn a_path_both_kept_and_removed_is_refused() {
        let f = fixture();
        let c = b"twice".as_slice();
        let p = write_copies(&f.root, &["a", "b"], c);
        // Different spelling (case) of the same path.
        let upper = PathBuf::from(p[0].display().to_string().to_uppercase());
        let r = remove_duplicates(&[group(c, &[&p[0]], &[&upper, &p[1]])], &bin_recycler(f.bin.clone())).unwrap();
        assert!(r.recycled.is_empty());
        assert!(p.iter().all(|x| x.exists()));
    }

    #[test]
    fn kept_copy_changed_since_scan_aborts_the_group() {
        let f = fixture();
        let c = b"original".as_slice();
        let p = write_copies(&f.root, &["keep", "dup"], c);
        fs::write(&p[0], b"modified").unwrap(); // same size, other content
        let r = remove_duplicates(&[group(c, &[&p[0]], &[&p[1]])], &bin_recycler(f.bin.clone())).unwrap();
        assert!(r.recycled.is_empty());
        assert!(r.failed[0].message.contains("None of the copies to keep"));
        assert!(p[1].exists(), "the only remaining original must stay");
    }

    #[test]
    fn falls_back_to_another_kept_copy_when_the_first_changed() {
        let f = fixture();
        let c = b"original".as_slice();
        let p = write_copies(&f.root, &["keep1", "keep2", "dup"], c);
        fs::write(&p[0], b"changed!").unwrap();
        let r = remove_duplicates(&[group(c, &[&p[0], &p[1]], &[&p[2]])], &bin_recycler(f.bin.clone())).unwrap();
        assert_eq!(r.recycled.len(), 1, "{:?}", r.failed);
        assert!(p[1].exists());
    }

    #[test]
    fn removed_copy_changed_since_scan_is_skipped() {
        let f = fixture();
        let c = b"original".as_slice();
        let p = write_copies(&f.root, &["keep", "dup1", "dup2"], c);
        fs::write(&p[1], b"edited!!").unwrap(); // same size
        fs::write(&p[2], b"longer content now").unwrap(); // other size
        let r = remove_duplicates(&[group(c, &[&p[0]], &[&p[1], &p[2]])], &bin_recycler(f.bin.clone())).unwrap();
        assert!(r.recycled.is_empty());
        assert_eq!(r.failed.len(), 2);
        assert!(p.iter().all(|x| x.exists()));
    }

    #[test]
    fn already_deleted_files_are_reported_not_fatal() {
        let f = fixture();
        let c = b"content".as_slice();
        let p = write_copies(&f.root, &["keep", "gone", "dup"], c);
        fs::remove_file(&p[1]).unwrap();
        let r = remove_duplicates(&[group(c, &[&p[0]], &[&p[1], &p[2]])], &bin_recycler(f.bin.clone())).unwrap();
        assert_eq!(r.recycled, vec![p[2].display().to_string()]);
        assert!(r.failed[0].message.contains("no longer exists"));
        assert!(p[0].exists());
    }

    #[test]
    fn missing_keeper_aborts_the_group() {
        let f = fixture();
        let c = b"content".as_slice();
        let p = write_copies(&f.root, &["keep", "dup"], c);
        fs::remove_file(&p[0]).unwrap();
        let r = remove_duplicates(&[group(c, &[&p[0]], &[&p[1]])], &bin_recycler(f.bin.clone())).unwrap();
        assert!(r.recycled.is_empty());
        assert!(p[1].exists());
    }

    #[test]
    fn invalid_or_incomplete_groups_are_refused() {
        let f = fixture();
        let c = b"content".as_slice();
        let p = write_copies(&f.root, &["keep", "dup"], c);
        let mut bad_hash = group(c, &[&p[0]], &[&p[1]]);
        bad_hash.hash = "not-a-hash".into();
        let mut wrong_hash = group(c, &[&p[0]], &[&p[1]]);
        wrong_hash.hash = blake3::hash(b"other").to_hex().to_string();
        let mut wrong_size = group(c, &[&p[0]], &[&p[1]]);
        wrong_size.size += 1;
        for g in [bad_hash, wrong_hash, wrong_size] {
            let r = remove_duplicates(&[g], &bin_recycler(f.bin.clone())).unwrap();
            assert!(r.recycled.is_empty());
            assert!(p.iter().all(|x| x.exists()));
        }
        assert!(remove_duplicates(&[], &bin_recycler(f.bin.clone())).is_err());
    }

    #[test]
    fn a_hard_link_of_the_kept_file_is_not_removed() {
        let f = fixture();
        let c = b"linked".as_slice();
        let p = write_copies(&f.root, &["keep"], c);
        let link = f.root.join("link");
        fs::hard_link(&p[0], &link).unwrap();
        let r = remove_duplicates(&[group(c, &[&p[0]], &[&link])], &bin_recycler(f.bin.clone())).unwrap();
        assert!(r.recycled.is_empty());
        assert!(r.failed[0].message.contains("same file as the copy being kept"));
        assert!(link.exists());
    }

    #[test]
    fn junctions_are_never_treated_as_copies() {
        let f = fixture();
        let c = b"content".as_slice();
        let p = write_copies(&f.root, &["keep"], c);
        let target = f.root.join("dir");
        fs::create_dir(&target).unwrap();
        let junction = f.root.join("junction");
        let out = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&junction)
            .arg(&target)
            .output()
            .unwrap();
        assert!(out.status.success());
        // As a file to remove, and as the only copy to keep.
        let r = remove_duplicates(&[group(c, &[&p[0]], &[&junction])], &bin_recycler(f.bin.clone())).unwrap();
        assert!(r.recycled.is_empty() && junction.exists());
        let dup = write_copies(&f.root, &["dup"], c);
        let r = remove_duplicates(&[group(c, &[&junction], &[&dup[0]])], &bin_recycler(f.bin.clone())).unwrap();
        assert!(r.recycled.is_empty() && dup[0].exists());
    }

    #[test]
    fn symlinks_are_never_treated_as_copies() {
        let f = fixture();
        let c = b"content".as_slice();
        let p = write_copies(&f.root, &["real", "dup"], c);
        let link = f.root.join("link");
        if std::os::windows::fs::symlink_file(&p[0], &link).is_err() {
            eprintln!("skipped: creating symbolic links needs Developer Mode or administrator rights");
            return;
        }
        // Keeping a link to the file being removed would lose the content.
        let r = remove_duplicates(&[group(c, &[&link], &[&p[0]])], &bin_recycler(f.bin.clone())).unwrap();
        assert!(r.recycled.is_empty() && p[0].exists());
    }

    #[test]
    fn failing_recycle_bin_reports_every_file_and_keeps_them() {
        let f = fixture();
        let c = b"content".as_slice();
        let p = write_copies(&f.root, &["keep", "dup1", "dup2"], c);
        let failing = |_: &[PathBuf]| Err(io::Error::other("simulated failure"));
        let r = remove_duplicates(&[group(c, &[&p[0]], &[&p[1], &p[2]])], &failing).unwrap();
        assert!(r.recycled.is_empty());
        assert_eq!(r.failed.len(), 2);
        // A shell that reports success but leaves the file (e.g. the user cancelled).
        let silent = |_: &[PathBuf]| Ok(());
        let r = remove_duplicates(&[group(c, &[&p[0]], &[&p[1]])], &silent).unwrap();
        assert!(r.recycled.is_empty() && r.failed.len() == 1);
        assert!(p.iter().all(|x| x.exists()));
    }

    #[test]
    fn kept_copy_is_locked_while_the_group_is_processed() {
        let f = fixture();
        let c = b"content".as_slice();
        let p = write_copies(&f.root, &["keep", "dup"], c);
        let keep = p[0].clone();
        let bin = f.bin.clone();
        // A Recycle Bin that also tries to take the kept copy, like another program would.
        let greedy = move |paths: &[PathBuf]| {
            assert!(fs::rename(&keep, bin.join("stolen")).is_err(), "the kept copy must be locked");
            assert!(fs::remove_file(&keep).is_err());
            assert!(fs::write(&keep, b"overwrite").is_err());
            for (i, x) in paths.iter().enumerate() {
                fs::rename(x, bin.join(i.to_string()))?;
            }
            Ok(())
        };
        let r = remove_duplicates(&[group(c, &[&p[0]], &[&p[1]])], &greedy).unwrap();
        assert_eq!(r.recycled.len(), 1, "{:?}", r.failed);
        assert_eq!(fs::read(&p[0]).unwrap(), c);
    }

    #[test]
    fn opposite_parallel_requests_never_remove_both_copies() {
        for _ in 0..20 {
            let f = fixture();
            let c = b"race".as_slice();
            let p = Arc::new(write_copies(&f.root, &["a", "b"], c));
            let bin = Arc::new(f.bin.clone());
            let threads: Vec<_> = [(0usize, 1usize), (1, 0)]
                .into_iter()
                .map(|(k, r)| {
                    let (p, bin) = (p.clone(), bin.clone());
                    std::thread::spawn(move || {
                        let g = group(c, &[&p[k]], &[&p[r]]);
                        remove_duplicates(&[g], &bin_recycler(bin.to_path_buf())).unwrap()
                    })
                })
                .collect();
            let total: usize = threads.into_iter().map(|t| t.join().unwrap().recycled.len()).sum();
            assert_eq!(total, 1);
            assert_eq!(p.iter().filter(|x| x.exists()).count(), 1);
        }
    }

    #[test]
    fn several_groups_are_independent() {
        let f = fixture();
        let a = b"group a".as_slice();
        let b = b"group b!".as_slice();
        let pa = write_copies(&f.root, &["a1", "a2"], a);
        let pb = write_copies(&f.root, &["b1", "b2"], b);
        let groups = [group(a, &[], &[&pa[0], &pa[1]]), group(b, &[&pb[0]], &[&pb[1]])];
        let r = remove_duplicates(&groups, &bin_recycler(f.bin.clone())).unwrap();
        assert_eq!(r.recycled, vec![pb[1].display().to_string()]);
        assert!(pa.iter().all(|x| x.exists()) && pb[0].exists());
    }

    #[test]
    #[ignore = "uses the real Windows Recycle Bin; run manually with --ignored"]
    fn real_recycle_bin_with_handles_held() {
        let f = fixture();
        let c = b"nexdir duplicate guard test".as_slice();
        let p = write_copies(&f.root, &["nexdir-dupe-keep.txt", "nexdir-dupe-remove.txt"], c);
        let shell = |paths: &[PathBuf]| {
            let batch = paths.to_vec();
            std::thread::spawn(move || platform::recycle(&batch, None)).join().unwrap()
        };
        let r = remove_duplicates(&[group(c, &[&p[0]], &[&p[1]])], &shell).unwrap();
        assert_eq!(r.recycled.len(), 1, "{:?}", r.failed);
        assert!(p[0].exists() && !p[1].exists());
    }
}
