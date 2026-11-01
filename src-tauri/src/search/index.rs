//! Optional local search index (SQLite + FTS5).
//!
//! The user decides which folders are indexed, whether text file contents are included,
//! whether the index refreshes on start, and can delete it at any time. The index lives
//! only on this PC (in the app's local data folder) and is never uploaded anywhere.
//! A rebuild writes a new database next to the old one and swaps it in at the end, so
//! searches keep working during a rebuild and an interrupted rebuild leaves the old index.

use super::{Matcher, SearchQuery, SearchResult};
use crate::filesystem::preview::decode_text;
use crate::filesystem::{EntryKind, FileEntry};
use crate::tasks::TaskContext;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Read;
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

const MAX_CONTENT_BYTES: u64 = 1024 * 1024;
const BATCH: usize = 5_000;
const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
const FILE_ATTRIBUTE_SYSTEM: u32 = 0x4;
const CONTENT_EXTENSIONS: &[&str] = &[
    "txt", "md", "log", "csv", "json", "xml", "yaml", "yml", "toml", "ini", "cfg", "html", "htm", "css", "js", "ts", "tsx", "jsx", "rs",
    "py", "java", "c", "h", "cpp", "cs", "go", "rb", "php", "sql", "ps1", "sh", "bat",
];

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct IndexSettings {
    pub folders: Vec<String>,
    /// Also index the text of small plain-text files (≤ 1 MiB).
    pub include_content: bool,
    pub update_on_startup: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexStatus {
    pub exists: bool,
    pub files: u64,
    pub last_built: Option<i64>,
    pub size_bytes: u64,
    pub folders: Vec<String>,
    pub include_content: bool,
}

const SCHEMA: &str = "
    PRAGMA journal_mode = WAL;
    CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
    CREATE TABLE IF NOT EXISTS files (
        id INTEGER PRIMARY KEY,
        path TEXT NOT NULL UNIQUE,
        name TEXT NOT NULL,
        ext TEXT NOT NULL,
        is_dir INTEGER NOT NULL,
        size INTEGER NOT NULL,
        modified INTEGER,
        created INTEGER,
        hidden INTEGER NOT NULL
    );
    CREATE INDEX IF NOT EXISTS files_size ON files(size);
    CREATE INDEX IF NOT EXISTS files_modified ON files(modified);
    CREATE VIRTUAL TABLE IF NOT EXISTS fts USING fts5(name, content, tokenize = 'unicode61 remove_diacritics 2');
";

fn sql_err(e: rusqlite::Error) -> String {
    format!("Search index error: {e}")
}

fn open(path: &Path) -> Result<Connection, String> {
    let conn = Connection::open(path).map_err(sql_err)?;
    conn.execute_batch(SCHEMA).map_err(sql_err)?;
    Ok(conn)
}

fn millis(t: std::io::Result<std::time::SystemTime>) -> Option<i64> {
    t.ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_millis() as i64)
}

fn read_content(path: &Path, ext: &str, size: u64) -> String {
    if size > MAX_CONTENT_BYTES || !CONTENT_EXTENSIONS.contains(&ext) {
        return String::new();
    }
    let mut buf = Vec::new();
    match fs::File::open(path).and_then(|f| f.take(MAX_CONTENT_BYTES).read_to_end(&mut buf)) {
        Ok(_) if !buf.contains(&0) || buf.starts_with(&[0xFF, 0xFE]) => decode_text(&buf),
        _ => String::new(),
    }
}

