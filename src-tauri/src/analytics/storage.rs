//! Storage analysis: where does the space go below a folder or drive?
//!
//! The walk is iterative (no recursion limits), never follows links or junctions, keeps
//! going past unreadable folders and stops promptly on cancellation, returning what it
//! has measured so far.

use super::category_of;
use crate::platform;
use crate::tasks::TaskContext;
use serde::Serialize;
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

const TOP_FILES: usize = 100;
const TOP_FOLDERS: usize = 50;
const MAX_CHILDREN: usize = 300;
const TOP_EXTENSIONS: usize = 20;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SizeNode {
    pub name: String,
    pub path: String,
    pub bytes: u64,
    pub files: u64,
    pub is_dir: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LargeFile {
    pub name: String,
    pub path: String,
    pub bytes: u64,
    pub modified: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeShare {
    pub key: String,
    pub bytes: u64,
    pub files: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeUsage {
    pub total_bytes: u64,
    pub free_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageReport {
    pub root: String,
    pub total_bytes: u64,
    pub file_count: u64,
    pub folder_count: u64,
    pub unreadable: u64,
    pub skipped_links: u64,
    /// Immediate children of the root, largest first.
    pub children: Vec<SizeNode>,
    /// Number of children not listed in `children`, and their combined size.
    pub other_children: u64,
    pub other_children_bytes: u64,
    pub largest_files: Vec<LargeFile>,
    /// Largest folders at any depth (excluding the root).
    pub largest_folders: Vec<SizeNode>,
    pub categories: Vec<TypeShare>,
    pub extensions: Vec<TypeShare>,
    pub volume: Option<VolumeUsage>,
    /// False when the scan was cancelled before finishing.
    pub complete: bool,
}

struct Dir {
    name: String,
    parent: Option<usize>,
    bytes: u64,
    files: u64,
}

fn path_of(dirs: &[Dir], root: &Path, mut idx: usize) -> PathBuf {
    let mut parts = Vec::new();
    while let Some(parent) = dirs[idx].parent {
        parts.push(dirs[idx].name.as_str());
        idx = parent;
    }
    let mut path = root.to_path_buf();
    for part in parts.iter().rev() {
        path.push(part);
    }
    path
}

pub fn analyze(root: &Path, ctx: &mut TaskContext) -> Result<StorageReport, String> {
    let meta = fs::metadata(root).map_err(|e| crate::error::AppError::from_io(&e, root).message)?;
    if !meta.is_dir() {
        return Err("Storage analysis needs a folder or drive.".into());
    }
    ctx.set_phase("Scanning", None);

    let mut dirs = vec![Dir {
        name: String::new(),
        parent: None,
        bytes: 0,
        files: 0,
    }];
    let mut root_files: Vec<SizeNode> = Vec::new();
    let mut largest: BinaryHeap<Reverse<(u64, String, Option<i64>)>> = BinaryHeap::new();
    let mut categories: HashMap<&'static str, (u64, u64)> = HashMap::new();
    let mut extensions: HashMap<String, (u64, u64)> = HashMap::new();
    let (mut unreadable, mut skipped_links) = (0u64, 0u64);
    let mut stack = vec![(0usize, root.to_path_buf())];

    while let Some((idx, dir_path)) = stack.pop() {
        if ctx.cancelled() {
            break;
        }
        let reader = match fs::read_dir(&dir_path) {
            Ok(r) => r,
            Err(_) => {
                unreadable += 1;
                continue;
            }
        };
        for entry in reader {
            let Ok(entry) = entry else {
                unreadable += 1;
                continue;
            };
            let Ok(m) = entry.metadata() else {
                unreadable += 1;
                continue;
            };
            let ft = m.file_type();
            if ft.is_symlink() {
                skipped_links += 1;
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            if ft.is_dir() {
                dirs.push(Dir {
                    name,
                    parent: Some(idx),
                    bytes: 0,
                    files: 0,
                });
                stack.push((dirs.len() - 1, entry.path()));
                continue;
            }
            let size = m.len();
            dirs[idx].bytes += size;
            dirs[idx].files += 1;
            let ext = Path::new(&name)
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            let c = categories.entry(category_of(&ext)).or_default();
            c.0 += size;
            c.1 += 1;
            let e = extensions.entry(if ext.is_empty() { "(none)".into() } else { ext }).or_default();
            e.0 += size;
            e.1 += 1;
            if largest.len() < TOP_FILES || largest.peek().is_some_and(|Reverse(min)| size > min.0) {
                let modified = m
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_millis() as i64);
                largest.push(Reverse((size, entry.path().display().to_string(), modified)));
                if largest.len() > TOP_FILES {
                    largest.pop();
                }
            }
            if idx == 0 {
                root_files.push(SizeNode {
                    path: entry.path().display().to_string(),
                    name,
                    bytes: size,
                    files: 1,
                    is_dir: false,
                });
            }
            let p = ctx.progress();
            p.items += 1;
            p.bytes += size;
            ctx.tick();
        }
        ctx.progress().current = Some(dir_path.display().to_string());
        ctx.tick();
    }

    ctx.set_phase("Summarising", None);
    // Children are always pushed after their parent, so a reverse pass aggregates bottom-up.
    for i in (1..dirs.len()).rev() {
        let parent = dirs[i].parent.expect("non-root has a parent");
        dirs[parent].bytes += dirs[i].bytes;
        dirs[parent].files += dirs[i].files;
    }

    let mut children: Vec<SizeNode> = dirs
        .iter()
        .enumerate()
        .filter(|(_, d)| d.parent == Some(0))
        .map(|(i, d)| SizeNode {
            name: d.name.clone(),
            path: path_of(&dirs, root, i).display().to_string(),
            bytes: d.bytes,
            files: d.files,
            is_dir: true,
        })
        .chain(root_files)
        .collect();
    children.sort_by(|a, b| b.bytes.cmp(&a.bytes).then_with(|| a.name.cmp(&b.name)));
    let (other_children, other_children_bytes) = if children.len() > MAX_CHILDREN {
        let rest = children.split_off(MAX_CHILDREN);
        (rest.len() as u64, rest.iter().map(|c| c.bytes).sum())
    } else {
        (0, 0)
    };

    let mut folder_order: Vec<usize> = (1..dirs.len()).collect();
    folder_order.sort_by(|&a, &b| dirs[b].bytes.cmp(&dirs[a].bytes));
    let largest_folders = folder_order
        .into_iter()
        .take(TOP_FOLDERS)
        .map(|i| SizeNode {
            name: dirs[i].name.clone(),
            path: path_of(&dirs, root, i).display().to_string(),
            bytes: dirs[i].bytes,
            files: dirs[i].files,
            is_dir: true,
        })
        .collect();

    let mut largest_files: Vec<LargeFile> = largest
        .into_iter()
        .map(|Reverse((bytes, path, modified))| LargeFile {
            name: Path::new(&path)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            path,
            bytes,
            modified,
        })
        .collect();
    largest_files.sort_by_key(|f| Reverse(f.bytes));

    let mut categories: Vec<TypeShare> = categories
        .into_iter()
        .map(|(k, (bytes, files))| TypeShare {
            key: k.into(),
            bytes,
            files,
        })
        .collect();
    categories.sort_by_key(|c| Reverse(c.bytes));
    let mut extensions: Vec<TypeShare> = extensions
        .into_iter()
        .map(|(key, (bytes, files))| TypeShare { key, bytes, files })
        .collect();
    extensions.sort_by_key(|e| Reverse(e.bytes));
    extensions.truncate(TOP_EXTENSIONS);

    let is_drive_root = root.parent().is_none();
    let volume = if is_drive_root {
        platform::volume_info(&root.display().to_string()).ok().map(|v| VolumeUsage {
            total_bytes: v.total_bytes,
            free_bytes: v.free_bytes,
        })
    } else {
        None
    };

    Ok(StorageReport {
        root: root.display().to_string(),
        total_bytes: dirs[0].bytes,
        file_count: dirs[0].files,
        folder_count: dirs.len() as u64 - 1,
        unreadable,
        skipped_links,
        children,
        other_children,
        other_children_bytes,
        largest_files,
        largest_folders,
        categories,
        extensions,
        volume,
        complete: !ctx.cancelled(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tasks::{testing::run_now, TaskKind};

    #[test]
    fn measures_sizes_types_and_largest_items() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("videos/old")).unwrap();
        fs::create_dir_all(root.join("docs")).unwrap();
        fs::write(root.join("videos/a.mp4"), vec![0u8; 5000]).unwrap();
        fs::write(root.join("videos/old/b.mkv"), vec![0u8; 3000]).unwrap();
        fs::write(root.join("docs/c.pdf"), vec![0u8; 100]).unwrap();
        fs::write(root.join("top.txt"), vec![0u8; 10]).unwrap();
        let out = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(root.join("link"))
            .arg(root.join("videos"))
            .output()
            .unwrap();
        assert!(out.status.success());

        let (report, progress) = run_now(TaskKind::StorageScan, |ctx| analyze(root, ctx).unwrap());
        assert!(report.complete);
        assert_eq!(report.total_bytes, 8110, "junction contents must not be counted twice");
        assert_eq!(report.file_count, 4);
        assert_eq!(report.folder_count, 3);
        assert_eq!(report.skipped_links, 1);
        assert_eq!(progress.items, 4);

        let names: Vec<_> = report.children.iter().map(|c| (c.name.as_str(), c.bytes)).collect();
        assert_eq!(names, vec![("videos", 8000), ("docs", 100), ("top.txt", 10)]);
        assert_eq!(report.largest_files[0].name, "a.mp4");
        assert_eq!(report.largest_folders[0].name, "videos");
        assert!(report.largest_folders[1].path.ends_with("old"));
        assert_eq!(report.categories[0].key, "video");
        assert_eq!(report.categories[0].bytes, 8000);
        assert_eq!(report.extensions[0].key, "mp4");
        assert!(report.volume.is_none());
    }

    #[test]
    fn rejects_files_and_missing_paths() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("f.txt");
        fs::write(&f, "x").unwrap();
        let (r, _) = run_now(TaskKind::StorageScan, |ctx| analyze(&f, ctx));
        assert!(r.is_err());
        let (r, _) = run_now(TaskKind::StorageScan, |ctx| analyze(&dir.path().join("missing"), ctx));
        assert!(r.is_err());
    }
}
