fn main() {
    // Every IPC command the frontend may call is declared here. Tauri generates an
    // `allow-<command>` permission for each, and only the ones referenced in
    // capabilities/main.json are reachable from the WebView.
    println!("cargo:rerun-if-changed=windows-app-manifest.xml");
    let windows = tauri_build::WindowsAttributes::new().app_manifest(include_str!("windows-app-manifest.xml"));
    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows).app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "list_dir",
            "list_drives",
            "known_folders",
            "resolve_path",
            "startup_location",
            "create_folder",
            "rename_entry",
            "recycle_entries",
            "open_path",
            "show_properties",
            "open_url",
            "system_info",
            "find_conflicts",
            "start_operation",
            "cancel_operation",
            "list_operations",
            "operation_report",
            "clipboard_set_files",
            "clipboard_get_files",
            "preview_path",
            "start_storage_scan",
            "start_duplicate_scan",
            "recycle_duplicates",
            "start_search",
            "index_search",
            "cancel_task",
            "task_status",
            "task_result",
            "index_get_settings",
            "index_save_settings",
            "index_status",
            "index_build",
            "index_delete",
            "rules_get",
            "rules_save",
            "organize_preview",
            "organize_apply",
        ]),
    ))
    .expect("failed to run tauri-build");
}
