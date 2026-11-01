//! File search: a live recursive walk with filters, and an optional SQLite FTS5 index
//! (`index`) for fast searches across user-selected folders.

pub mod index;

use crate::analytics::category_of;
use crate::filesystem::{entry_from_metadata, EntryKind, FileEntry};
use crate::tasks::TaskContext;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

const DEFAULT_LIMIT: usize = 5_000;
const MAX_LIMIT: usize = 50_000;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SearchKind {
    #[default]
    Any,
    Files,
    Folders,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SearchQuery {
    pub root: String,
    pub recursive: bool,
    pub include_hidden: bool,
    /// Words that must all occur in the name, or a wildcard pattern (`*.log`, `IMG_????`).
    pub text: Option<String>,
    pub extensions: Vec<String>,
    /// Categories as returned by `analytics::category_of` (image, video, audio, …).
    pub categories: Vec<String>,
    pub min_size: Option<u64>,
    pub max_size: Option<u64>,
    /// Milliseconds since the Unix epoch.
    pub modified_after: Option<i64>,
    pub modified_before: Option<i64>,
    pub kind: SearchKind,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub entries: Vec<FileEntry>,
    pub scanned: u64,
    pub truncated: bool,
    pub complete: bool,
    /// "live" for a folder walk, "index" for results from the search index.
    pub source: &'static str,
}

/// Case-insensitive wildcard match supporting `*` and `?`.
pub fn glob_match(pattern: &str, name: &str) -> bool {
    let p: Vec<char> = pattern.to_lowercase().chars().collect();
    let n: Vec<char> = name.to_lowercase().chars().collect();
    let (mut pi, mut ni) = (0, 0);
    let (mut star, mut mark) = (None, 0);
    while ni < n.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == n[ni]) {
            pi += 1;
            ni += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ni;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ni = mark;
        } else {
            return false;
        }
    }
    p[pi..].iter().all(|&c| c == '*')
}

pub struct Matcher<'a> {
    q: &'a SearchQuery,
    terms: Vec<String>,
    glob: Option<String>,
    extensions: Vec<String>,
}

impl<'a> Matcher<'a> {
    pub fn new(q: &'a SearchQuery) -> Self {
        let text = q.text.as_deref().unwrap_or("").trim();
        let glob = (text.contains('*') || text.contains('?')).then(|| text.to_string());
        let terms = if glob.is_some() {
            Vec::new()
        } else {
            text.split_whitespace().map(|t| t.to_lowercase()).collect()
        };
        let extensions = q.extensions.iter().map(|e| e.trim_start_matches('.').to_lowercase()).collect();
        Self {
            q,
            terms,
            glob,
            extensions,
        }
    }

    pub fn matches_name(&self, name: &str) -> bool {
        if let Some(g) = &self.glob {
            return glob_match(g, name);
        }
        let lower = name.to_lowercase();
        self.terms.iter().all(|t| lower.contains(t.as_str()))
    }

    /// Checks everything except the name.
    pub fn matches_attributes(&self, is_dir: bool, ext: &str, size: u64, modified: Option<i64>) -> bool {
        let q = self.q;
        match q.kind {
            SearchKind::Files if is_dir => return false,
            SearchKind::Folders if !is_dir => return false,
            _ => {}
        }
        let type_filtered = !self.extensions.is_empty() || !q.categories.is_empty();
        if type_filtered {
            if is_dir {
                return false;
            }
            let by_ext = self.extensions.iter().any(|e| e == ext);
            let by_cat = q.categories.iter().any(|c| c == category_of(ext));
            if !by_ext && !by_cat {
                return false;
            }
        }
        if (q.min_size.is_some() || q.max_size.is_some()) && is_dir {
            return false;
        }
        if q.min_size.is_some_and(|m| size < m) || q.max_size.is_some_and(|m| size > m) {
            return false;
        }
        if q.modified_after.is_some() || q.modified_before.is_some() {
            let Some(t) = modified else { return false };
            if q.modified_after.is_some_and(|a| t < a) || q.modified_before.is_some_and(|b| t >= b) {
                return false;
            }
        }
        true
    }
}

fn millis(m: &fs::Metadata) -> Option<i64> {
    m.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
}

