//! Read-only filesystem access: directory listings, drives and well-known folders.

pub mod drives;
pub mod preview;

use crate::error::{AppError, AppResult, ErrorCode};
use crate::platform;
use serde::Serialize;
use std::fs;
use std::os::windows::fs::{FileTypeExt, MetadataExt};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const FILE_ATTRIBUTE_READONLY: u32 = 0x1;
const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
const FILE_ATTRIBUTE_SYSTEM: u32 = 0x4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum EntryKind {
    File,
    Dir,
    /// Symbolic link to a file.
    FileLink,
    /// Symbolic link to a directory.
    DirLink,
    /// NTFS junction (mount point reparse tag).
    Junction,
}

impl EntryKind {
    pub fn is_navigable(self) -> bool {
        matches!(self, EntryKind::Dir | EntryKind::DirLink | EntryKind::Junction)
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub kind: EntryKind,
    /// Size in bytes for files; `None` for folders and links.
    pub size: Option<u64>,
    /// Milliseconds since the Unix epoch.
    pub modified: Option<i64>,
    pub created: Option<i64>,
    pub extension: Option<String>,
    pub hidden: bool,
    pub system: bool,
    pub readonly: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirListing {
    pub path: String,
    pub parent: Option<String>,
    pub entries: Vec<FileEntry>,
    /// Entries that could not be read (e.g. permission denied); they are left out.
    pub unreadable: u32,
    /// Hidden or system entries left out because `show_hidden` is false.
    pub hidden_count: u32,
}

fn to_millis(t: std::io::Result<SystemTime>) -> Option<i64> {
    let t = t.ok()?;
    Some(match t.duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_millis() as i64,
        Err(e) => -(e.duration().as_millis() as i64),
    })
}

fn entry_kind(path: &Path, ft: fs::FileType) -> EntryKind {
    if ft.is_symlink_dir() {
        // std reports junctions as directory symlinks; the reparse tag tells them apart.
        if platform::reparse_tag(path) == Some(platform::IO_REPARSE_TAG_MOUNT_POINT) {
            EntryKind::Junction
        } else {
            EntryKind::DirLink
        }
    } else if ft.is_symlink_file() || ft.is_symlink() {
        EntryKind::FileLink
    } else if ft.is_dir() {
        EntryKind::Dir
    } else {
        EntryKind::File
    }
}

/// Builds a `FileEntry` from link-level metadata (links are never followed).
pub fn entry_from_metadata(path: &Path, meta: &fs::Metadata) -> FileEntry {
    let kind = entry_kind(path, meta.file_type());
    let attrs = meta.file_attributes();
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string());
    let extension = if kind.is_navigable() {
        None
    } else {
        path.extension().map(|e| e.to_string_lossy().to_lowercase())
    };
    FileEntry {
        name,
        path: path.display().to_string(),
        kind,
        size: (kind == EntryKind::File).then_some(meta.len()),
        modified: to_millis(meta.modified()),
        created: to_millis(meta.created()),
        extension,
        hidden: attrs & FILE_ATTRIBUTE_HIDDEN != 0,
        system: attrs & FILE_ATTRIBUTE_SYSTEM != 0,
        readonly: attrs & FILE_ATTRIBUTE_READONLY != 0,
    }
}

