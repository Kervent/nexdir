//! Nexdir backend. All filesystem logic lives here; the React frontend talks to it
//! exclusively through the typed commands in `commands`.

mod analytics;
mod commands;
#[cfg(test)]
mod compat_tests;
mod error;
mod filesystem;
mod operations;
#[cfg(test)]
mod perf;
mod platform;
mod search;
mod security;
mod store;
mod tasks;
#[doc(hidden)] // public for the integration tests only
pub mod updater;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Remembers window size, position and maximised state between sessions.
        .plugin(tauri_plugin_window_state::Builder::default().build())
        // Only the Rust side uses the updater; the WebView gets no updater permissions.
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            commands::init_state(app.handle());
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing while a copy or move runs would interrupt it: ask first (see confirm_close).
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                use tauri::{Emitter, Manager};
                let busy = window.state::<operations::manager::OperationManager>().busy();
                if busy > 0 && !commands::close_confirmed() {
                    api.prevent_close();
                    let _ = window.emit("close-blocked", busy);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_dir,
            commands::list_drives,
            commands::known_folders,
            commands::resolve_path,
            commands::startup_location,
            commands::create_folder,
            commands::rename_entry,
            commands::recycle_entries,
            commands::open_path,
            commands::show_properties,
            commands::open_url,
            commands::open_project_url,
            commands::system_info,
            commands::find_conflicts,
            commands::start_operation,
            commands::cancel_operation,
            commands::list_operations,
            commands::operation_report,
            commands::clipboard_set_files,
            commands::clipboard_get_files,
            commands::preview_path,
            commands::tools::start_storage_scan,
            commands::tools::start_duplicate_scan,
            commands::tools::recycle_duplicates,
            commands::tools::start_search,
            commands::tools::index_search,
            commands::tools::cancel_task,
            commands::tools::task_status,
            commands::tools::task_result,
            commands::tools::index_get_settings,
            commands::tools::index_save_settings,
            commands::tools::index_status,
            commands::tools::index_build,
            commands::tools::index_delete,
            commands::tools::rules_get,
            commands::tools::rules_save,
            commands::tools::organize_preview,
            commands::tools::organize_apply,
            commands::confirm_close,
            updater::update_info,
            updater::update_ack_last_install,
            updater::update_save_settings,
            updater::update_check,
            updater::update_download,
            updater::update_cancel,
            updater::update_snooze,
            updater::update_install,
            updater::update_open_release,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Nexdir");
}