/// Walks `q.root` (never following links) and returns matching entries.
pub fn search(root: &Path, q: &SearchQuery, ctx: &mut TaskContext) -> Result<SearchResult, String> {
    if !fs::metadata(root).map(|m| m.is_dir()).unwrap_or(false) {
        return Err("The search location is not an accessible folder.".into());
    }
    let limit = q.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
    let matcher = Matcher::new(q);
    ctx.set_phase("Searching", None);
    let mut entries = Vec::new();
    let mut scanned = 0u64;
    let mut truncated = false;
    let mut stack = vec![root.to_path_buf()];
    'walk: while let Some(dir) = stack.pop() {
        if ctx.cancelled() {
            break;
        }
        let Ok(reader) = fs::read_dir(&dir) else { continue };
        ctx.progress().current = Some(dir.display().to_string());
        for item in reader.flatten() {
            let Ok(meta) = item.metadata() else { continue };
            scanned += 1;
            ctx.progress().items = scanned;
            ctx.tick();
            let path = item.path();
            let entry = entry_from_metadata(&path, &meta);
            if (entry.hidden || entry.system) && !q.include_hidden {
                continue;
            }
            if q.recursive && entry.kind == EntryKind::Dir {
                stack.push(path.clone());
            }
            let ext = entry.extension.clone().unwrap_or_default();
            if matcher.matches_name(&entry.name) && matcher.matches_attributes(entry.kind.is_navigable(), &ext, meta.len(), millis(&meta)) {
                entries.push(entry);
                ctx.progress().done = entries.len() as u64;
                if entries.len() >= limit {
                    truncated = true;
                    break 'walk;
                }
            }
        }
    }
    Ok(SearchResult {
        entries,
        scanned,
        truncated,
        complete: !ctx.cancelled() && !truncated,
        source: "live",
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tasks::{testing::run_now, TaskKind};

    #[test]
    fn wildcards() {
        assert!(glob_match("*.LOG", "server.log"));
        assert!(glob_match("img_????.jpg", "IMG_0042.JPG"));
        assert!(!glob_match("img_????.jpg", "IMG_42.JPG"));
        assert!(glob_match("*report*", "Q3 Report final.pdf"));
        assert!(!glob_match("*.log", "log.txt"));
        assert!(glob_match("*", ""));
    }

    fn tree() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let r = dir.path();
        fs::create_dir_all(r.join("Projects/Report Drafts")).unwrap();
        fs::write(r.join("Quarterly Report.pdf"), vec![0u8; 2000]).unwrap();
        fs::write(r.join("Projects/report-2025.docx"), vec![0u8; 50]).unwrap();
        fs::write(r.join("Projects/holiday.mp4"), vec![0u8; 5000]).unwrap();
        fs::write(r.join("Projects/Report Drafts/notes.txt"), "x").unwrap();
        dir
    }

    fn run(q: SearchQuery) -> Vec<String> {
        let root = std::path::PathBuf::from(&q.root);
        let (r, _) = run_now(TaskKind::Search, |ctx| search(&root, &q, ctx).unwrap());
        let mut names: Vec<String> = r.entries.into_iter().map(|e| e.name).collect();
        names.sort();
        names
    }

    #[test]
    fn filters_by_name_type_size_and_kind() {
        let dir = tree();
        let root = dir.path().display().to_string();
        let base = SearchQuery {
            root: root.clone(),
            recursive: true,
            ..Default::default()
        };

        assert_eq!(
            run(SearchQuery {
                text: Some("report".into()),
                ..base.clone()
            }),
            vec!["Quarterly Report.pdf", "Report Drafts", "report-2025.docx"]
        );
        assert_eq!(
            run(SearchQuery {
                text: Some("report".into()),
                kind: SearchKind::Folders,
                ..base.clone()
            }),
            vec!["Report Drafts"]
        );
        assert_eq!(
            run(SearchQuery {
                categories: vec!["video".into()],
                ..base.clone()
            }),
            vec!["holiday.mp4"]
        );
        assert_eq!(
            run(SearchQuery {
                extensions: vec![".PDF".into()],
                ..base.clone()
            }),
            vec!["Quarterly Report.pdf"]
        );
        assert_eq!(
            run(SearchQuery {
                min_size: Some(1000),
                ..base.clone()
            }),
            vec!["Quarterly Report.pdf", "holiday.mp4"]
        );
        assert_eq!(
            run(SearchQuery {
                text: Some("*.txt".into()),
                ..base.clone()
            }),
            vec!["notes.txt"]
        );
        assert_eq!(
            run(SearchQuery {
                text: Some("report".into()),
                recursive: false,
                ..base.clone()
            }),
            vec!["Quarterly Report.pdf"]
        );
        let future = i64::MAX / 2;
        assert!(run(SearchQuery {
            modified_after: Some(future),
            ..base.clone()
        })
        .is_empty());
        assert_eq!(
            run(SearchQuery {
                text: Some("quarterly pdf".into()),
                ..base
            }),
            vec!["Quarterly Report.pdf"]
        );
    }

    #[test]
    fn stops_at_the_limit() {
        let dir = tree();
        let q = SearchQuery {
            root: dir.path().display().to_string(),
            recursive: true,
            limit: Some(2),
            ..Default::default()
        };
        let (r, _) = run_now(TaskKind::Search, |ctx| search(dir.path(), &q, ctx).unwrap());
        assert_eq!(r.entries.len(), 2);
        assert!(r.truncated);
    }
}
