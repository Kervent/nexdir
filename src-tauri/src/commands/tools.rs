//! Commands for Phase 3 tools: storage analysis, duplicates, search, index, rules.

use super::{blocking, validate_many, window_handle, MAX_BATCH};
use crate::analytics::{duplicates, storage};
use crate::error::{AppError, AppResult, ErrorCode};
use crate::operations::dedupe::{self, RemovalGroup};
use crate::operations::manager::OperationManager;
use crate::operations::organize::{self, Plan, Rule};
use crate::operations::RecycleResult;
use crate::search::index::{self, IndexSettings, IndexStatus};
use crate::search::{self, SearchQuery, SearchResult};
use crate::security::validate_path;
use crate::store::{self, AppPaths};
use crate::tasks::{TaskKind, TaskManager, TaskProgress};
use serde::Deserialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{State, WebviewWindow};

const MAX_RULES: usize = 200;

/// Set while an index build runs, so two builds never write the database at once.
static INDEX_BUILDING: AtomicBool = AtomicBool::new(false);

#[tauri::command]
pub fn start_storage_scan(tasks: State<'_, TaskManager>, path: String) -> AppResult<u64> {
    let root = validate_path(&path)?;
    Ok(tasks.spawn(TaskKind::StorageScan, move |ctx| storage::analyze(&root, ctx)))
}

#[tauri::command]
pub fn start_duplicate_scan(tasks: State<'_, TaskManager>, roots: Vec<String>, min_size: u64, include_hidden: bool) -> AppResult<u64> {
    let roots = validate_many(&roots)?;
    let opts = duplicates::DuplicateOptions {
        roots,
        min_size,
        include_hidden,
    };
    Ok(tasks.spawn(TaskKind::Duplicates, move |ctx| duplicates::find_duplicates(&opts, ctx)))
}

/// A duplicate group as resolved in the UI. Paths are validated before use.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateRemoval {
    size: u64,
    hash: String,
    keep: Vec<String>,
    remove: Vec<String>,
}

/// Moves selected duplicate copies to the Recycle Bin. The backend refuses to remove the last
/// copy of a group and re-verifies every file first, whatever the caller sends (see
/// `operations::dedupe`).
#[tauri::command]
pub async fn recycle_duplicates(window: WebviewWindow, groups: Vec<DuplicateRemoval>) -> AppResult<RecycleResult> {
    let count: usize = groups.iter().map(|g| g.keep.len() + g.remove.len()).sum();
    if count > MAX_BATCH {
        return Err(AppError::new(ErrorCode::Unsupported, "Too many items in one request."));
    }
    let groups = groups
        .into_iter()
        .map(|g| {
            let paths = |list: &[String]| list.iter().map(|p| validate_path(p)).collect::<AppResult<Vec<_>>>();
            Ok(RemovalGroup {
                size: g.size,
                hash: g.hash,
                keep: paths(&g.keep)?,
                remove: paths(&g.remove)?,
            })
        })
        .collect::<AppResult<Vec<_>>>()?;
    let owner = window_handle(&window);
    blocking(move || {
        // The shell operation needs its own single-threaded COM apartment.
        let shell = |paths: &[std::path::PathBuf]| {
            let batch = paths.to_vec();
            std::thread::spawn(move || crate::platform::recycle(&batch, owner))
                .join()
                .unwrap_or_else(|_| Err(std::io::Error::other("the Recycle Bin operation failed unexpectedly")))
        };
        dedupe::remove_duplicates(&groups, &shell)
    })
    .await
}

#[tauri::command]
pub fn start_search(tasks: State<'_, TaskManager>, query: SearchQuery) -> AppResult<u64> {
    let root = validate_path(&query.root)?;
    Ok(tasks.spawn(TaskKind::Search, move |ctx| search::search(&root, &query, ctx)))
}

#[tauri::command]
pub async fn index_search(paths: State<'_, AppPaths>, query: SearchQuery) -> AppResult<SearchResult> {
    if !query.root.is_empty() {
        validate_path(&query.root)?;
    }
    let db = paths.index_db();
    blocking(move || index::query(&db, &query).map_err(|m| AppError::new(ErrorCode::Io, m))).await
}

