//! Organisation rules: "files in Downloads ending in .pdf → move to Documents\PDF".
//!
//! A rule never acts on its own. `plan` is a dry run that lists what would happen; `apply`
//! only transfers the items the user confirmed from that plan, re-checks that each one
//! still matches, and hands them to the regular copy/move queue (with conflict policy,
//! progress, cancellation and a per-item report).

use super::transfer::{ConflictPolicy, TransferKind, TransferRequest};
use crate::error::{AppError, AppResult, ErrorCode};
use crate::security::{is_same_or_inside, validate_path};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

const MAX_PLAN: usize = 10_000;
const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
const FILE_ATTRIBUTE_SYSTEM: u32 = 0x4;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    pub id: String,
    pub name: String,
    pub source: String,
    #[serde(default)]
    pub include_subfolders: bool,
    /// Extensions without dot; empty = any.
    #[serde(default)]
    pub extensions: Vec<String>,
    #[serde(default)]
    pub name_contains: Option<String>,
    #[serde(default)]
    pub older_than_days: Option<u32>,
    #[serde(default)]
    pub min_size: Option<u64>,
    pub action: TransferKind,
    pub destination: String,
    pub policy: ConflictPolicy,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanItem {
    pub source: String,
    pub target: String,
    pub size: u64,
    /// An item with the target name already exists; the rule's conflict policy applies.
    pub conflict: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub items: Vec<PlanItem>,
    pub scanned: u64,
    pub truncated: bool,
    pub destination_exists: bool,
}

fn check_rule(rule: &Rule) -> AppResult<(PathBuf, PathBuf)> {
    let source = validate_path(&rule.source)?;
    let destination = validate_path(&rule.destination)?;
    if !fs::metadata(&source).map(|m| m.is_dir()).unwrap_or(false) {
        return Err(AppError::new(ErrorCode::NotFound, "The source folder does not exist.").with_path(&source));
    }
    if is_same_or_inside(&source, &destination) {
        return Err(AppError::new(
            ErrorCode::InvalidPath,
            "The destination cannot be the source folder or one of its parent folders.",
        ));
    }
    Ok((source, destination))
}

fn matches(rule: &Rule, path: &Path, meta: &fs::Metadata, now: SystemTime) -> bool {
    let name = path.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
    if !rule.extensions.is_empty() {
        let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        if !rule.extensions.iter().any(|e| e.trim_start_matches('.').eq_ignore_ascii_case(&ext)) {
            return false;
        }
    }
    if let Some(text) = rule.name_contains.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
        if !name.contains(&text.to_lowercase()) {
            return false;
        }
    }
    if let Some(days) = rule.older_than_days {
        let age = meta
            .modified()
            .ok()
            .and_then(|m| now.duration_since(m).ok())
            .map(|d| d.as_secs() / 86_400);
        if age.is_none_or(|a| a < days as u64) {
            return false;
        }
    }
    if rule.min_size.is_some_and(|min| meta.len() < min) {
        return false;
    }
    true
}

/// Dry run: lists the files the rule would transfer. Changes nothing.
pub fn plan(rule: &Rule) -> AppResult<Plan> {
    let (source, destination) = check_rule(rule)?;
    let now = SystemTime::now();
    let mut items = Vec::new();
    let mut scanned = 0u64;
    let mut truncated = false;
    let mut stack = vec![source];
    'walk: while let Some(dir) = stack.pop() {
        let Ok(reader) = fs::read_dir(&dir) else { continue };
        for entry in reader.flatten() {
            let Ok(meta) = entry.metadata() else { continue };
            if meta.file_type().is_symlink() || meta.file_attributes() & (FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM) != 0 {
                continue;
            }
            let path = entry.path();
            if meta.is_dir() {
                // Never descend into the destination (it may live inside the source).
                if rule.include_subfolders && !is_same_or_inside(&path, &destination) {
                    stack.push(path);
                }
                continue;
            }
            scanned += 1;
            if !matches(rule, &path, &meta, now) {
                continue;
            }
            let target = destination.join(entry.file_name());
            items.push(PlanItem {
                conflict: fs::symlink_metadata(&target).is_ok(),
                source: path.display().to_string(),
                target: target.display().to_string(),
                size: meta.len(),
            });
            if items.len() >= MAX_PLAN {
                truncated = true;
                break 'walk;
            }
        }
    }
    items.sort_by_key(|a| a.source.to_lowercase());
    Ok(Plan {
        items,
        scanned,
        truncated,
        destination_exists: destination.is_dir(),
    })
}

