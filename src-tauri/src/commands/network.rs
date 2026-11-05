//! Network storage commands (see `crate::network`). Every call that may contact a server runs
//! off the async runtime and through the bounded network gate.

use super::{blocking, window_handle};
use crate::error::{AppError, AppResult, ErrorCode};
use crate::network::{self, favorites::Favorite, status::StatusReport, unc, ConnectOutcome, NetworkState, Overview};
use crate::operations::manager::OperationManager;
use crate::store::AppPaths;
use crate::tasks::{TaskKind, TaskManager};
use std::sync::Arc;
use tauri::{State, WebviewWindow};

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedNetworkPath {
    pub path: String,
    pub kind: unc::UncKind,
    pub server: String,
    pub share: Option<String>,
}

/// Validates a network path as the user types it (no network access).
#[tauri::command]
pub fn network_parse(input: String) -> AppResult<ParsedNetworkPath> {
    let p = unc::parse(&input)?;
    Ok(ParsedNetworkPath {
        path: p.display(),
        kind: p.kind(),
        server: p.server.clone(),
        share: p.share.clone(),
    })
}

/// Network drives, connections and saved locations. Reads Windows' local state only.
#[tauri::command]
pub async fn network_overview(state: State<'_, Arc<NetworkState>>, paths: State<'_, AppPaths>) -> AppResult<Overview> {
    let state = state.inner().clone();
    let file = paths.network_file();
    blocking(move || Ok(network::overview(&state, &network::favorites::load(&file)))).await
}

/// Checks whether a share or folder answers (contacts the server).
#[tauri::command]
pub async fn network_probe(state: State<'_, Arc<NetworkState>>, path: String) -> AppResult<StatusReport> {
    let state = state.inner().clone();
    blocking(move || network::probe(&state, &path)).await
}

/// Visible shares of one server (contacts the server). The server name is validated first.
#[tauri::command]
pub async fn network_list_shares(
    state: State<'_, Arc<NetworkState>>,
    server: String,
) -> AppResult<Result<Vec<network::DiscoveredShare>, StatusReport>> {
    let p = unc::parse(&server_path(&server))?;
    let state = state.inner().clone();
    blocking(move || Ok(network::list_shares(&state, &p.server))).await
}

/// Accepts a server as `NAS` or `\\NAS` (the Network view passes the bare name).
fn server_path(server: &str) -> String {
    let s = server.trim();
    if s.starts_with(r"\\") || s.starts_with("//") {
        s.to_string()
    } else {
        format!(r"\\{s}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_names_are_accepted_with_or_without_backslashes() {
        for input in ["localhost", r"\\localhost", "//localhost", " NAS-WOHNZIMMER "] {
            let p = unc::parse(&server_path(input)).unwrap();
            assert_eq!(p.kind(), unc::UncKind::Server, "{input}");
        }
        assert!(unc::parse(&server_path(r"\.\pipe")).is_err());
        assert!(unc::parse(&server_path("a b")).is_err());
    }
}

/// Starts a network search as a cancellable background task (result: `network::Discovery`).
/// Servers known from connections and favourites are included; nothing else is probed.
#[tauri::command]
pub fn network_discover(state: State<'_, Arc<NetworkState>>, paths: State<'_, AppPaths>, tasks: State<'_, TaskManager>) -> AppResult<u64> {
    if tasks.is_running(TaskKind::NetworkDiscovery) {
        return Err(AppError::new(ErrorCode::Unsupported, "A network search is already running."));
    }
    let state = state.inner().clone();
    let file = paths.network_file();
    Ok(tasks.spawn(TaskKind::NetworkDiscovery, move |ctx| {
        let overview = network::overview(&state, &network::favorites::load(&file));
        let mut known: Vec<String> = overview.locations.iter().map(|l| l.server.clone()).collect();
        known.sort_by_key(|s| s.to_lowercase());
        known.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
        Ok(network::discover(&state, &known, ctx))
    }))
}

/// Connects to a share. `sign_in` shows Windows' own sign-in dialog (owned by Nexdir's window).
#[tauri::command]
pub async fn network_connect(
    window: WebviewWindow,
    state: State<'_, Arc<NetworkState>>,
    path: String,
    sign_in: bool,
) -> AppResult<ConnectOutcome> {
    let owner = if sign_in {
        Some(window_handle(&window).ok_or_else(|| AppError::new(ErrorCode::Io, "No window available for the sign-in dialog."))?)
    } else {
        None
    };
    let state = state.inner().clone();
    blocking(move || network::connect(&state, &path, owner)).await
}

/// Disconnects a connection Nexdir created. Refused while file operations are running.
#[tauri::command]
pub async fn network_disconnect(
    state: State<'_, Arc<NetworkState>>,
    operations: State<'_, OperationManager>,
    path: String,
) -> AppResult<StatusReport> {
    if operations.busy() > 0 {
        return Err(AppError::new(
            ErrorCode::InUse,
            "Wait until the running file operations have finished, then disconnect.",
        ));
    }
    let state = state.inner().clone();
    blocking(move || network::disconnect(&state, &path)).await
}

#[tauri::command]
pub fn network_favorites_get(paths: State<'_, AppPaths>) -> Vec<Favorite> {
    network::favorites::load(&paths.network_file())
}

/// Saves the whole list of network favourites (paths and names only).
#[tauri::command]
pub fn network_favorites_save(paths: State<'_, AppPaths>, favorites: Vec<Favorite>) -> AppResult<Vec<Favorite>> {
    network::favorites::save(&paths.network_file(), &favorites)
}