/// Lists a directory. Individual unreadable entries are counted and skipped rather
/// than failing the whole listing.
pub fn list_dir(dir: &Path, show_hidden: bool) -> AppResult<DirListing> {
    let meta = fs::metadata(dir).map_err(|e| AppError::from_io(&e, dir))?;
    if !meta.is_dir() {
        return Err(AppError::new(ErrorCode::NotADirectory, "The path is not a folder.").with_path(dir));
    }
    let reader = fs::read_dir(dir).map_err(|e| AppError::from_io(&e, dir))?;
    let mut entries = Vec::new();
    let (mut unreadable, mut hidden_count) = (0u32, 0u32);
    for item in reader {
        let Ok(item) = item else {
            unreadable += 1;
            continue;
        };
        // On Windows DirEntry::metadata comes from the directory enumeration itself
        // and does not follow links, so this is cheap and link-safe.
        let Ok(meta) = item.metadata() else {
            unreadable += 1;
            continue;
        };
        let entry = entry_from_metadata(&item.path(), &meta);
        if !show_hidden && (entry.hidden || entry.system) {
            hidden_count += 1;
            continue;
        }
        entries.push(entry);
    }
    Ok(DirListing {
        path: dir.display().to_string(),
        parent: dir.parent().map(|p| p.display().to_string()),
        entries,
        unreadable,
        hidden_count,
    })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KnownFolder {
    pub id: &'static str,
    pub label: &'static str,
    pub path: String,
}

pub fn known_folders() -> Vec<KnownFolder> {
    let candidates: [(&'static str, &'static str, Option<PathBuf>); 7] = [
        ("home", "Home folder", dirs::home_dir()),
        ("desktop", "Desktop", dirs::desktop_dir()),
        ("downloads", "Downloads", dirs::download_dir()),
        ("documents", "Documents", dirs::document_dir()),
        ("pictures", "Pictures", dirs::picture_dir()),
        ("music", "Music", dirs::audio_dir()),
        ("videos", "Videos", dirs::video_dir()),
    ];
    candidates
        .into_iter()
        .filter_map(|(id, label, path)| {
            path.map(|p| KnownFolder {
                id,
                label,
                path: p.display().to_string(),
            })
        })
        .collect()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedPath {
    /// Folder to navigate to.
    pub path: String,
    /// Name of a file inside `path` to select, when the input pointed at a file.
    pub select: Option<String>,
}

/// Expands `%VAR%` references using the current environment.
pub fn expand_env_vars(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(start) = rest.find('%') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        match after.find('%') {
            Some(end) if end > 0 => {
                let name = &after[..end];
                match std::env::var(name) {
                    Ok(value) => out.push_str(&value),
                    Err(_) => {
                        out.push('%');
                        out.push_str(name);
                        out.push('%');
                    }
                }
                rest = &after[end + 1..];
            }
            _ => {
                out.push('%');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Interprets text typed into the address bar.
pub fn resolve_input(input: &str) -> AppResult<ResolvedPath> {
    let mut text = expand_env_vars(input.trim().trim_matches('"'));
    if text == "~" || text.starts_with("~\\") || text.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            text = format!("{}{}", home.display(), &text[1..]);
        }
    }
    let mut text = text.replace('/', "\\");
    // "C:" alone means the drive root, not the current directory on that drive.
    if text.len() == 2 && text.ends_with(':') {
        text.push('\\');
    }
    let path = crate::security::validate_path(&text)?;
    let meta = fs::metadata(&path).map_err(|e| AppError::from_io(&e, &path))?;
    if meta.is_dir() {
        return Ok(ResolvedPath {
            path: path.display().to_string(),
            select: None,
        });
    }
    let parent = path
        .parent()
        .ok_or_else(|| AppError::new(ErrorCode::InvalidPath, "Invalid path.").with_path(&path))?;
    Ok(ResolvedPath {
        path: parent.display().to_string(),
        select: path.file_name().map(|n| n.to_string_lossy().into_owned()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_files_folders_and_unicode_names() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("Sub folder")).unwrap();
        fs::write(dir.path().join("Résumé 日本.TXT"), "hello").unwrap();
        fs::write(dir.path().join("emoji 😀.md"), "").unwrap();

        let listing = list_dir(dir.path(), false).unwrap();
        assert_eq!(listing.entries.len(), 3);
        let file = listing.entries.iter().find(|e| e.name.starts_with("Résumé")).unwrap();
        assert_eq!(file.kind, EntryKind::File);
        assert_eq!(file.size, Some(5));
        assert_eq!(file.extension.as_deref(), Some("txt"));
        assert!(file.modified.is_some());
        let sub = listing.entries.iter().find(|e| e.name == "Sub folder").unwrap();
        assert_eq!(sub.kind, EntryKind::Dir);
        assert_eq!(sub.size, None);
    }

    #[test]
    fn hides_hidden_files_unless_requested() {
        let dir = tempfile::tempdir().unwrap();
        let hidden = dir.path().join("secret.txt");
        fs::write(&hidden, "x").unwrap();
        let status = std::process::Command::new("attrib").arg("+h").arg(&hidden).status().unwrap();
        assert!(status.success());
        fs::write(dir.path().join("visible.txt"), "x").unwrap();

        let listing = list_dir(dir.path(), false).unwrap();
        assert_eq!(listing.entries.len(), 1);
        assert_eq!(listing.hidden_count, 1);
        let listing = list_dir(dir.path(), true).unwrap();
        assert_eq!(listing.entries.len(), 2);
        assert!(listing.entries.iter().any(|e| e.hidden));
    }

    #[test]
    fn listing_errors_are_typed() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("missing");
        assert_eq!(list_dir(&missing, false).unwrap_err().code, ErrorCode::NotFound);
        let file = dir.path().join("f.txt");
        fs::write(&file, "x").unwrap();
        assert_eq!(list_dir(&file, false).unwrap_err().code, ErrorCode::NotADirectory);
    }

    #[test]
    fn detects_junctions_without_following_them() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("inner.txt"), "x").unwrap();
        let junction = dir.path().join("junction");
        // mklink /J needs no special privilege, unlike symbolic links.
        let status = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&junction)
            .arg(&target)
            .output()
            .unwrap();
        assert!(status.status.success(), "mklink /J failed: {:?}", status);

        let listing = list_dir(dir.path(), true).unwrap();
        let j = listing.entries.iter().find(|e| e.name == "junction").unwrap();
        assert_eq!(j.kind, EntryKind::Junction);
        assert!(j.kind.is_navigable());
        // Navigating into a junction lists the target's content.
        assert_eq!(list_dir(&junction, true).unwrap().entries.len(), 1);
    }

    #[test]
    fn expands_environment_variables() {
        std::env::set_var("NEXDIR_TEST_VAR", r"C:\Data");
        assert_eq!(expand_env_vars(r"%NEXDIR_TEST_VAR%\x"), r"C:\Data\x");
        assert_eq!(expand_env_vars("%NOT_DEFINED_NEXDIR%"), "%NOT_DEFINED_NEXDIR%");
        assert_eq!(expand_env_vars("100% sure"), "100% sure");
    }

    #[test]
    fn resolves_files_to_parent_and_selection() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("pick me.txt");
        fs::write(&file, "x").unwrap();
        let r = resolve_input(&format!("\"{}\"", file.display())).unwrap();
        assert_eq!(r.path, dir.path().display().to_string());
        assert_eq!(r.select.as_deref(), Some("pick me.txt"));
        let r = resolve_input(&dir.path().display().to_string().replace('\\', "/")).unwrap();
        assert_eq!(r.path, dir.path().display().to_string());
        assert!(resolve_input("relative").is_err());
    }
}
