//! Operations that change the filesystem. Every function here refuses to replace or
//! delete data unless the user explicitly chose to, and nothing is ever permanently
//! deleted: removal always goes through the Windows Recycle Bin.

pub mod dedupe;
pub mod manager;
pub mod organize;
pub mod safe_copy;
pub mod transfer;

use crate::error::{AppError, AppResult, ErrorCode};
use crate::platform::{self, DriveType};
use crate::security;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

/// Creates a new, empty folder. Never touches an existing item of the same name.
pub fn create_folder(parent: &Path, name: &str) -> AppResult<PathBuf> {
    security::validate_name(name)?;
    let meta = fs::metadata(parent).map_err(|e| AppError::from_io(&e, parent))?;
    if !meta.is_dir() {
        return Err(AppError::new(ErrorCode::NotADirectory, "The destination is not a folder.").with_path(parent));
    }
    let target = parent.join(name);
    // create_dir fails atomically if anything with this name already exists.
    fs::create_dir(&target).map_err(|e| AppError::from_io(&e, &target))?;
    Ok(target)
}

/// Renames an item inside its current folder. Fails rather than replacing an existing
/// item; a case-only rename ("a.txt" → "A.txt") is allowed.
pub fn rename_entry(path: &Path, new_name: &str) -> AppResult<PathBuf> {
    security::validate_name(new_name)?;
    let parent = path
        .parent()
        .ok_or_else(|| AppError::new(ErrorCode::Unsupported, "Drives and root folders cannot be renamed here.").with_path(path))?;
    fs::symlink_metadata(path).map_err(|e| AppError::from_io(&e, path))?;
    let target = parent.join(new_name);
    if target == path {
        return Ok(target);
    }
    platform::move_no_replace(path, &target).map_err(|e| AppError::from_io(&e, &target))?;
    Ok(target)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemFailure {
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecycleResult {
    pub recycled: Vec<String>,
    pub failed: Vec<ItemFailure>,
}

/// Checks whether `path` may be sent to the Recycle Bin. Only local fixed drives have a
/// Recycle Bin; on network shares and most removable media Windows would delete
/// permanently, which Nexdir does not do.
pub(crate) fn check_recyclable(path: &Path) -> Result<(), String> {
    if path.parent().is_none() {
        return Err("Drives and root folders cannot be deleted.".into());
    }
    if let Err(e) = fs::symlink_metadata(path) {
        return Err(AppError::from_io(&e, path).message);
    }
    match platform::drive_type_of(path) {
        DriveType::Fixed => Ok(()),
        DriveType::Remote => Err("Network locations have no Recycle Bin. Permanent deletion is not supported in this version.".into()),
        DriveType::Removable => Err("This removable drive has no Recycle Bin. Permanent deletion is not supported in this version.".into()),
        _ => Err("This drive has no Recycle Bin. Permanent deletion is not supported in this version.".into()),
    }
}

/// Moves items to the Recycle Bin. Items that cannot be recycled are reported and left
/// untouched.
pub fn recycle_entries(paths: &[PathBuf], owner: Option<isize>) -> AppResult<RecycleResult> {
    let mut failed = Vec::new();
    let mut accepted = Vec::new();
    for path in paths {
        match check_recyclable(path) {
            Ok(()) => accepted.push(path.clone()),
            Err(message) => failed.push(ItemFailure {
                path: path.display().to_string(),
                message,
            }),
        }
    }
    if !accepted.is_empty() {
        // The shell operation needs its own single-threaded COM apartment.
        let batch = accepted.clone();
        std::thread::spawn(move || platform::recycle(&batch, owner))
            .join()
            .map_err(|_| AppError::new(ErrorCode::Io, "The Recycle Bin operation failed unexpectedly."))?
            .map_err(|e| AppError::from_io(&e, &accepted[0]))?;
    }
    let mut recycled = Vec::new();
    for path in accepted {
        if fs::symlink_metadata(&path).is_err() {
            recycled.push(path.display().to_string());
        } else {
            failed.push(ItemFailure {
                path: path.display().to_string(),
                message: "Not moved to the Recycle Bin. It may be in use, or the operation was cancelled.".into(),
            });
        }
    }
    Ok(RecycleResult { recycled, failed })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_folders_without_touching_existing_items() {
        let dir = tempfile::tempdir().unwrap();
        let created = create_folder(dir.path(), "New folder").unwrap();
        assert!(created.is_dir());
        fs::write(created.join("keep.txt"), "x").unwrap();
        let err = create_folder(dir.path(), "New folder").unwrap_err();
        assert_eq!(err.code, ErrorCode::AlreadyExists);
        assert!(created.join("keep.txt").exists());
        assert_eq!(create_folder(dir.path(), "bad:name").unwrap_err().code, ErrorCode::InvalidName);
        assert!(create_folder(dir.path(), "Ordner äöü 文件夹").unwrap().is_dir());
    }

    #[test]
    fn rename_never_overwrites() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.txt");
        let b = dir.path().join("b.txt");
        fs::write(&a, "A").unwrap();
        fs::write(&b, "B").unwrap();
        assert_eq!(rename_entry(&a, "b.txt").unwrap_err().code, ErrorCode::AlreadyExists);
        assert_eq!(fs::read_to_string(&b).unwrap(), "B");

        let renamed = rename_entry(&a, "c.txt").unwrap();
        assert_eq!(fs::read_to_string(renamed).unwrap(), "A");
        assert!(!a.exists());
    }

    #[test]
    fn rename_allows_case_only_change() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("name.txt");
        fs::write(&a, "x").unwrap();
        rename_entry(&a, "NAME.txt").unwrap();
        let names: Vec<_> = fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(names, vec![std::ffi::OsString::from("NAME.txt")]);
    }

    #[test]
    fn rename_rejects_invalid_names_and_roots() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.txt");
        fs::write(&a, "x").unwrap();
        assert_eq!(rename_entry(&a, "..\\escape.txt").unwrap_err().code, ErrorCode::InvalidName);
        assert_eq!(rename_entry(Path::new("C:\\"), "x").unwrap_err().code, ErrorCode::Unsupported);
    }

    #[test]
    fn recycle_refuses_roots_and_missing_items() {
        let dir = tempfile::tempdir().unwrap();
        let result = recycle_entries(&[PathBuf::from("C:\\"), dir.path().join("missing.txt")], None).unwrap();
        assert!(result.recycled.is_empty());
        assert_eq!(result.failed.len(), 2);
    }

    #[test]
    #[ignore = "uses the real Windows Recycle Bin; run manually with --ignored"]
    fn recycles_a_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("nexdir-recycle-test.txt");
        fs::write(&f, "x").unwrap();
        let result = recycle_entries(std::slice::from_ref(&f), None).unwrap();
        assert_eq!(result.recycled.len(), 1, "{:?}", result.failed);
        assert!(!f.exists());
    }
}
