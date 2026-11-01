//! IPC commands exposed to the WebView. This layer only validates input, moves blocking
//! work off the async runtime and delegates to the domain modules.

use crate::error::{AppError, AppResult, ErrorCode};
use crate::filesystem::{self, drives::DriveInfo, DirListing, KnownFolder, ResolvedPath};
use crate::operations::manager::OperationManager;
use crate::operations::transfer::{self, ConflictCheck, ConflictPolicy, OperationProgress, OperationReport, TransferKind, TransferRequest};
use crate::operations::{self, RecycleResult};
use crate::platform;
use crate::security::{self, validate_path};
use std::path::PathBuf;
use tauri::{AppHandle, Manager, State, WebviewWindow};

pub mod tools;

/// Upper bound on items per request, to keep a misbehaving frontend from exhausting resources.
pub(crate) const MAX_BATCH: usize = 100_000;

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> AppResult<T> + Send + 'static) -> AppResult<T> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|_| AppError::new(ErrorCode::Io, "A background task failed unexpectedly."))?
}

fn validate_many(paths: &[String]) -> AppResult<Vec<PathBuf>> {
    if paths.is_empty() {
        return Err(AppError::new(ErrorCode::InvalidPath, "No items were selected."));
    }
    if paths.len() > MAX_BATCH {
        return Err(AppError::new(ErrorCode::Unsupported, "Too many items in one request."));
    }
    paths.iter().map(|p| validate_path(p)).collect()
}

fn window_handle(window: &WebviewWindow) -> Option<isize> {
    window.hwnd().ok().map(|h| h.0 as isize)
}

#[tauri::command]
pub async fn list_dir(path: String, show_hidden: bool) -> AppResult<DirListing> {
    let path = validate_path(&path)?;
    blocking(move || filesystem::list_dir(&path, show_hidden)).await
}

#[tauri::command]
pub async fn list_drives() -> AppResult<Vec<DriveInfo>> {
    blocking(|| Ok(filesystem::drives::list_drives())).await
}

#[tauri::command]
pub fn known_folders() -> Vec<KnownFolder> {
    filesystem::known_folders()
}

#[tauri::command]
pub async fn resolve_path(input: String) -> AppResult<ResolvedPath> {
    blocking(move || filesystem::resolve_input(&input)).await
}

/// Folder (or file to select) passed on the command line, e.g. `nexdir.exe "D:\Photos"`.
/// Returns `None` when no usable path was given; the UI then starts on Home.
#[tauri::command]
pub async fn startup_location() -> Option<ResolvedPath> {
    let arg = std::env::args().skip(1).find(|a| !a.starts_with('-'))?;
    blocking(move || filesystem::resolve_input(&arg)).await.ok()
}

#[tauri::command]
pub async fn create_folder(parent: String, name: String) -> AppResult<String> {
    let parent = validate_path(&parent)?;
    blocking(move || operations::create_folder(&parent, &name).map(|p| p.display().to_string())).await
}

#[tauri::command]
pub async fn rename_entry(path: String, new_name: String) -> AppResult<String> {
    let path = validate_path(&path)?;
    blocking(move || operations::rename_entry(&path, &new_name).map(|p| p.display().to_string())).await
}

#[tauri::command]
pub async fn recycle_entries(window: WebviewWindow, paths: Vec<String>) -> AppResult<RecycleResult> {
    let paths = validate_many(&paths)?;
    let owner = window_handle(&window);
    blocking(move || operations::recycle_entries(&paths, owner)).await
}

/// Opens a file with its associated program. Executable types (programs, scripts,
/// shortcuts) only run when the user explicitly confirmed it in the UI.
#[tauri::command]
pub async fn open_path(path: String, allow_executable: bool) -> AppResult<()> {
    let path = validate_path(&path)?;
    if security::is_executable(&path) && !allow_executable {
        return Err(AppError::new(ErrorCode::Unsupported, "Opening programs and scripts requires confirmation.").with_path(&path));
    }
    blocking(move || {
        std::fs::symlink_metadata(&path).map_err(|e| AppError::from_io(&e, &path))?;
        platform::shell_open(&path).map_err(|e| AppError::from_io(&e, &path))
    })
    .await
}

/// Opens one of the project's web pages (repository, issues, docs, Discord invite) in the
/// default browser. Only called after a click; Nexdir itself makes no network requests.
#[tauri::command]
pub async fn open_url(url: String) -> AppResult<()> {
    security::validate_external_url(&url)?;
    blocking(move || platform::open_url(&url).map_err(|e| AppError::new(ErrorCode::Io, format!("Could not open the browser: {e}")))).await
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    pub app_version: &'static str,
    pub os_name: String,
    pub os_version: Option<String>,
    pub os_build: Option<String>,
    pub architecture: &'static str,
}

