//! Staged file copy: an existing destination survives every failure and every cancellation,
//! and Nexdir never deletes or adopts a file it did not create itself.
//!
//! Every step works on **handles**, never on "whatever this path names right now":
//!
//! 1. **Reserve** a temporary file `~nexdir-<random>.partial` in the destination folder with
//!    `CREATE_NEW` and no sharing (`OwnedTemp`). An existing file is never opened, so a name
//!    taken by someone else only makes Nexdir try another name. While Nexdir holds the handle,
//!    no other program can open, write, rename, replace or delete the file.
//! 2. **Copy** the source into that handle (1 MiB chunks, cancellable), hashing the bytes with
//!    BLAKE3 as they are read. The source is opened once; a source that changes during the copy
//!    (size or modification time) fails the operation. Alternate data streams such as
//!    `Zone.Identifier` (the "downloaded from the internet" mark) are copied too.
//! 3. **Verify**: optionally flush to disk, then read the temporary file back *through the same
//!    handle* and compare length and BLAKE3 hash.
//! 4. **Put it in place** by renaming the handle (`SetFileInformationByHandle`, never replacing):
//!    * new destination: rename temp → destination; fails if the name appeared meanwhile;
//!    * existing destination (*Replace*): open the destination itself, rename *that handle* to a
//!      backup name, rename temp → destination, and on failure rename the backup handle back.
//!      Only after success is the backup deleted — through its handle, so only the previous
//!      version itself can be removed.
//! 5. **Move**: the source is deleted through the handle that was copied from, and only if it
//!    did not change. A file that replaced the source path in the meantime is never touched.
//!
//! Cleanup of the temporary file also goes through its handle (`FileDispositionInfo`). If that
//! fails, the file is left in place and reported; nothing is deleted by path.
//!
//! This replaces the earlier `CopyFileExW`/`ReplaceFileW` approach: both APIs open files by
//! path themselves, which requires releasing Nexdir's exclusive handle and reopens the window in
//! which another program could swap the file.

use crate::platform::{self, HandleInfo};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub const TEMP_PREFIX: &str = "~nexdir-";
const CHUNK: usize = 1 << 20;
/// Attempts to find a free temporary name before giving up.
const MAX_NAME_ATTEMPTS: u32 = 32;

const ERROR_FILE_NOT_FOUND: i32 = 2;
const ERROR_PATH_NOT_FOUND: i32 = 3;
const ERROR_FILE_EXISTS: i32 = 80;
const ERROR_ALREADY_EXISTS: i32 = 183;
const ATTR_READONLY: u32 = 0x1;
/// Attributes carried over from the source (read-only, hidden, system, archive).
const COPIED_ATTRIBUTES: u32 = 0x1 | 0x2 | 0x4 | 0x20;