/// Builds a fresh index for `settings.folders` and atomically replaces `db_path`.
pub fn build(db_path: &Path, settings: &IndexSettings, ctx: &mut TaskContext) -> Result<IndexStatus, String> {
    if let Some(dir) = db_path.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("Cannot create the index folder: {e}"))?;
    }
    let tmp = db_path.with_extension("building");
    let _ = fs::remove_file(&tmp);
    let mut conn = open(&tmp)?;
    ctx.set_phase("Indexing", None);
    let mut count = 0u64;
    {
        let mut tx = conn.transaction().map_err(sql_err)?;
        let mut pending = 0usize;
        for folder in &settings.folders {
            let mut stack = vec![PathBuf::from(folder)];
            while let Some(dir) = stack.pop() {
                if ctx.cancelled() {
                    break;
                }
                let Ok(reader) = fs::read_dir(&dir) else { continue };
                ctx.progress().current = Some(dir.display().to_string());
                for item in reader.flatten() {
                    let Ok(m) = item.metadata() else { continue };
                    if m.file_type().is_symlink() {
                        continue;
                    }
                    let path = item.path();
                    let is_dir = m.is_dir();
                    if is_dir {
                        stack.push(path.clone());
                    }
                    let name = item.file_name().to_string_lossy().into_owned();
                    let ext = if is_dir {
                        String::new()
                    } else {
                        path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default()
                    };
                    let hidden = m.file_attributes() & (FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM) != 0;
                    let size = if is_dir { 0 } else { m.len() };
                    let content = if settings.include_content && !is_dir {
                        read_content(&path, &ext, size)
                    } else {
                        String::new()
                    };
                    let changed = tx
                        .execute(
                            "INSERT OR IGNORE INTO files (path, name, ext, is_dir, size, modified, created, hidden) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                            params![path.display().to_string(), name, ext, is_dir, size as i64, millis(m.modified()), millis(m.created()), hidden],
                        )
                        .map_err(sql_err)?;
                    if changed == 1 {
                        let id = tx.last_insert_rowid();
                        tx.execute(
                            "INSERT INTO fts (rowid, name, content) VALUES (?1, ?2, ?3)",
                            params![id, name, content],
                        )
                        .map_err(sql_err)?;
                        count += 1;
                    }
                    ctx.progress().items = count;
                    ctx.tick();
                    pending += 1;
                    if pending >= BATCH {
                        tx.commit().map_err(sql_err)?;
                        tx = conn.transaction().map_err(sql_err)?;
                        pending = 0;
                    }
                }
            }
        }
        tx.execute(
            "INSERT OR REPLACE INTO meta (key, value) VALUES ('last_built', ?1)",
            params![crate::tasks::now_millis().to_string()],
        )
        .map_err(sql_err)?;
        tx.execute(
            "INSERT OR REPLACE INTO meta (key, value) VALUES ('settings', ?1)",
            params![serde_json::to_string(settings).unwrap_or_default()],
        )
        .map_err(sql_err)?;
        tx.commit().map_err(sql_err)?;
    }
    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode = DELETE;")
        .map_err(sql_err)?;
    drop(conn);
    if ctx.cancelled() {
        let _ = fs::remove_file(&tmp);
        return Err("Indexing was cancelled. The previous index is unchanged.".into());
    }
    remove_files(db_path);
    fs::rename(&tmp, db_path).map_err(|e| format!("Could not activate the new index: {e}"))?;
    status(db_path)
}

fn remove_files(db_path: &Path) {
    for suffix in ["", "-wal", "-shm"] {
        let mut p = db_path.as_os_str().to_owned();
        p.push(suffix);
        let _ = fs::remove_file(PathBuf::from(p));
    }
}

/// Deletes the index database. Only Nexdir's own index files are touched.
pub fn delete(db_path: &Path) -> Result<(), String> {
    remove_files(db_path);
    let _ = fs::remove_file(db_path.with_extension("building"));
    Ok(())
}

pub fn status(db_path: &Path) -> Result<IndexStatus, String> {
    if !db_path.exists() {
        return Ok(IndexStatus {
            exists: false,
            files: 0,
            last_built: None,
            size_bytes: 0,
            folders: Vec::new(),
            include_content: false,
        });
    }
    let conn = open(db_path)?;
    let files: i64 = conn.query_row("SELECT COUNT(*) FROM files", [], |r| r.get(0)).map_err(sql_err)?;
    let meta = |key: &str| -> Result<Option<String>, String> {
        conn.query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0))
            .optional()
            .map_err(sql_err)
    };
    let last_built = meta("last_built")?.and_then(|v| v.parse().ok());
    let settings: IndexSettings = meta("settings")?.and_then(|v| serde_json::from_str(&v).ok()).unwrap_or_default();
    Ok(IndexStatus {
        exists: true,
        files: files as u64,
        last_built,
        size_bytes: fs::metadata(db_path).map(|m| m.len()).unwrap_or(0),
        folders: settings.folders,
        include_content: settings.include_content,
    })
}

