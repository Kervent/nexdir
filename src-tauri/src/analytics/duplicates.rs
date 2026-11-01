//! Duplicate file detection. Nothing is ever deleted here: the result only lists groups
//! of byte-identical files; removing copies is a separate, user-confirmed action.
//!
//! Stages: collect files → group by size → BLAKE3 of the first 64 KiB → full BLAKE3 for
//! remaining candidates. Hashing streams in 1 MiB chunks, so memory use does not grow
//! with file size.

use crate::platform;
use crate::tasks::TaskContext;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Read;
use std::os::windows::fs::MetadataExt;
use std::path::PathBuf;
use std::time::UNIX_EPOCH;

const PARTIAL_BYTES: u64 = 64 * 1024;
const CHUNK: usize = 1024 * 1024;
const MAX_GROUPS: usize = 5_000;
const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
const FILE_ATTRIBUTE_SYSTEM: u32 = 0x4;

#[derive(Debug, Clone)]
pub struct DuplicateOptions {
    pub roots: Vec<PathBuf>,
    pub min_size: u64,
    pub include_hidden: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateFile {
    pub path: String,
    pub name: String,
    pub modified: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateGroup {
    pub size: u64,
    /// BLAKE3 hash (hex) of the full content.
    pub hash: String,
    pub files: Vec<DuplicateFile>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateReport {
    pub groups: Vec<DuplicateGroup>,
    /// Space that could be freed by keeping one file per group.
    pub wasted_bytes: u64,
    pub files_scanned: u64,
    pub bytes_hashed: u64,
    pub unreadable: u64,
    pub truncated: bool,
    pub complete: bool,
}

struct Candidate {
    path: PathBuf,
    modified: Option<i64>,
}

/// Volume and file index of an open file: the same for every hard link of the file.
fn identity(file: &fs::File) -> Option<(u32, u64)> {
    platform::handle_info(file).ok().map(|i| i.identity)
}

/// Hashes up to `limit` bytes of an open file (all of it when `None`). Returns `None` when the
/// file cannot be read or the task is cancelled.
fn hash_file(file: fs::File, limit: Option<u64>, ctx: &mut TaskContext) -> Option<(blake3::Hash, u64)> {
    let mut reader: Box<dyn Read> = match limit {
        Some(n) => Box::new(file.take(n)),
        None => Box::new(file),
    };
    let mut hasher = blake3::Hasher::new();
    let mut buf = vec![0u8; CHUNK];
    let mut read_total = 0u64;
    loop {
        if ctx.cancelled() {
            return None;
        }
        let n = reader.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        read_total += n as u64;
        ctx.progress().bytes += n as u64;
        ctx.tick();
    }
    Some((hasher.finalize(), read_total))
}

/// Splits each group by the hash of its members; keeps sub-groups with 2+ files. Hard links
/// are one file under several names — they take no extra space and removing one name frees
/// nothing — so only the first name of each file stays in a group, and it is read only once.
fn refine(
    groups: Vec<(u64, Vec<Candidate>)>,
    limit: Option<u64>,
    ctx: &mut TaskContext,
    unreadable: &mut u64,
) -> Vec<(u64, blake3::Hash, Vec<Candidate>)> {
    let total = groups.iter().map(|(_, g)| g.len() as u64).sum();
    ctx.set_phase(
        if limit.is_some() {
            "Comparing file beginnings"
        } else {
            "Comparing full contents"
        },
        Some(total),
    );
    let mut out = Vec::new();
    for (size, group) in groups {
        let mut by_hash: HashMap<blake3::Hash, Vec<Candidate>> = HashMap::new();
        let mut seen = HashSet::new();
        for c in group {
            if ctx.cancelled() {
                return out;
            }
            ctx.progress().current = Some(c.path.display().to_string());
            let hashed = match fs::File::open(&c.path) {
                Ok(file) if identity(&file).is_some_and(|id| !seen.insert(id)) => {
                    // Another name of a file already in this group.
                    ctx.progress().done += 1;
                    continue;
                }
                Ok(file) => hash_file(file, limit, ctx),
                Err(_) => None,
            };
            match hashed {
                Some((hash, read)) if limit.is_some() || read == size => by_hash.entry(hash).or_default().push(c),
                Some(_) => *unreadable += 1, // file changed size while hashing
                None if ctx.cancelled() => return out,
                None => *unreadable += 1,
            }
            ctx.progress().done += 1;
            ctx.tick();
        }
        out.extend(by_hash.into_iter().filter(|(_, v)| v.len() > 1).map(|(h, v)| (size, h, v)));
    }
    out
}

pub fn find_duplicates(opts: &DuplicateOptions, ctx: &mut TaskContext) -> Result<DuplicateReport, String> {
    ctx.set_phase("Collecting files", None);
    let mut by_size: HashMap<u64, Vec<Candidate>> = HashMap::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut unreadable = 0u64;
    let mut files_scanned = 0u64;

    for root in &opts.roots {
        let mut stack = vec![root.clone()];
        while let Some(dir) = stack.pop() {
            if ctx.cancelled() {
                break;
            }
            let Ok(reader) = fs::read_dir(&dir) else {
                unreadable += 1;
                continue;
            };
            for entry in reader.flatten() {
                let Ok(m) = entry.metadata() else {
                    unreadable += 1;
                    continue;
                };
                if m.file_type().is_symlink() {
                    continue; // never follow links or junctions
                }
                let hidden = m.file_attributes() & (FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM) != 0;
                if hidden && !opts.include_hidden {
                    continue;
                }
                let path = entry.path();
                if m.is_dir() {
                    stack.push(path);
                    continue;
                }
                // Overlapping roots must not report a file as a duplicate of itself.
                if !seen.insert(path.display().to_string().to_lowercase()) {
                    continue;
                }
                files_scanned += 1;
                ctx.progress().items = files_scanned;
                ctx.tick();
                if m.len() < opts.min_size.max(1) {
                    continue;
                }
                let modified = m
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_millis() as i64);
                by_size.entry(m.len()).or_default().push(Candidate { path, modified });
            }
        }
    }

    let same_size: Vec<(u64, Vec<Candidate>)> = by_size.into_iter().filter(|(_, v)| v.len() > 1).collect();
    let partial = refine(same_size, Some(PARTIAL_BYTES), ctx, &mut unreadable);
    // Files up to 64 KiB were hashed completely in the first pass.
    let (small, large): (Vec<_>, Vec<_>) = partial.into_iter().partition(|(size, _, _)| *size <= PARTIAL_BYTES);
    let mut confirmed = small;
    confirmed.extend(refine(
        large.into_iter().map(|(s, _, v)| (s, v)).collect(),
        None,
        ctx,
        &mut unreadable,
    ));

    confirmed.sort_by_key(|(size, _, files)| std::cmp::Reverse(size * (files.len() as u64 - 1)));
    let truncated = confirmed.len() > MAX_GROUPS;
    confirmed.truncate(MAX_GROUPS);
    let wasted_bytes = confirmed.iter().map(|(size, _, v)| size * (v.len() as u64 - 1)).sum();
    let groups = confirmed
        .into_iter()
        .map(|(size, hash, mut files)| {
            files.sort_by(|a, b| a.modified.cmp(&b.modified).then_with(|| a.path.cmp(&b.path)));
            DuplicateGroup {
                size,
                hash: hash.to_hex().to_string(),
                files: files
                    .into_iter()
                    .map(|c| DuplicateFile {
                        name: c.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
                        path: c.path.display().to_string(),
                        modified: c.modified,
                    })
                    .collect(),
            }
        })
        .collect();

    Ok(DuplicateReport {
        groups,
        wasted_bytes,
        files_scanned,
        bytes_hashed: ctx.progress().bytes,
        unreadable,
        truncated,
        complete: !ctx.cancelled(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tasks::{testing::run_now, TaskKind};
    use std::path::Path;

    fn opts(root: &Path) -> DuplicateOptions {
        DuplicateOptions {
            roots: vec![root.to_path_buf()],
            min_size: 1,
            include_hidden: false,
        }
    }

    #[test]
    fn finds_identical_files_only() {
        let dir = tempfile::tempdir().unwrap();
        let r = dir.path();
        fs::create_dir(r.join("sub")).unwrap();
        fs::write(r.join("a.txt"), "same content").unwrap();
        fs::write(r.join("sub/b.txt"), "same content").unwrap();
        fs::write(r.join("c.txt"), "diff content").unwrap(); // same size, different bytes
        fs::write(r.join("d.txt"), "unique").unwrap();
        fs::write(r.join("empty1"), "").unwrap();
        fs::write(r.join("empty2"), "").unwrap();

        let (report, _) = run_now(TaskKind::Duplicates, |ctx| find_duplicates(&opts(r), ctx).unwrap());
        assert!(report.complete);
        assert_eq!(report.groups.len(), 1, "{:?}", report.groups);
        let g = &report.groups[0];
        assert_eq!(g.size, 12);
        assert_eq!(g.files.len(), 2);
        assert_eq!(report.wasted_bytes, 12);
        assert_eq!(report.files_scanned, 6);
        // Nothing was modified.
        assert!(r.join("a.txt").exists() && r.join("sub/b.txt").exists());
    }

    #[test]
    fn large_files_differing_after_the_first_block_are_not_duplicates() {
        let dir = tempfile::tempdir().unwrap();
        let r = dir.path();
        let mut a = vec![7u8; 200 * 1024];
        let b = a.clone();
        fs::write(r.join("a.bin"), &a).unwrap();
        fs::write(r.join("b.bin"), &b).unwrap();
        a[150 * 1024] = 8;
        fs::write(r.join("c.bin"), &a).unwrap();

        let (report, _) = run_now(TaskKind::Duplicates, |ctx| find_duplicates(&opts(r), ctx).unwrap());
        assert_eq!(report.groups.len(), 1);
        let names: Vec<_> = report.groups[0].files.iter().map(|f| f.name.as_str()).collect();
        assert!(names.contains(&"a.bin") && names.contains(&"b.bin") && !names.contains(&"c.bin"));
        assert_eq!(report.groups[0].hash, blake3::hash(&b).to_hex().to_string());
    }

    #[test]
    fn overlapping_roots_and_junctions_do_not_create_false_duplicates() {
        let dir = tempfile::tempdir().unwrap();
        let r = dir.path();
        fs::create_dir(r.join("inner")).unwrap();
        fs::write(r.join("inner/only.txt"), "one of a kind").unwrap();
        let out = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(r.join("link"))
            .arg(r.join("inner"))
            .output()
            .unwrap();
        assert!(out.status.success());
        let o = DuplicateOptions {
            roots: vec![r.to_path_buf(), r.join("inner")],
            min_size: 1,
            include_hidden: false,
        };
        let (report, _) = run_now(TaskKind::Duplicates, |ctx| find_duplicates(&o, ctx).unwrap());
        assert!(report.groups.is_empty(), "{:?}", report.groups);
    }

    #[test]
    fn respects_minimum_size() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a"), "xy").unwrap();
        fs::write(dir.path().join("b"), "xy").unwrap();
        let o = DuplicateOptions {
            min_size: 10,
            ..opts(dir.path())
        };
        let (report, _) = run_now(TaskKind::Duplicates, |ctx| find_duplicates(&o, ctx).unwrap());
        assert!(report.groups.is_empty());
    }

    #[test]
    fn hard_links_are_not_reported_as_duplicates() {
        let dir = tempfile::tempdir().unwrap();
        let r = dir.path();
        fs::write(r.join("a.txt"), "linked content").unwrap();
        fs::hard_link(r.join("a.txt"), r.join("b.txt")).unwrap();
        let (report, _) = run_now(TaskKind::Duplicates, |ctx| find_duplicates(&opts(r), ctx).unwrap());
        assert!(report.groups.is_empty(), "{:?}", report.groups);

        // A real copy next to the two links: one group with the copy and one of the names.
        fs::write(r.join("c.txt"), "linked content").unwrap();
        let (report, _) = run_now(TaskKind::Duplicates, |ctx| find_duplicates(&opts(r), ctx).unwrap());
        assert_eq!(report.groups.len(), 1);
        assert_eq!(report.groups[0].files.len(), 2);
        assert_eq!(report.wasted_bytes, 14);
    }
}