/// Points where tests can interfere, to trigger race conditions deterministically. The real
/// implementation (`NoHooks`) does nothing; the payloads exist for hooks and are not all read.
#[allow(dead_code)]
pub enum Stage<'a> {
    /// Right before the temporary file is created under `path`.
    BeforeCreateTemp(&'a Path),
    /// The data is in the temporary file (handle and path), not yet verified.
    AfterCopy(&'a File, &'a Path),
    /// Right before the existing destination is moved to the backup name (*Replace*).
    BeforeBackup(&'a Path),
    /// Right before the temporary file is renamed to the destination. An error here is
    /// treated like a failed rename.
    BeforeAdopt(&'a Path),
    /// Right before a backup is renamed back to the destination. An error here is treated
    /// like a failed rename.
    BeforeRestore(&'a Path),
}

pub trait Hooks: Sync {
    fn temp_name(&self, dir: &Path, suffix: &str, _attempt: u32) -> PathBuf {
        random_name(dir, suffix)
    }
    fn at(&self, _stage: Stage) -> io::Result<()> {
        Ok(())
    }
    /// Called before each chunk is written; an error simulates a write failure.
    fn on_chunk(&self, _index: u64) -> io::Result<()> {
        Ok(())
    }
    /// Renames a whole item for same-volume moves (used by the transfer engine). Atomic and
    /// never replaces.
    fn rename_item(&self, from: &Path, to: &Path) -> io::Result<()> {
        platform::move_no_replace(from, to)
    }
}

pub struct NoHooks;
impl Hooks for NoHooks {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// The destination must not exist.
    CreateNew,
    /// The destination exists and the user chose to replace it.
    Replace,
}

#[derive(Debug, Clone, Copy)]
pub struct Options {
    pub mode: Mode,
    /// Flush to disk before putting the copy in place (always done for *Replace*).
    pub durable: bool,
    /// Move: delete the source after the copy is in place.
    pub delete_source: bool,
    /// Copy the file a source link points to (top-level "copy" of a file link, like Explorer).
    /// Otherwise a source that is a link is refused.
    pub follow_source_link: bool,
}

#[derive(Debug)]
pub enum CopyError {
    Cancelled,
    /// Nothing at the destination was changed (and the source is unchanged).
    Failed(String),
    /// The previous destination file could not be put back automatically; it is kept under
    /// `backup` and never deleted by Nexdir.
    BackupKept {
        message: String,
        backup: PathBuf,
    },
}

/// A successful copy. The fields name things that could not be cleaned up; they are reported,
/// never resolved by deleting files by path.
#[derive(Debug, Default)]
pub struct Copied {
    /// The previous version (after *Replace*) could not be removed and is kept here.
    pub leftover: Option<PathBuf>,
    /// Move only: why the source was kept.
    pub source_kept: Option<String>,
    /// A temporary file that could not be removed.
    pub temp_left: Option<PathBuf>,
}

fn random_name(dir: &Path, suffix: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let id = nanos ^ (u64::from(std::process::id()) << 32) ^ n.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    dir.join(format!("{TEMP_PREFIX}{id:016x}.{suffix}"))
}

fn describe(e: &io::Error) -> String {
    match e.raw_os_error() {
        Some(32) | Some(33) => "The file is in use by another program.".into(),
        Some(5) => "Access was denied (the file may be read-only, or you lack permission).".into(),
        Some(112) | Some(39) => "There is not enough space on the destination drive.".into(),
        Some(19) => "The destination is write-protected.".into(),
        _ => e.to_string(),
    }
}

fn exists_error(e: &io::Error) -> bool {
    e.kind() == io::ErrorKind::AlreadyExists || matches!(e.raw_os_error(), Some(ERROR_FILE_EXISTS) | Some(ERROR_ALREADY_EXISTS))
}

// --- ownership of the temporary file ------------------------------------------------------

/// A temporary file that this process created itself (`CREATE_NEW`) and holds open without
/// sharing. Ownership is the open handle, not the path: removal goes through the handle, so it
/// can only ever affect this file. Dropping it removes the file unless it was adopted.
struct OwnedTemp {
    file: Option<File>,
    path: PathBuf,
}

impl OwnedTemp {
    /// Reserves a new temporary file next to `target`. A name that already exists (whoever
    /// created it, whenever) is skipped; it is never opened, changed or removed.
    fn create(hooks: &dyn Hooks, dir: &Path) -> Result<Self, CopyError> {
        for attempt in 0..MAX_NAME_ATTEMPTS {
            let path = hooks.temp_name(dir, "partial", attempt);
            let _ = hooks.at(Stage::BeforeCreateTemp(&path));
            match OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .access_mode(platform::ACCESS_GENERIC_READ | platform::ACCESS_GENERIC_WRITE | platform::ACCESS_DELETE)
                .share_mode(0)
                .open(&path)
            {
                Ok(file) => return Ok(Self { file: Some(file), path }),
                Err(e) if exists_error(&e) => continue,
                Err(e) => return Err(CopyError::Failed(describe(&e))),
            }
        }
        Err(CopyError::Failed(
            "Could not reserve a temporary file name in the destination folder (all tried names were taken). Nothing was changed.".into(),
        ))
    }

    fn file(&self) -> &File {
        self.file.as_ref().expect("present until adopted or discarded")
    }

    /// Removes the temporary file through its handle. Returns its path if that failed (the
    /// file is then left in place for the user, never deleted by path).
    fn discard(mut self) -> Option<PathBuf> {
        let file = self.file.take()?;
        let result = platform::delete_by_handle(&file);
        drop(file);
        result.err().map(|_| self.path.clone())
    }

    /// Renames the temporary file to `target` without replacing anything. On success the file
    /// is no longer temporary and is returned (handle still open).
    fn adopt(mut self, hooks: &dyn Hooks, target: &Path) -> Result<File, (io::Error, Self)> {
        if let Err(e) = hooks.at(Stage::BeforeAdopt(target)) {
            return Err((e, self));
        }
        match platform::rename_by_handle(self.file(), target, false) {
            Ok(()) => Ok(self.file.take().expect("present")),
            Err(e) => Err((e, self)),
        }
    }
}

impl Drop for OwnedTemp {
    fn drop(&mut self) {
        // Safety net for early returns and panics; normal paths call `discard` and report.
        if let Some(file) = self.file.take() {
            let _ = platform::delete_by_handle(&file);
        }
    }
}

// --- the copy -----------------------------------------------------------------------------

fn open_source(src: &Path, opts: &Options) -> Result<(File, HandleInfo), CopyError> {
    let mut access = platform::ACCESS_GENERIC_READ;
    if opts.delete_source {
        access |= platform::ACCESS_DELETE;
    }
    let mut o = OpenOptions::new();
    o.read(true)
        .access_mode(access)
        // Others may keep reading/writing; changes are detected below.
        .share_mode(platform::SHARE_READ | platform::SHARE_WRITE | platform::SHARE_DELETE);
    if !opts.follow_source_link {
        o.custom_flags(platform::FLAG_OPEN_REPARSE_POINT);
    }
    let file = o.open(src).map_err(|e| CopyError::Failed(describe(&e)))?;
    let info = platform::handle_info(&file).map_err(|e| CopyError::Failed(describe(&e)))?;
    if info.is_dir() {
        return Err(CopyError::Failed("The source is a folder.".into()));
    }
    if info.is_reparse_point() {
        return Err(CopyError::Failed("The source is a link. Links are not followed.".into()));
    }
    Ok((file, info))
}

/// Opens the existing destination for the swap: with DELETE access (to rename it to the backup
/// name), without following links, and without letting others write, rename or delete it
/// meanwhile. `Ok(None)` if it does not exist (any more).
fn open_destination(target: &Path, src_info: &HandleInfo) -> Result<Option<File>, CopyError> {
    let file = match OpenOptions::new()
        .access_mode(platform::ACCESS_DELETE | platform::ACCESS_READ_ATTRIBUTES)
        .share_mode(platform::SHARE_READ)
        .custom_flags(platform::FLAG_OPEN_REPARSE_POINT)
        .open(target)
    {
        Ok(f) => f,
        Err(e) if matches!(e.raw_os_error(), Some(ERROR_FILE_NOT_FOUND) | Some(ERROR_PATH_NOT_FOUND)) => return Ok(None),
        Err(e) => return Err(CopyError::Failed(format!("{} The existing file was left unchanged.", describe(&e)))),
    };
    let info = platform::handle_info(&file).map_err(|e| CopyError::Failed(describe(&e)))?;
    if info.is_reparse_point() || info.is_dir() {
        return Err(CopyError::Failed(
            "The destination is a link or folder and was left unchanged.".into(),
        ));
    }
    if info.identity == src_info.identity {
        return Err(CopyError::Failed("Source and destination are the same file.".into()));
    }
    if info.attributes & ATTR_READONLY != 0 {
        return Err(CopyError::Failed("The existing file is read-only. It was left unchanged.".into()));
    }
    Ok(Some(file))
}

/// Copies the main data stream into the temporary file and returns the BLAKE3 hash of what was
/// read from the source.
fn copy_data(
    hooks: &dyn Hooks,
    src: &mut File,
    temp: &mut File,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(u64),
) -> Result<(blake3::Hash, u64), CopyError> {
    let mut hasher = blake3::Hasher::new();
    let mut buf = vec![0u8; CHUNK];
    let mut done = 0u64;
    let mut index = 0u64;
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(CopyError::Cancelled);
        }
        let n = src.read(&mut buf).map_err(|e| CopyError::Failed(describe(&e)))?;
        if n == 0 {
            return Ok((hasher.finalize(), done));
        }
        hooks.on_chunk(index).map_err(|e| CopyError::Failed(describe(&e)))?;
        temp.write_all(&buf[..n]).map_err(|e| CopyError::Failed(describe(&e)))?;
        hasher.update(&buf[..n]);
        done += n as u64;
        index += 1;
        progress(done);
    }
}

fn hash_handle(file: &mut File, cancel: &AtomicBool) -> Result<(blake3::Hash, u64), CopyError> {
    file.seek(SeekFrom::Start(0)).map_err(|e| CopyError::Failed(describe(&e)))?;
    let mut hasher = blake3::Hasher::new();
    let mut buf = vec![0u8; CHUNK];
    let mut len = 0u64;
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(CopyError::Cancelled);
        }
        let n = file.read(&mut buf).map_err(|e| CopyError::Failed(describe(&e)))?;
        if n == 0 {
            return Ok((hasher.finalize(), len));
        }
        hasher.update(&buf[..n]);
        len += n as u64;
    }
}