/// Facts for bug reports. Contains no user names, paths or hardware identifiers.
#[tauri::command]
pub fn system_info() -> SystemInfo {
    let os = platform::os_version();
    SystemInfo {
        app_version: env!("CARGO_PKG_VERSION"),
        os_name: os.name,
        os_version: os.display_version,
        os_build: os.build,
        architecture: std::env::consts::ARCH,
    }
}

#[tauri::command]
pub fn show_properties(app: AppHandle, window: WebviewWindow, path: String) -> AppResult<()> {
    let path = validate_path(&path)?;
    std::fs::symlink_metadata(&path).map_err(|e| AppError::from_io(&e, &path))?;
    let owner = window_handle(&window);
    app.run_on_main_thread(move || {
        let _ = platform::shell_properties(&path, owner);
    })
    .map_err(|_| AppError::new(ErrorCode::Io, "Could not open the properties dialog."))
}

#[tauri::command]
pub async fn find_conflicts(sources: Vec<String>, destination: String) -> AppResult<ConflictCheck> {
    let sources = validate_many(&sources)?;
    let destination = validate_path(&destination)?;
    blocking(move || Ok(transfer::find_conflicts(&sources, &destination))).await
}

#[tauri::command]
pub fn start_operation(
    manager: State<'_, OperationManager>,
    kind: TransferKind,
    sources: Vec<String>,
    destination: String,
    policy: ConflictPolicy,
) -> AppResult<u64> {
    let sources = validate_many(&sources)?;
    let destination = validate_path(&destination)?;
    Ok(manager.enqueue(TransferRequest {
        kind,
        sources,
        destination,
        policy,
    }))
}

#[tauri::command]
pub fn cancel_operation(manager: State<'_, OperationManager>, id: u64) -> bool {
    manager.cancel(id)
}

#[tauri::command]
pub fn list_operations(manager: State<'_, OperationManager>) -> Vec<OperationProgress> {
    manager.list()
}

#[tauri::command]
pub fn operation_report(manager: State<'_, OperationManager>, id: u64) -> Option<OperationReport> {
    manager.report(id)
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardFiles {
    pub paths: Vec<String>,
    pub cut: bool,
}

/// Puts files on the Windows clipboard so they can be pasted in Nexdir or Explorer.
#[tauri::command]
pub async fn clipboard_set_files(window: WebviewWindow, paths: Vec<String>, cut: bool) -> AppResult<()> {
    let paths = validate_many(&paths)?;
    let owner = window_handle(&window).ok_or_else(|| AppError::new(ErrorCode::Io, "No window available for the clipboard."))?;
    blocking(move || {
        platform::clipboard::set_files(&paths, cut, owner)
            .map_err(|e| AppError::new(ErrorCode::Io, format!("Could not access the clipboard: {e}")))
    })
    .await
}

/// Files currently on the Windows clipboard (from Nexdir, Explorer or other apps).
/// Entries that are not valid absolute paths are dropped.
#[tauri::command]
pub async fn clipboard_get_files() -> AppResult<Option<ClipboardFiles>> {
    blocking(|| {
        let files =
            platform::clipboard::get_files().map_err(|e| AppError::new(ErrorCode::Io, format!("Could not access the clipboard: {e}")))?;
        Ok(files.and_then(|f| {
            let paths: Vec<String> = f
                .paths
                .iter()
                .filter_map(|p| validate_path(&p.to_string_lossy()).ok())
                .map(|p| p.display().to_string())
                .take(MAX_BATCH)
                .collect();
            (!paths.is_empty()).then_some(ClipboardFiles { paths, cut: f.cut })
        }))
    })
    .await
}

/// Read-only preview of a file or folder. Never executes or interprets content.
#[tauri::command]
pub async fn preview_path(path: String) -> AppResult<filesystem::preview::Preview> {
    let path = validate_path(&path)?;
    blocking(move || filesystem::preview::preview(&path)).await
}

/// Registers state that commands depend on.
pub fn init_state(app: &AppHandle) {
    use tauri::Emitter;
    let handle = app.clone();
    let manager = OperationManager::new(std::sync::Arc::new(move |p: &OperationProgress| {
        let _ = handle.emit("operation-progress", p);
    }));
    app.manage(manager);

    let tasks_handle = app.clone();
    let tasks = tools::task_manager(move |p| {
        let _ = tasks_handle.emit("task-progress", p);
    });
    let paths = crate::store::AppPaths {
        config_dir: app.path().app_config_dir().unwrap_or_else(|_| std::env::temp_dir().join("nexdir")),
        data_dir: app
            .path()
            .app_local_data_dir()
            .unwrap_or_else(|_| std::env::temp_dir().join("nexdir")),
    };
    // Refresh the search index on start only if the user enabled that option.
    let settings: crate::search::index::IndexSettings = crate::store::load(&paths.index_settings_file());
    if settings.update_on_startup && !settings.folders.is_empty() {
        let _ = tools::spawn_index_build(&paths, &tasks);
    }
    app.manage(tasks);
    app.manage(paths);
}