/// Escapes user words for an FTS5 MATCH expression: every word becomes a quoted prefix query.
fn fts_expression(terms: &[String]) -> String {
    terms
        .iter()
        .map(|t| format!("\"{}\"*", t.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Searches the index. The root of the query limits results to that folder (if set).
pub fn query(db_path: &Path, q: &SearchQuery) -> Result<SearchResult, String> {
    if !db_path.exists() {
        return Err("There is no search index yet. Build it in the index settings.".into());
    }
    let conn = open(db_path)?;
    let matcher = Matcher::new(q);
    let limit = q.limit.unwrap_or(super::DEFAULT_LIMIT).clamp(1, super::MAX_LIMIT);
    let terms: Vec<String> = if matcher.glob.is_some() {
        Vec::new()
    } else {
        matcher.terms.clone()
    };

    let mut sql = String::from("SELECT f.path, f.name, f.ext, f.is_dir, f.size, f.modified, f.created, f.hidden FROM files f");
    let mut args: Vec<rusqlite::types::Value> = Vec::new();
    let mut wheres: Vec<String> = Vec::new();
    if !terms.is_empty() {
        sql.push_str(" JOIN fts ON fts.rowid = f.id");
        wheres.push("fts MATCH ?".into());
        args.push(fts_expression(&terms).into());
    }
    let root = q.root.trim_end_matches('\\');
    if !root.is_empty() {
        // '^' escapes LIKE wildcards because '\' is the path separator.
        let escaped = root.to_lowercase().replace('^', "^^").replace('%', "^%").replace('_', "^_");
        wheres.push("lower(f.path) LIKE ? ESCAPE '^'".into());
        args.push(format!("{escaped}\\%").into());
    }
    if !q.include_hidden {
        wheres.push("f.hidden = 0".into());
    }
    if !wheres.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&wheres.join(" AND "));
    }
    let mut stmt = conn.prepare(&sql).map_err(sql_err)?;
    let mut rows = stmt.query(rusqlite::params_from_iter(args)).map_err(sql_err)?;

    let mut entries = Vec::new();
    let mut scanned = 0u64;
    let mut truncated = false;
    while let Some(row) = rows.next().map_err(sql_err)? {
        scanned += 1;
        let path: String = row.get(0).map_err(sql_err)?;
        let name: String = row.get(1).map_err(sql_err)?;
        let ext: String = row.get(2).map_err(sql_err)?;
        let is_dir: bool = row.get(3).map_err(sql_err)?;
        let size: i64 = row.get(4).map_err(sql_err)?;
        let modified: Option<i64> = row.get(5).map_err(sql_err)?;
        let created: Option<i64> = row.get(6).map_err(sql_err)?;
        let hidden: bool = row.get(7).map_err(sql_err)?;
        // Words are matched by FTS (name or content); wildcard patterns are checked here.
        if matcher.glob.is_some() && !matcher.matches_name(&name) {
            continue;
        }
        if !matcher.matches_attributes(is_dir, &ext, size.max(0) as u64, modified) {
            continue;
        }
        entries.push(FileEntry {
            name,
            path,
            kind: if is_dir { EntryKind::Dir } else { EntryKind::File },
            size: (!is_dir).then_some(size.max(0) as u64),
            modified,
            created,
            extension: (!is_dir && !ext.is_empty()).then_some(ext),
            hidden,
            system: false,
            readonly: false,
        });
        if entries.len() >= limit {
            truncated = true;
            break;
        }
    }
    Ok(SearchResult {
        entries,
        scanned,
        truncated,
        complete: true,
        source: "index",
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tasks::{testing::run_now, TaskKind};

    #[test]
    fn builds_queries_rebuilds_and_deletes() {
        let data = tempfile::tempdir().unwrap();
        let docs = data.path().join("docs");
        fs::create_dir_all(docs.join("Sub_Folder")).unwrap();
        fs::write(docs.join("Rëport 2026.pdf"), vec![0u8; 3000]).unwrap();
        fs::write(docs.join("Sub_Folder/meeting.md"), "agenda: budget planning").unwrap();
        fs::write(docs.join("photo.jpg"), vec![0u8; 10]).unwrap();
        let db = data.path().join("index/index.sqlite");
        let settings = IndexSettings {
            folders: vec![docs.display().to_string()],
            include_content: true,
            update_on_startup: false,
        };

        let (status, _) = run_now(TaskKind::IndexBuild, |ctx| build(&db, &settings, ctx).unwrap());
        assert!(status.exists);
        assert_eq!(status.files, 4);
        assert!(status.last_built.is_some());

        let q = |text: &str| SearchQuery {
            text: Some(text.into()),
            ..Default::default()
        };
        let names = |r: SearchResult| r.entries.into_iter().map(|e| e.name).collect::<Vec<_>>();
        // Prefix and diacritic-insensitive name matching.
        assert_eq!(names(query(&db, &q("rep")).unwrap()), vec!["Rëport 2026.pdf"]);
        assert_eq!(names(query(&db, &q("report 2026")).unwrap()), vec!["Rëport 2026.pdf"]);
        // Content search for small text files.
        assert_eq!(names(query(&db, &q("budget")).unwrap()), vec!["meeting.md"]);
        // Wildcards and filters.
        assert_eq!(names(query(&db, &q("*.jpg")).unwrap()), vec!["photo.jpg"]);
        let big = SearchQuery {
            min_size: Some(1000),
            ..Default::default()
        };
        assert_eq!(names(query(&db, &big).unwrap()), vec!["Rëport 2026.pdf"]);
        // Scope: the folder's own name contains '_' which must not act as a wildcard.
        let scoped = SearchQuery {
            root: docs.join("Sub_Folder").display().to_string(),
            ..Default::default()
        };
        assert_eq!(names(query(&db, &scoped).unwrap()), vec!["meeting.md"]);
        // Quotes in user input cannot break the FTS expression.
        assert!(query(&db, &q("\"unbalanced")).is_ok());

        // Rebuild replaces the index atomically.
        fs::write(docs.join("new file.txt"), "x").unwrap();
        let (status, _) = run_now(TaskKind::IndexBuild, |ctx| build(&db, &settings, ctx).unwrap());
        assert_eq!(status.files, 5);

        delete(&db).unwrap();
        assert!(!status_exists(&db));
        assert!(query(&db, &q("rep")).is_err());
    }

    fn status_exists(db: &Path) -> bool {
        status(db).unwrap().exists
    }
}