/// Alternate data streams (e.g. `Zone.Identifier`). Best effort: file systems without streams
/// (FAT, some network shares) simply do not keep them, as with Explorer.
fn copy_streams(src: &Path, temp_path: &Path) {
    let Ok(names) = platform::alternate_streams(src) else { return };
    for name in names {
        let stream_path = |p: &Path| {
            let mut s = p.as_os_str().to_owned();
            s.push(format!(":{name}"));
            PathBuf::from(s)
        };
        if let Ok(data) = fs::read(stream_path(src)) {
            if let Ok(mut out) = OpenOptions::new().write(true).create_new(true).open(stream_path(temp_path)) {
                let _ = out.write_all(&data);
            }
        }
    }
}

/// Copies `src` to `target` without ever risking an existing `target` or a file that does not
/// belong to Nexdir. See the module documentation.
pub fn copy(
    hooks: &dyn Hooks,
    src: &Path,
    target: &Path,
    opts: Options,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(u64),
) -> Result<Copied, CopyError> {
    let dir = target
        .parent()
        .ok_or_else(|| CopyError::Failed("The destination has no parent folder.".into()))?;
    let (mut source, src_info) = open_source(src, &opts)?;
    let src_meta = source.metadata().map_err(|e| CopyError::Failed(describe(&e)))?;

    // An existing destination is checked and locked against changes before anything is written.
    let destination = match opts.mode {
        Mode::Replace => open_destination(target, &src_info)?,
        Mode::CreateNew => None,
    };

    // 1. Reserve; 2. copy.
    let mut temp = OwnedTemp::create(hooks, dir)?;
    let copied = copy_data(hooks, &mut source, temp.file.as_mut().expect("present"), cancel, progress);
    let fail = |temp: OwnedTemp, e: CopyError| -> CopyError {
        match (temp.discard(), e) {
            (Some(left), CopyError::Failed(m)) => {
                CopyError::Failed(format!("{m} A temporary file could not be removed: {}", left.display()))
            }
            (_, e) => e,
        }
    };
    let (src_hash, len) = match copied {
        Ok(v) => v,
        Err(e) => return Err(fail(temp, e)),
    };
    let after = platform::handle_info(&source).map_err(|e| CopyError::Failed(describe(&e)));
    match after {
        Ok(a) if a.size == src_info.size && a.last_write == src_info.last_write && len == src_info.size => {}
        Ok(_) => {
            return Err(fail(
                temp,
                CopyError::Failed("The source changed while it was being copied. Nothing was changed at the destination.".into()),
            ))
        }
        Err(e) => return Err(fail(temp, e)),
    }
    copy_streams(src, &temp.path);
    if let Err(e) = hooks.at(Stage::AfterCopy(temp.file(), &temp.path)) {
        return Err(fail(temp, CopyError::Failed(describe(&e))));
    }

    // 3. Verify through the same handle.
    if opts.durable || opts.mode == Mode::Replace {
        if let Err(e) = temp.file().sync_all() {
            return Err(fail(temp, CopyError::Failed(describe(&e))));
        }
    }
    match hash_handle(temp.file.as_mut().expect("present"), cancel) {
        Ok((h, l)) if h == src_hash && l == len => {}
        Ok(_) => {
            return Err(fail(
                temp,
                CopyError::Failed("The copy does not match the original, so it was discarded.".into()),
            ))
        }
        Err(e) => return Err(fail(temp, e)),
    }

    // 4. Put it in place.
    let mut result = Copied::default();
    let placed = match destination {
        None => match temp.adopt(hooks, target) {
            Ok(f) => f,
            Err((e, temp)) => {
                let message = if exists_error(&e) {
                    "Another item with this name appeared in the destination meanwhile; it was left unchanged.".into()
                } else {
                    describe(&e)
                };
                return Err(fail(temp, CopyError::Failed(message)));
            }
        },
        Some(old) => {
            let (placed, leftover) = swap(hooks, temp, old, target, dir)?;
            result.leftover = leftover;
            placed
        }
    };

    // Timestamps and attributes like the source (best effort, through the handle).
    let attributes = src_meta.file_attributes() & COPIED_ATTRIBUTES;
    let _ = platform::set_basic_info_by_handle(
        &placed,
        src_meta.creation_time() as i64,
        src_meta.last_write_time() as i64,
        attributes,
    );
    drop(placed);

    // 5. Move: delete the source through the handle it was copied from, if unchanged.
    if opts.delete_source {
        result.source_kept = match platform::handle_info(&source) {
            Ok(now) if now == src_info => platform::delete_by_handle(&source).err().map(|e| describe(&e)),
            Ok(_) => Some("The original changed after it was copied, so it was kept.".into()),
            Err(e) => Some(describe(&e)),
        };
    }
    Ok(result)
}