/// Builds the transfer for the confirmed items. Items that no longer match the rule (or
/// were not part of the plan) are dropped. Creates the destination folder if needed.
pub fn prepare(rule: &Rule, approved: &[String]) -> AppResult<TransferRequest> {
    let current = plan(rule)?;
    let approved: HashSet<String> = approved.iter().map(|p| p.to_lowercase()).collect();
    let sources: Vec<PathBuf> = current
        .items
        .into_iter()
        .filter(|i| approved.contains(&i.source.to_lowercase()))
        .map(|i| PathBuf::from(i.source))
        .collect();
    if sources.is_empty() {
        return Err(AppError::new(
            ErrorCode::NotFound,
            "None of the selected files match the rule any more.",
        ));
    }
    let destination = validate_path(&rule.destination)?;
    fs::create_dir_all(&destination).map_err(|e| AppError::from_io(&e, &destination))?;
    Ok(TransferRequest {
        kind: rule.action,
        sources,
        destination,
        policy: rule.policy,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(src: &Path, dst: &Path) -> Rule {
        Rule {
            id: "r1".into(),
            name: "PDFs".into(),
            source: src.display().to_string(),
            include_subfolders: false,
            extensions: vec!["pdf".into()],
            name_contains: None,
            older_than_days: None,
            min_size: None,
            action: TransferKind::Move,
            destination: dst.display().to_string(),
            policy: ConflictPolicy::KeepBoth,
        }
    }

    #[test]
    fn plans_without_changing_anything() {
        let dir = tempfile::tempdir().unwrap();
        let (src, dst) = (dir.path().join("Downloads"), dir.path().join("Documents/PDF"));
        fs::create_dir_all(src.join("nested")).unwrap();
        fs::write(src.join("a.PDF"), "a").unwrap();
        fs::write(src.join("b.txt"), "b").unwrap();
        fs::write(src.join("nested/c.pdf"), "c").unwrap();

        let p = plan(&rule(&src, &dst)).unwrap();
        assert_eq!(p.items.len(), 1);
        assert!(p.items[0].source.ends_with("a.PDF"));
        assert!(!p.destination_exists);
        assert!(!dst.exists(), "a dry run must not create anything");
        assert!(src.join("a.PDF").exists());

        let deep = Rule {
            include_subfolders: true,
            ..rule(&src, &dst)
        };
        assert_eq!(plan(&deep).unwrap().items.len(), 2);
    }

    #[test]
    fn applies_only_confirmed_and_still_matching_items() {
        let dir = tempfile::tempdir().unwrap();
        let (src, dst) = (dir.path().join("in"), dir.path().join("out"));
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("keep.pdf"), "1").unwrap();
        fs::write(src.join("go.pdf"), "2").unwrap();
        let r = rule(&src, &dst);
        let go = src.join("go.pdf").display().to_string();
        let foreign = dir.path().join("elsewhere.pdf").display().to_string();
        let req = prepare(&r, &[go.clone(), foreign]).unwrap();
        assert_eq!(req.sources, vec![PathBuf::from(&go)]);
        assert_eq!(req.kind, TransferKind::Move);
        assert!(dst.is_dir(), "destination is created on apply");
        assert!(prepare(&r, &[]).is_err());
    }

    #[test]
    fn conditions_and_conflicts() {
        let dir = tempfile::tempdir().unwrap();
        let (src, dst) = (dir.path().join("in"), dir.path().join("out"));
        fs::create_dir_all(&src).unwrap();
        fs::create_dir_all(&dst).unwrap();
        fs::write(src.join("Invoice 1.pdf"), vec![0u8; 100]).unwrap();
        fs::write(src.join("photo.pdf"), vec![0u8; 5]).unwrap();
        fs::write(dst.join("Invoice 1.pdf"), "existing").unwrap();
        let r = Rule {
            name_contains: Some("invoice".into()),
            min_size: Some(50),
            ..rule(&src, &dst)
        };
        let p = plan(&r).unwrap();
        assert_eq!(p.items.len(), 1);
        assert!(p.items[0].conflict);
        let old = Rule {
            older_than_days: Some(3650),
            ..rule(&src, &dst)
        };
        assert!(plan(&old).unwrap().items.is_empty());
    }

    #[test]
    fn rejects_dangerous_destinations() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("in");
        fs::create_dir_all(&src).unwrap();
        assert!(plan(&rule(&src, &src)).is_err());
        assert!(plan(&rule(&src, dir.path())).is_err(), "parent of the source");
        // A destination inside the source is allowed but never scanned.
        fs::create_dir_all(src.join("sorted")).unwrap();
        fs::write(src.join("sorted/x.pdf"), "x").unwrap();
        let r = Rule {
            include_subfolders: true,
            ..rule(&src, &src.join("sorted"))
        };
        assert!(plan(&r).unwrap().items.is_empty());
    }
}