#[tauri::command]
pub fn cancel_task(tasks: State<'_, TaskManager>, id: u64) -> bool {
    tasks.cancel(id)
}

#[tauri::command]
pub fn task_status(tasks: State<'_, TaskManager>, id: u64) -> Option<TaskProgress> {
    tasks.progress(id)
}

#[tauri::command]
pub fn task_result(tasks: State<'_, TaskManager>, id: u64) -> Option<serde_json::Value> {
    tasks.result(id)
}

#[tauri::command]
pub fn index_get_settings(paths: State<'_, AppPaths>) -> IndexSettings {
    store::load(&paths.index_settings_file())
}

#[tauri::command]
pub fn index_save_settings(paths: State<'_, AppPaths>, settings: IndexSettings) -> AppResult<()> {
    let folders = settings
        .folders
        .iter()
        .map(|f| validate_path(f).map(|p| p.display().to_string()))
        .collect::<AppResult<Vec<_>>>()?;
    let clean = IndexSettings { folders, ..settings };
    store::save(&paths.index_settings_file(), &clean).map_err(|m| AppError::new(ErrorCode::Io, m))
}

#[tauri::command]
pub async fn index_status(paths: State<'_, AppPaths>) -> AppResult<IndexStatus> {
    let db = paths.index_db();
    blocking(move || index::status(&db).map_err(|m| AppError::new(ErrorCode::Io, m))).await
}

/// Starts an index build from the saved settings. Returns the task id.
pub fn spawn_index_build(paths: &AppPaths, tasks: &TaskManager) -> AppResult<u64> {
    let settings: IndexSettings = store::load(&paths.index_settings_file());
    if settings.folders.is_empty() {
        return Err(AppError::new(ErrorCode::InvalidPath, "Choose at least one folder to index."));
    }
    if INDEX_BUILDING.swap(true, Ordering::SeqCst) {
        return Err(AppError::new(ErrorCode::Unsupported, "The index is already being built."));
    }
    let db = paths.index_db();
    Ok(tasks.spawn(TaskKind::IndexBuild, move |ctx| {
        let result = index::build(&db, &settings, ctx);
        INDEX_BUILDING.store(false, Ordering::SeqCst);
        result
    }))
}

#[tauri::command]
pub fn index_build(paths: State<'_, AppPaths>, tasks: State<'_, TaskManager>) -> AppResult<u64> {
    spawn_index_build(&paths, &tasks)
}

#[tauri::command]
pub fn index_delete(paths: State<'_, AppPaths>) -> AppResult<()> {
    if INDEX_BUILDING.load(Ordering::SeqCst) {
        return Err(AppError::new(
            ErrorCode::Unsupported,
            "Wait for the running index build to finish or cancel it first.",
        ));
    }
    index::delete(&paths.index_db()).map_err(|m| AppError::new(ErrorCode::Io, m))
}

#[tauri::command]
pub fn rules_get(paths: State<'_, AppPaths>) -> Vec<Rule> {
    store::load(&paths.rules_file())
}

#[tauri::command]
pub fn rules_save(paths: State<'_, AppPaths>, rules: Vec<Rule>) -> AppResult<()> {
    if rules.len() > MAX_RULES {
        return Err(AppError::new(ErrorCode::Unsupported, "Too many rules."));
    }
    store::save(&paths.rules_file(), &rules).map_err(|m| AppError::new(ErrorCode::Io, m))
}

/// Dry run of a rule: lists what would be transferred, changes nothing.
#[tauri::command]
pub async fn organize_preview(rule: Rule) -> AppResult<Plan> {
    blocking(move || organize::plan(&rule)).await
}

/// Transfers the confirmed items of a rule through the regular operation queue.
#[tauri::command]
pub async fn organize_apply(manager: State<'_, OperationManager>, rule: Rule, sources: Vec<String>) -> AppResult<u64> {
    validate_many(&sources)?;
    let request = blocking(move || organize::prepare(&rule, &sources)).await?;
    Ok(manager.enqueue(request))
}

pub fn task_manager(emit: impl Fn(&TaskProgress) + Send + Sync + 'static) -> TaskManager {
    TaskManager::new(Arc::new(emit))
}
