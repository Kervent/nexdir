fn main() {
    // Every IPC command the frontend may call is declared here. Tauri generates an
    // `allow-<command>` permission for each, and only the ones referenced in
    // capabilities/main.json are reachable from the WebView.
    println!("cargo:rerun-if-changed=windows-app-manifest.xml");
    // Test binaries get the same manifest (common controls v6), which Tauri's mock runtime in
    // the updater tests needs to load.
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        let manifest = std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("windows-app-manifest.xml");
        println!("cargo:rustc-link-arg-tests=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg-tests=/MANIFESTINPUT:{}", manifest.display());
    }
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
            "open_project_url",
            "system_info",
            "find_conflicts",
            "start_operation",
            "cancel_operation",
            "list_operations",
            "operation_report",
            "clipboard_set_files",
            "clipboard_get_files",
            "preview_path",
            "network_parse",
            "network_overview",
            "network_probe",
            "network_list_shares",
            "network_discover",
            "network_connect",
            "network_disconnect",
            "network_favorites_get",
            "network_favorites_save",
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
            "confirm_close",
            "update_info",
            "update_ack_last_install",
            "update_save_settings",
            "update_check",
            "update_download",
            "update_cancel",
            "update_snooze",
            "update_install",
            "update_open_release",
        ]),
    ))
    .expect("failed to run tauri-build");
}