/// *Replace*: previous version → backup name (through its handle), temp → destination, then
/// the backup is deleted through its handle. Rolls back if the destination cannot be taken.
fn swap(hooks: &dyn Hooks, temp: OwnedTemp, old: File, target: &Path, dir: &Path) -> Result<(File, Option<PathBuf>), CopyError> {
    let left = |temp: OwnedTemp, message: String| match temp.discard() {
        Some(path) => CopyError::Failed(format!("{message} A temporary file could not be removed: {}", path.display())),
        None => CopyError::Failed(message),
    };
    if let Err(e) = hooks.at(Stage::BeforeBackup(target)) {
        return Err(left(temp, describe(&e)));
    }
    // Move the previous version aside under a fresh name (never replacing anything).
    let mut backup = None;
    for attempt in 0..MAX_NAME_ATTEMPTS {
        let candidate = hooks.temp_name(dir, "backup", attempt);
        match platform::rename_by_handle(&old, &candidate, false) {
            Ok(()) => {
                backup = Some(candidate);
                break;
            }
            Err(e) if exists_error(&e) => continue,
            Err(e) => return Err(left(temp, format!("{} The existing file was left unchanged.", describe(&e)))),
        }
    }
    let Some(backup) = backup else {
        return Err(left(
            temp,
            "Could not reserve a backup name. The existing file was left unchanged.".into(),
        ));
    };

    match temp.adopt(hooks, target) {
        Ok(placed) => {
            // The new file is in place. Remove the previous version — only that file.
            let leftover = platform::delete_by_handle(&old).err().map(|_| backup);
            Ok((placed, leftover))
        }
        Err((e, temp)) => {
            // Put the previous version back. Never replace whatever now has the name.
            let restored = hooks
                .at(Stage::BeforeRestore(target))
                .and_then(|()| platform::rename_by_handle(&old, target, false));
            let why = if exists_error(&e) {
                "Another program created a file with this name during the replace.".to_string()
            } else {
                describe(&e)
            };
            match restored {
                Ok(()) => Err(left(
                    temp,
                    format!("The file could not be replaced ({why}); the previous version was restored."),
                )),
                Err(_) => {
                    let message = format!("The file could not be replaced ({why}), and the previous version could not be renamed back.");
                    let _ = temp.discard();
                    Err(CopyError::BackupKept { message, backup })
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::sync::{Arc, Barrier, Mutex};

    fn own_leftovers(dir: &Path) -> Vec<String> {
        fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with(TEMP_PREFIX) && !n.contains("foreign"))
            .collect()
    }

    fn setup() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src.bin");
        let dst = dir.path().join("dst").join("report.bin");
        fs::create_dir(dir.path().join("dst")).unwrap();
        fs::write(&src, (0..300_000u32).map(|i| (i % 251) as u8).collect::<Vec<_>>()).unwrap();
        fs::write(&dst, b"original contents").unwrap();
        (dir, src, dst)
    }

    fn opts(mode: Mode) -> Options {
        Options {
            mode,
            durable: true,
            delete_source: false,
            follow_source_link: false,
        }
    }

    fn run(hooks: &dyn Hooks, src: &Path, dst: &Path, mode: Mode, cancel: &AtomicBool) -> Result<Copied, CopyError> {
        copy(hooks, src, dst, opts(mode), cancel, &mut |_| {})
    }

    fn assert_original(dst: &Path) {
        assert_eq!(
            fs::read(dst).unwrap(),
            b"original contents",
            "the existing destination must be unchanged"
        );
        let left = own_leftovers(dst.parent().unwrap());
        assert!(left.is_empty(), "no Nexdir temporary files may remain: {left:?}");
    }

    /// Fixed names for the temporary file, to provoke collisions on purpose (backup names stay
    /// random).
    struct Names(Vec<&'static str>);
    impl Hooks for Names {
        fn temp_name(&self, dir: &Path, suffix: &str, attempt: u32) -> PathBuf {
            if suffix != "partial" {
                return random_name(dir, suffix);
            }
            dir.join(self.0[(attempt as usize).min(self.0.len() - 1)])
        }
    }

    // --- Test A: temporary name already taken -------------------------------------------------

    #[test]
    fn a_taken_temp_name_is_skipped_and_never_touched() {
        let (_d, src, dst) = setup();
        let foreign = dst.with_file_name("~nexdir-foreign-a.partial");
        fs::write(&foreign, "belongs to someone else").unwrap();
        let hooks = Names(vec!["~nexdir-foreign-a.partial", "~nexdir-own-1.partial"]);
        run(&hooks, &src, &dst, Mode::Replace, &AtomicBool::new(false)).unwrap();
        assert_eq!(fs::read_to_string(&foreign).unwrap(), "belongs to someone else");
        assert_eq!(fs::read(&dst).unwrap(), fs::read(&src).unwrap());
        assert!(own_leftovers(dst.parent().unwrap()).is_empty());
    }

    #[test]
    fn a_all_temp_names_taken_aborts_without_touching_anything() {
        let (_d, src, dst) = setup();
        let foreign = dst.with_file_name("~nexdir-foreign-b.partial");
        fs::write(&foreign, "foreign").unwrap();
        let r = run(
            &Names(vec!["~nexdir-foreign-b.partial"]),
            &src,
            &dst,
            Mode::Replace,
            &AtomicBool::new(false),
        );
        assert!(matches!(r, Err(CopyError::Failed(ref m)) if m.contains("reserve")), "{r:?}");
        assert_eq!(fs::read_to_string(&foreign).unwrap(), "foreign");
        assert_original(&dst);
    }

    // --- Test B: another process takes the name during creation --------------------------------

    /// Another *process* creates a file under the exact name Nexdir is about to use, in the
    /// window right before Nexdir's create call (the old check-then-create race).
    struct ProcessTakesName(Mutex<Vec<PathBuf>>);
    impl Hooks for ProcessTakesName {
        fn temp_name(&self, dir: &Path, _suffix: &str, attempt: u32) -> PathBuf {
            dir.join(format!("~nexdir-foreign-race-{attempt}.partial"))
        }
        fn at(&self, stage: Stage) -> io::Result<()> {
            if let Stage::BeforeCreateTemp(path) = stage {
                let mut taken = self.0.lock().unwrap();
                if taken.is_empty() {
                    let out = std::process::Command::new("cmd").args(["/C", "echo foreign>"]).arg(path).output()?;
                    assert!(out.status.success());
                    taken.push(path.to_path_buf());
                }
            }
            Ok(())
        }
    }

    #[test]
    fn b_name_taken_by_another_process_during_creation_is_left_alone() {
        let (_d, src, dst) = setup();
        let hooks = ProcessTakesName(Mutex::new(Vec::new()));
        run(&hooks, &src, &dst, Mode::Replace, &AtomicBool::new(false)).unwrap();
        let taken = hooks.0.lock().unwrap()[0].clone();
        assert_eq!(
            fs::read_to_string(&taken).unwrap().trim(),
            "foreign",
            "the other process's file must be unchanged"
        );
        assert_eq!(fs::read(&dst).unwrap(), fs::read(&src).unwrap());
    }

    // --- Test C: error during the copy -----------------------------------------------------------

    struct FailAtChunk(u64, i32);
    impl Hooks for FailAtChunk {
        fn on_chunk(&self, index: u64) -> io::Result<()> {
            if index == self.0 {
                Err(io::Error::from_raw_os_error(self.1))
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn c_disk_full_during_copy_keeps_target_source_and_foreign_files() {
        let (d, _src, dst) = setup();
        let big = d.path().join("big.bin");
        fs::write(&big, vec![0x11u8; 3 * CHUNK + 5]).unwrap();
        let before = fs::read(&big).unwrap();
        let foreign = dst.with_file_name("~nexdir-foreign-c.partial");
        fs::write(&foreign, "foreign").unwrap();
        let r = run(&FailAtChunk(1, 112), &big, &dst, Mode::Replace, &AtomicBool::new(false));
        assert!(
            matches!(r, Err(CopyError::Failed(ref m)) if m.contains("not enough space")),
            "{r:?}"
        );
        assert_original(&dst);
        assert_eq!(fs::read(&big).unwrap(), before, "the source must be unchanged");
        assert_eq!(fs::read_to_string(&foreign).unwrap(), "foreign");
    }

    #[test]
    fn c_source_changing_during_copy_is_detected() {
        let (d, _src, dst) = setup();
        let big = d.path().join("big.bin");
        fs::write(&big, vec![0x22u8; 3 * CHUNK]).unwrap();
        struct Writer(PathBuf);
        impl Hooks for Writer {
            fn on_chunk(&self, index: u64) -> io::Result<()> {
                if index == 1 {
                    let mut f = OpenOptions::new().append(true).open(&self.0)?;
                    f.write_all(b"appended by another program")?;
                }
                Ok(())
            }
        }
        let r = run(&Writer(big.clone()), &big, &dst, Mode::Replace, &AtomicBool::new(false));
        assert!(matches!(r, Err(CopyError::Failed(ref m)) if m.contains("changed while")), "{r:?}");
        assert_original(&dst);
    }

    #[test]
    fn c_corrupted_temp_file_is_detected_and_discarded() {
        let (_d, src, dst) = setup();
        struct Corrupt;
        impl Hooks for Corrupt {
            fn at(&self, stage: Stage) -> io::Result<()> {
                if let Stage::AfterCopy(file, _) = stage {
                    let mut f = file;
                    f.seek(SeekFrom::Start(123))?;
                    f.write_all(&[0xFF])?;
                }
                Ok(())
            }
        }
        let r = run(&Corrupt, &src, &dst, Mode::Replace, &AtomicBool::new(false));
        assert!(matches!(r, Err(CopyError::Failed(ref m)) if m.contains("does not match")), "{r:?}");
        assert_original(&dst);
    }

    // --- Test D: cancellation --------------------------------------------------------------------

    #[test]
    fn d_cancel_before_start_keeps_everything() {
        let (_d, src, dst) = setup();
        let r = run(&NoHooks, &src, &dst, Mode::Replace, &AtomicBool::new(true));
        assert!(matches!(r, Err(CopyError::Cancelled)), "{r:?}");
        assert_original(&dst);
    }

    #[test]
    fn d_cancel_in_the_middle_keeps_target_and_foreign_files() {
        let (d, _src, dst) = setup();
        let big = d.path().join("big.bin");
        fs::write(&big, vec![0x5A; 64 * 1024 * 1024]).unwrap();
        let foreign = dst.with_file_name("~nexdir-foreign-d.partial");
        fs::write(&foreign, "foreign").unwrap();
        let cancel = AtomicBool::new(false);
        let r = copy(&NoHooks, &big, &dst, opts(Mode::Replace), &cancel, &mut |done| {
            if done >= 2 * CHUNK as u64 {
                cancel.store(true, Ordering::Relaxed);
            }
        });
        assert!(matches!(r, Err(CopyError::Cancelled)), "{r:?}");
        assert_original(&dst);
        assert_eq!(fs::read_to_string(&foreign).unwrap(), "foreign");
    }

    // --- Test E: parallel operations ---------------------------------------------------------------

    #[test]
    fn e_parallel_copies_into_one_folder_do_not_collide() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("dest");
        fs::create_dir(&dest).unwrap();
        fs::write(dest.join("shared.txt"), "initial").unwrap();
        let mut sources = Vec::new();
        for i in 0..8 {
            let s = dir.path().join(format!("src{i}"));
            fs::create_dir(&s).unwrap();
            // Same file name in every source folder, different content.
            fs::write(s.join("report.txt"), format!("content {i} ").repeat(50_000 + i * 1000)).unwrap();
            sources.push(s.join("report.txt"));
        }
        let barrier = Arc::new(Barrier::new(16));
        let results = Arc::new(Mutex::new(Vec::new()));
        std::thread::scope(|scope| {
            for (i, s) in sources.iter().enumerate() {
                let (b1, r1, d1) = (barrier.clone(), results.clone(), dest.clone());
                // 8 threads create new files with the same source name…
                scope.spawn(move || {
                    let (barrier, results, dest) = (b1, r1, d1);
                    barrier.wait();
                    let r = copy(
                        &NoHooks,
                        s,
                        &dest.join(format!("report-{i}.txt")),
                        opts(Mode::CreateNew),
                        &AtomicBool::new(false),
                        &mut |_| {},
                    );
                    results.lock().unwrap().push(("new", i, r.is_ok()));
                });
                let (barrier, results, dest) = (barrier.clone(), results.clone(), dest.clone());
                // …and 8 threads replace the same destination at the same time.
                scope.spawn(move || {
                    barrier.wait();
                    let r = copy(
                        &NoHooks,
                        s,
                        &dest.join("shared.txt"),
                        opts(Mode::Replace),
                        &AtomicBool::new(false),
                        &mut |_| {},
                    );
                    let kept = matches!(r, Err(CopyError::BackupKept { .. }));
                    results.lock().unwrap().push(("replace", i, r.is_ok() || kept));
                });
            }
        });
        let results = results.lock().unwrap();
        for (i, source) in sources.iter().enumerate() {
            assert!(results.contains(&("new", i, true)), "new copy {i} failed: {results:?}");
            assert_eq!(fs::read(dest.join(format!("report-{i}.txt"))).unwrap(), fs::read(source).unwrap());
        }
        let replaced_ok = results.iter().filter(|r| r.0 == "replace" && r.2).count();
        let backups = fs::read_dir(&dest)
            .unwrap()
            .filter(|e| e.as_ref().unwrap().file_name().to_string_lossy().ends_with(".backup"))
            .count();
        eprintln!("parallel replace: {replaced_ok}/8 succeeded or kept a backup, {backups} backup file(s) reported");
        assert!(replaced_ok >= 1, "at least one replace must succeed: {results:?}");
        // The shared file is complete: exactly one source's content, never a mixture.
        let shared = fs::read(dest.join("shared.txt")).unwrap();
        assert!(
            sources.iter().any(|s| fs::read(s).unwrap() == shared),
            "shared.txt must equal one complete source"
        );
        // No temporary files; backups only where they were reported, each a complete version.
        let names: HashSet<String> = fs::read_dir(&dest)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert!(!names.iter().any(|n| n.ends_with(".partial")), "{names:?}");
        for n in names.iter().filter(|n| n.ends_with(".backup")) {
            let data = fs::read(dest.join(n)).unwrap();
            assert!(
                data == b"initial" || sources.iter().any(|s| fs::read(s).unwrap() == data),
                "backup {n} is incomplete"
            );
        }
    }

    // --- Test F: someone tries to swap the temporary file ------------------------------------------

    struct Tamper(Mutex<Vec<(String, bool)>>);
    impl Hooks for Tamper {
        fn at(&self, stage: Stage) -> io::Result<()> {
            if let Stage::AfterCopy(_, path) = stage {
                let mut log = self.0.lock().unwrap();
                let other = path.with_file_name("~nexdir-foreign-f.moved");
                log.push(("rename in this process".into(), fs::rename(path, &other).is_ok()));
                log.push(("delete in this process".into(), fs::remove_file(path).is_ok()));
                log.push(("overwrite in this process".into(), fs::write(path, "foreign content").is_ok()));
                let out = std::process::Command::new("cmd")
                    .args(["/C", "move", "/Y"])
                    .arg(path)
                    .arg(&other)
                    .output()?;
                log.push(("rename by another process".into(), out.status.success() && !path.exists()));
                let out = std::process::Command::new("cmd").args(["/C", "echo foreign>"]).arg(path).output()?;
                log.push(("overwrite by another process".into(), out.status.success()));
            }
            Ok(())
        }
    }

    #[test]
    fn f_temporary_file_cannot_be_renamed_replaced_or_deleted_by_others() {
        let (_d, src, dst) = setup();
        let hooks = Tamper(Mutex::new(Vec::new()));
        run(&hooks, &src, &dst, Mode::Replace, &AtomicBool::new(false)).unwrap();
        let log = hooks.0.lock().unwrap();
        assert_eq!(log.len(), 5);
        for (what, succeeded) in log.iter() {
            assert!(!succeeded, "{what} must be impossible while Nexdir holds the temporary file");
        }
        assert_eq!(
            fs::read(&dst).unwrap(),
            fs::read(&src).unwrap(),
            "only the verified copy may be adopted"
        );
        assert!(!dst.with_file_name("~nexdir-foreign-f.moved").exists());
    }

    // --- Test G: successful replace ----------------------------------------------------------------

    struct WatchSwap(Mutex<Vec<String>>, PathBuf);
    impl Hooks for WatchSwap {
        fn at(&self, stage: Stage) -> io::Result<()> {
            let mut log = self.0.lock().unwrap();
            match stage {
                Stage::AfterCopy(..) => log.push(format!("after copy: {}", fs::read_to_string(&self.1).unwrap_or_default())),
                Stage::BeforeBackup(_) => log.push(format!("before swap: {}", fs::read_to_string(&self.1).unwrap_or_default())),
                _ => {}
            }
            Ok(())
        }
    }

    #[test]
    fn g_replace_keeps_the_old_file_until_the_swap_and_leaves_nothing_behind() {
        let (_d, src, dst) = setup();
        let hooks = WatchSwap(Mutex::new(Vec::new()), dst.clone());
        let copied = run(&hooks, &src, &dst, Mode::Replace, &AtomicBool::new(false)).unwrap();
        assert!(copied.leftover.is_none() && copied.temp_left.is_none());
        assert_eq!(
            *hooks.0.lock().unwrap(),
            vec![
                "after copy: original contents".to_string(),
                "before swap: original contents".to_string()
            ]
        );
        assert_eq!(fs::read(&dst).unwrap(), fs::read(&src).unwrap());
        assert!(own_leftovers(dst.parent().unwrap()).is_empty());
    }

    // --- swap failures and other real Windows behaviour -------------------------------------------

    struct FailAt(fn(&Stage) -> bool, Option<PathBuf>);
    impl Hooks for FailAt {
        fn at(&self, stage: Stage) -> io::Result<()> {
            if (self.0)(&stage) {
                if let (Some(p), Stage::BeforeAdopt(_)) = (&self.1, &stage) {
                    fs::write(p, "created by another program")?; // someone takes the name
                    return Ok(());
                }
                return Err(io::Error::from_raw_os_error(32));
            }
            Ok(())
        }
    }

    #[test]
    fn failed_adopt_restores_the_previous_version() {
        let (_d, src, dst) = setup();
        let r = run(
            &FailAt(|s| matches!(s, Stage::BeforeAdopt(_)), None),
            &src,
            &dst,
            Mode::Replace,
            &AtomicBool::new(false),
        );
        assert!(matches!(r, Err(CopyError::Failed(ref m)) if m.contains("restored")), "{r:?}");
        assert_original(&dst);
    }

    #[test]
    fn name_taken_during_the_swap_keeps_both_files_and_the_backup() {
        let (_d, src, dst) = setup();
        let r = run(
            &FailAt(|s| matches!(s, Stage::BeforeAdopt(_)), Some(dst.clone())),
            &src,
            &dst,
            Mode::Replace,
            &AtomicBool::new(false),
        );
        let Err(CopyError::BackupKept { backup, message }) = r else {
            panic!("{r:?}")
        };
        assert!(message.contains("Another program"), "{message}");
        assert_eq!(
            fs::read_to_string(&dst).unwrap(),
            "created by another program",
            "the other program's file is kept"
        );
        assert_eq!(
            fs::read(&backup).unwrap(),
            b"original contents",
            "the previous version survives as the backup"
        );
        assert!(!own_leftovers(dst.parent().unwrap()).iter().any(|n| n.ends_with(".partial")));
    }

    #[test]
    fn failed_restore_keeps_the_backup_and_reports_it() {
        let (_d, src, dst) = setup();
        let r = run(
            &FailAt(|s| matches!(s, Stage::BeforeAdopt(_) | Stage::BeforeRestore(_)), None),
            &src,
            &dst,
            Mode::Replace,
            &AtomicBool::new(false),
        );
        let Err(CopyError::BackupKept { backup, .. }) = r else {
            panic!("{r:?}")
        };
        assert_eq!(fs::read(&backup).unwrap(), b"original contents");
    }

    #[test]
    fn create_new_does_not_overwrite_a_file_that_appeared() {
        let (_d, src, dst) = setup();
        let new = dst.with_file_name("new.bin");
        let r = run(
            &FailAt(|s| matches!(s, Stage::BeforeAdopt(_)), Some(new.clone())),
            &src,
            &new,
            Mode::CreateNew,
            &AtomicBool::new(false),
        );
        assert!(matches!(r, Err(CopyError::Failed(ref m)) if m.contains("appeared")), "{r:?}");
        assert_eq!(fs::read_to_string(&new).unwrap(), "created by another program");
        assert_original(&dst);
    }

    #[test]
    fn create_new_and_move_delete_the_source_only_through_its_handle() {
        let (d, src, dst) = setup();
        let new = dst.with_file_name("moved.bin");
        let data = fs::read(&src).unwrap();
        let o = Options {
            mode: Mode::CreateNew,
            durable: true,
            delete_source: true,
            follow_source_link: false,
        };
        let c = copy(&NoHooks, &src, &new, o, &AtomicBool::new(false), &mut |_| {}).unwrap();
        assert!(c.source_kept.is_none(), "{c:?}");
        assert!(!src.exists());
        assert_eq!(fs::read(&new).unwrap(), data);

        // The source path is replaced by another file after the copy: that file must survive.
        let src2 = d.path().join("src2.bin");
        fs::write(&src2, "first").unwrap();
        struct SwapSource(PathBuf);
        impl Hooks for SwapSource {
            fn at(&self, stage: Stage) -> io::Result<()> {
                if let Stage::AfterCopy(..) = stage {
                    fs::rename(&self.0, self.0.with_extension("old"))?;
                    fs::write(&self.0, "newer file from another program")?;
                }
                Ok(())
            }
        }
        let c = copy(
            &SwapSource(src2.clone()),
            &src2,
            &dst.with_file_name("m2.bin"),
            o,
            &AtomicBool::new(false),
            &mut |_| {},
        )
        .unwrap();
        assert!(c.source_kept.is_none(), "{c:?}");
        assert_eq!(
            fs::read_to_string(&src2).unwrap(),
            "newer file from another program",
            "the new file at the source path is kept"
        );
        assert!(
            !src2.with_extension("old").exists(),
            "the copied original (renamed away) was the one deleted"
        );
    }

    #[test]
    fn mark_of_the_web_is_copied() {
        let (d, _src, dst) = setup();
        let dl = d.path().join("setup.exe");
        fs::write(&dl, "MZ").unwrap();
        fs::write(format!("{}:Zone.Identifier", dl.display()), "[ZoneTransfer]\r\nZoneId=3\r\n").unwrap();
        let out = dst.with_file_name("setup.exe");
        run(&NoHooks, &dl, &out, Mode::CreateNew, &AtomicBool::new(false)).unwrap();
        assert!(fs::read_to_string(format!("{}:Zone.Identifier", out.display()))
            .unwrap()
            .contains("ZoneId=3"));
    }

    #[test]
    fn locked_destination_is_left_unchanged() {
        let (_d, src, dst) = setup();
        let lock = OpenOptions::new().read(true).share_mode(0).open(&dst).unwrap();
        let r = run(&NoHooks, &src, &dst, Mode::Replace, &AtomicBool::new(false));
        assert!(matches!(r, Err(CopyError::Failed(_))), "{r:?}");
        drop(lock);
        assert_original(&dst);
    }

    #[test]
    fn locked_source_fails_without_touching_destination() {
        let (_d, src, dst) = setup();
        let lock = OpenOptions::new().read(true).share_mode(0).open(&src).unwrap();
        let r = run(&NoHooks, &src, &dst, Mode::Replace, &AtomicBool::new(false));
        assert!(matches!(r, Err(CopyError::Failed(_))), "{r:?}");
        drop(lock);
        assert_original(&dst);
    }

    #[test]
    fn read_only_destination_is_refused() {
        let (_d, src, dst) = setup();
        let mut perms = fs::metadata(&dst).unwrap().permissions();
        perms.set_readonly(true);
        fs::set_permissions(&dst, perms.clone()).unwrap();
        let r = run(&NoHooks, &src, &dst, Mode::Replace, &AtomicBool::new(false));
        assert!(matches!(r, Err(CopyError::Failed(ref m)) if m.contains("read-only")), "{r:?}");
        #[allow(clippy::permissions_set_readonly_false)] // test cleanup of our own temp file
        perms.set_readonly(false);
        fs::set_permissions(&dst, perms).unwrap();
        assert_original(&dst);
    }

    #[test]
    fn denied_write_access_to_destination_folder_keeps_target() {
        let (_d, src, dst) = setup();
        let folder = dst.parent().unwrap().to_path_buf();
        let user = std::env::var("USERNAME").unwrap();
        // Deny creating files in this isolated test folder only; removed again below.
        let deny = std::process::Command::new("icacls")
            .arg(&folder)
            .args(["/deny", &format!("{user}:(WD,AD)")])
            .output()
            .unwrap();
        assert!(deny.status.success(), "{}", String::from_utf8_lossy(&deny.stdout));
        let r = run(&NoHooks, &src, &dst, Mode::Replace, &AtomicBool::new(false));
        let undo = std::process::Command::new("icacls")
            .arg(&folder)
            .args(["/remove:d", &user])
            .output()
            .unwrap();
        assert!(undo.status.success());
        assert!(matches!(r, Err(CopyError::Failed(_))), "{r:?}");
        assert_original(&dst);
    }

    #[test]
    fn same_file_under_another_name_is_refused() {
        let (_d, _src, dst) = setup();
        let alias = dst.with_file_name("alias.bin");
        fs::hard_link(&dst, &alias).unwrap();
        let r = run(&NoHooks, &alias, &dst, Mode::Replace, &AtomicBool::new(false));
        assert!(matches!(r, Err(CopyError::Failed(ref m)) if m.contains("same file")), "{r:?}");
        fs::remove_file(&alias).unwrap();
        assert_original(&dst);
    }

    #[test]
    fn destination_junction_and_source_junction_are_refused() {
        let (d, src, dst) = setup();
        let outside = d.path().join("outside");
        fs::create_dir(&outside).unwrap();
        let j = dst.with_file_name("junction");
        let out = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&j)
            .arg(&outside)
            .output()
            .unwrap();
        assert!(out.status.success());
        let r = run(&NoHooks, &src, &j, Mode::Replace, &AtomicBool::new(false));
        assert!(matches!(r, Err(CopyError::Failed(_))), "{r:?}");
        let r = run(&NoHooks, &j, &dst.with_file_name("x"), Mode::CreateNew, &AtomicBool::new(false));
        assert!(matches!(r, Err(CopyError::Failed(_))), "{r:?}");
        assert_eq!(fs::read_dir(&outside).unwrap().count(), 0);
        assert_original(&dst);
    }

    #[test]
    fn destination_symlink_is_not_written_through() {
        let (d, src, _dst) = setup();
        let outside = d.path().join("outside.txt");
        fs::write(&outside, "outside").unwrap();
        let link = d.path().join("dst").join("link.bin");
        if std::os::windows::fs::symlink_file(&outside, &link).is_err() {
            eprintln!("skipped: creating symbolic links needs Developer Mode or admin rights");
            return;
        }
        let r = run(&NoHooks, &src, &link, Mode::Replace, &AtomicBool::new(false));
        assert!(matches!(r, Err(CopyError::Failed(_))), "{r:?}");
        assert_eq!(fs::read_to_string(&outside).unwrap(), "outside");
    }

    #[test]
    fn timestamps_and_attributes_follow_the_source() {
        let (_d, src, dst) = setup();
        let old = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_600_000_000);
        File::options().write(true).open(&src).unwrap().set_modified(old).unwrap();
        let new = dst.with_file_name("dated.bin");
        run(&NoHooks, &src, &new, Mode::CreateNew, &AtomicBool::new(false)).unwrap();
        assert_eq!(fs::metadata(&new).unwrap().modified().unwrap(), old);
    }
}
