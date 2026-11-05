//! Network storage (SMB shares, NAS): existing Windows connections, discovery, manual
//! connection, sign-in through Windows and saved network locations.
//!
//! Principles (see docs/technical/smb-integration.md):
//! * Windows does all the networking. Nexdir calls the Windows network functions and the SMB
//!   client; it never implements SMB, never changes security settings, firewall rules or the
//!   registry, and never falls back to guest access.
//! * Nothing happens in the background. Existing connections are read locally from Windows;
//!   servers are contacted only after a user action (opening a location, "Search the network",
//!   "Connect", "Check status").
//! * Every potentially blocking call runs through `pool::NETWORK` (bounded, with timeouts).
//! * Nexdir only disconnects connections that it created itself in this session.
//! * Credentials are entered in Windows' own sign-in dialog. Nexdir never sees, stores or logs
//!   a user name or password.

pub mod favorites;
pub mod pool;
pub mod status;
pub mod unc;
pub mod wnet;

use crate::error::{AppError, AppResult, ErrorCode};
use crate::tasks::TaskContext;
use pool::{Gate, GateError};
use serde::Serialize;
use status::{NetworkStatus, StatusReport};
use std::collections::{BTreeMap, HashSet};
use std::path::{Component, Path, Prefix};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use unc::{UncKind, UncPath};
use wnet::{NetApi, NetResource, ResourceKind};

/// Waiting time for quick local queries of Windows (existing connections).
const LOCAL_TIMEOUT: Duration = Duration::from_secs(5);
/// Waiting time for a server (status check, list of shares, connect without sign-in).
pub const SERVER_TIMEOUT: Duration = Duration::from_secs(20);
/// The Windows sign-in dialog stays open while the user types.
const PROMPT_TIMEOUT: Duration = Duration::from_secs(600);
/// Bounds for discovery: servers whose shares are listed, enumeration depth.
const MAX_SERVERS: usize = 64;
const MAX_DEPTH: usize = 3;

/// True if `path` is on a network share: a UNC path or a drive letter mapped to a share.
/// Mapped drives are recognised by their drive type, which Windows answers locally.
pub fn is_remote(path: &Path) -> bool {
    match path.components().next() {
        Some(Component::Prefix(p)) => match p.kind() {
            Prefix::UNC(..) | Prefix::VerbatimUNC(..) => true,
            Prefix::Disk(_) | Prefix::VerbatimDisk(_) => crate::platform::drive_type_of(path) == crate::platform::DriveType::Remote,
            _ => false,
        },
        _ => false,
    }
}

fn gate_report(e: GateError) -> StatusReport {
    match e {
        GateError::TimedOut => StatusReport::timed_out(),
        GateError::Busy => StatusReport {
            status: NetworkStatus::TimedOut,
            message: "Several network requests are still waiting for servers that do not answer. Try again in a moment.".into(),
            code: None,
        },
    }
}

/// Connections Nexdir created itself (only these may be disconnected by Nexdir).
#[derive(Default)]
pub struct Registry {
    managed: Mutex<HashSet<String>>,
}

impl Registry {
    pub fn all(&self) -> Vec<String> {
        self.managed.lock().unwrap().iter().cloned().collect()
    }
    pub fn insert(&self, share_root: &str) {
        self.managed.lock().unwrap().insert(share_root.to_lowercase());
    }
    pub fn contains(&self, share_root: &str) -> bool {
        self.managed.lock().unwrap().contains(&share_root.to_lowercase())
    }
    pub fn remove(&self, share_root: &str) {
        self.managed.lock().unwrap().remove(&share_root.to_lowercase());
    }
}

pub struct NetworkState {
    pub api: Arc<dyn NetApi>,
    pub gate: &'static Gate,
    pub registry: Registry,
}

impl NetworkState {
    pub fn windows() -> Self {
        Self {
            api: Arc::new(wnet::WindowsNet),
            gate: &pool::NETWORK,
            registry: Registry::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Source {
    /// Mapped to a drive letter.
    Drive,
    /// Connected without a drive letter.
    Connection,
    /// A drive mapping Windows remembers but that is not connected now.
    Remembered,
    Favorite,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkLocation {
    /// Canonical UNC path (`\\server\share[\folder]`).
    pub path: String,
    pub server: String,
    pub share: Option<String>,
    /// Name to show: favourite name, else the share or folder name.
    pub label: String,
    /// Drive letter (`Z:`) for mapped drives.
    pub drive: Option<String>,
    pub sources: Vec<Source>,
    /// What Windows reports without contacting the server: `connected`, `notConnected`
    /// (remembered drive) or `None` (favourite; status unknown until checked).
    pub status: Option<NetworkStatus>,
    /// Created by Nexdir in this session; only such connections can be disconnected here.
    pub managed: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Overview {
    pub locations: Vec<NetworkLocation>,
    /// Set when Windows could not report its connections (the rest is still shown).
    pub warning: Option<String>,
}

fn label_of(p: &UncPath) -> String {
    p.segments
        .last()
        .cloned()
        .or_else(|| p.share.clone())
        .unwrap_or_else(|| p.server.clone())
}

/// Merges mapped drives, connections, remembered drives and favourites into one list without
/// duplicates (the same share under a drive letter and as UNC path is one entry).
pub fn merge(
    connected: &[NetResource],
    remembered: &[NetResource],
    favs: &[favorites::Favorite],
    registry: &Registry,
) -> Vec<NetworkLocation> {
    let mut by_key: BTreeMap<String, NetworkLocation> = BTreeMap::new();
    let mut add = |remote: &str, drive: Option<&str>, source: Source, status: Option<NetworkStatus>, label: Option<&str>| {
        let Ok(p) = unc::parse(remote) else { return };
        if p.kind() == UncKind::Server {
            return;
        }
        let entry = by_key.entry(p.key()).or_insert_with(|| NetworkLocation {
            path: p.display(),
            server: p.server.clone(),
            share: p.share.clone(),
            label: label_of(&p),
            drive: None,
            sources: vec![],
            status: None,
            managed: p.share_root().is_some_and(|r| registry.contains(&r) && p.segments.is_empty()),
        });
        if !entry.sources.contains(&source) {
            entry.sources.push(source);
        }
        if let Some(d) = drive.filter(|d| d.len() == 2 && d.ends_with(':')) {
            entry.drive = Some(d.to_ascii_uppercase());
        }
        if let Some(l) = label.filter(|l| !l.is_empty()) {
            entry.label = l.to_string();
        }
        // "connected" wins over "not connected"; favourites add no status.
        if status == Some(NetworkStatus::Connected) || entry.status.is_none() {
            entry.status = status.or(entry.status);
        }
    };
    for r in connected {
        let source = if r.local.is_some() { Source::Drive } else { Source::Connection };
        add(&r.remote, r.local.as_deref(), source, Some(NetworkStatus::Connected), None);
    }
    for r in remembered {
        add(
            &r.remote,
            r.local.as_deref(),
            Source::Remembered,
            Some(NetworkStatus::NotConnected),
            None,
        );
    }
    for f in favs {
        add(&f.path, None, Source::Favorite, None, Some(&f.label));
    }
    by_key.into_values().collect()
}

/// Lists what Windows already knows. Local query only: no server is contacted.
pub fn overview(state: &NetworkState, favs: &[favorites::Favorite]) -> Overview {
    let api = state.api.clone();
    let result = state.gate.run(LOCAL_TIMEOUT, move || (api.connected(), api.remembered()));
    let (connected, remembered, warning) = match result {
        Ok((c, r)) => {
            let warning = c
                .as_ref()
                .err()
                .map(|code| format!("Windows could not list its network connections (error {code})."));
            (c.unwrap_or_default(), r.unwrap_or_default(), warning)
        }
        Err(e) => (vec![], vec![], Some(gate_report(e).message)),
    };
    Overview {
        locations: merge(&connected, &remembered, favs, &state.registry),
        warning,
    }
}

/// Checks whether a location answers. Contacts the server.
pub fn probe(state: &NetworkState, path: &str) -> AppResult<StatusReport> {
    let p = unc::parse(path)?;
    if p.kind() == UncKind::Server {
        return Err(AppError::new(
            ErrorCode::InvalidPath,
            "A server alone cannot be checked; choose one of its shares.",
        ));
    }
    let api = state.api.clone();
    let target = p.display();
    Ok(match state.gate.run(SERVER_TIMEOUT, move || api.probe(&target)) {
        Ok(Ok(())) => StatusReport::connected(),
        Ok(Err(code)) => status::from_code(code),
        Err(e) => gate_report(e),
    })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredShare {
    pub path: String,
    pub name: String,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredServer {
    /// `\\server`
    pub path: String,
    pub name: String,
    /// True if Windows' network list reported the server; false if it was only known from
    /// existing connections or favourites.
    pub announced: bool,
    /// The server's shares, or `None` if they could not be listed (see `status`).
    pub shares: Option<Vec<DiscoveredShare>>,
    pub status: Option<StatusReport>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Discovery {
    pub servers: Vec<DiscoveredServer>,
    /// False when Windows' network list was empty or unavailable (common on current Windows;
    /// the shares of known servers are still listed).
    pub network_list_available: bool,
    pub cancelled: bool,
}

fn server_name(remote: &str) -> Option<String> {
    let p = unc::parse(remote).ok()?;
    Some(p.server)
}

/// Lists the visible shares of one server (Windows asks the server; hidden `$` shares are left
/// out, like in Explorer). Contacts the server.
pub fn list_shares(state: &NetworkState, server: &str) -> Result<Vec<DiscoveredShare>, StatusReport> {
    let api = state.api.clone();
    let container = NetResource {
        remote: format!(r"\\{server}"),
        local: None,
        kind: ResourceKind::Server,
        container: true,
        comment: None,
    };
    let items = match state.gate.run(SERVER_TIMEOUT, move || api.enumerate(Some(&container))) {
        Ok(Ok(items)) => items,
        Ok(Err(code)) => return Err(status::from_code(code)),
        Err(e) => return Err(gate_report(e)),
    };
    let mut seen = HashSet::new();
    let mut shares: Vec<DiscoveredShare> = items
        .into_iter()
        .filter_map(|r| {
            let p = unc::parse(&r.remote).ok()?;
            let share = p.share.clone()?;
            (p.kind() == UncKind::Share && !share.ends_with('$') && p.server.eq_ignore_ascii_case(server) && seen.insert(p.key())).then(
                || DiscoveredShare {
                    path: p.display(),
                    name: share,
                    comment: r.comment,
                },
            )
        })
        .collect();
    shares.sort_by_key(|s| s.name.to_lowercase());
    Ok(shares)
}

/// Searches for SMB servers and their shares, started by the user. Sources: Windows' network
/// list (providers → workgroups → servers, as far as Windows reports them) and servers already
/// known from connections and favourites. No address ranges, no port scans.
pub fn discover(state: &NetworkState, known_servers: &[String], ctx: &mut TaskContext) -> Discovery {
    let mut result = Discovery::default();
    let mut servers: BTreeMap<String, (String, bool)> = BTreeMap::new();
    for s in known_servers {
        servers.entry(s.to_lowercase()).or_insert((s.clone(), false));
    }

    // Windows' network list, breadth-first and bounded.
    ctx.set_phase("Reading the Windows network list", None);
    let mut queue: Vec<(Option<NetResource>, usize)> = vec![(None, 0)];
    while let Some((container, depth)) = queue.pop() {
        if ctx.cancelled() {
            result.cancelled = true;
            break;
        }
        let api = state.api.clone();
        let c = container.clone();
        let items = match state.gate.run(SERVER_TIMEOUT, move || api.enumerate(c.as_ref())) {
            Ok(Ok(items)) => items,
            _ => continue,
        };
        ctx.progress().items += items.len() as u64;
        ctx.tick();
        for item in items {
            match item.kind {
                ResourceKind::Server => {
                    result.network_list_available = true;
                    if let Some(name) = server_name(&item.remote) {
                        servers
                            .entry(name.to_lowercase())
                            .and_modify(|e| e.1 = true)
                            .or_insert((name, true));
                    }
                }
                ResourceKind::Network | ResourceKind::Domain | ResourceKind::Other if item.container && depth + 1 < MAX_DEPTH => {
                    result.network_list_available = true;
                    queue.push((Some(item), depth + 1));
                }
                _ => {}
            }
        }
        if servers.len() >= MAX_SERVERS {
            break;
        }
    }

    // Shares of every server found (one server at a time; each call is bounded).
    let list: Vec<(String, bool)> = servers.into_values().take(MAX_SERVERS).collect();
    ctx.set_phase("Listing shares", Some(list.len() as u64));
    for (name, announced) in list {
        if ctx.cancelled() {
            result.cancelled = true;
            break;
        }
        ctx.progress().current = Some(format!(r"\\{name}"));
        ctx.tick();
        let (shares, status) = match list_shares(state, &name) {
            Ok(shares) => (Some(shares), Some(StatusReport::connected())),
            Err(report) => (None, Some(report)),
        };
        result.servers.push(DiscoveredServer {
            path: format!(r"\\{name}"),
            name,
            announced,
            shares,
            status,
        });
        ctx.progress().done += 1;
        ctx.tick();
    }
    result
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectOutcome {
    /// Canonical path that was requested (open this one).
    pub path: String,
    pub share_root: String,
    pub report: StatusReport,
    /// Nexdir created the connection (and may disconnect it later).
    pub managed: bool,
    /// Windows already had a connection to this share; Nexdir did not change it.
    pub existing: bool,
}

/// Connects to a share (or a folder in a share). Without `prompt_owner` the signed-in Windows
/// user is used; with it, Windows shows its own sign-in dialog. Existing connections are never
/// replaced or changed.
pub fn connect(state: &NetworkState, input: &str, prompt_owner: Option<isize>) -> AppResult<ConnectOutcome> {
    let p = unc::parse(input)?;
    let share_root = p.share_root().ok_or_else(|| {
        AppError::new(ErrorCode::InvalidPath, "Choose a share on this server, for example \\\\NAS\\Media.").with_path(input)
    })?;
    let path = p.display();
    let api = state.api.clone();
    let root = share_root.clone();
    let existing = match state.gate.run(LOCAL_TIMEOUT, move || api.connected()) {
        Ok(Ok(list)) => list
            .iter()
            .any(|r| r.remote.eq_ignore_ascii_case(&root) || r.remote.to_lowercase() == root.to_lowercase()),
        _ => false,
    } || state.registry.contains(&share_root);

    let mut managed = state.registry.contains(&share_root);
    if !existing {
        let api = state.api.clone();
        let root = share_root.clone();
        let timeout = if prompt_owner.is_some() { PROMPT_TIMEOUT } else { SERVER_TIMEOUT };
        match state.gate.run(timeout, move || api.add_connection(&root, prompt_owner)) {
            Ok(Ok(())) => {
                state.registry.insert(&share_root);
                managed = true;
            }
            Ok(Err(code)) => {
                return Ok(ConnectOutcome {
                    path,
                    share_root,
                    report: status::from_code(code),
                    managed: false,
                    existing: false,
                })
            }
            Err(e) => {
                return Ok(ConnectOutcome {
                    path,
                    share_root,
                    report: gate_report(e),
                    managed: false,
                    existing: false,
                })
            }
        }
    }
    // The share is connected; check that the requested folder exists and is accessible.
    let report = probe(state, &path)?;
    Ok(ConnectOutcome {
        path,
        share_root,
        report,
        managed,
        existing,
    })
}

/// Disconnects a connection that Nexdir created. Never forced: open files keep it alive.
pub fn disconnect(state: &NetworkState, input: &str) -> AppResult<StatusReport> {
    let p = unc::parse(input)?;
    let root = p
        .share_root()
        .ok_or_else(|| AppError::new(ErrorCode::InvalidPath, "Choose a share.").with_path(input))?;
    if !state.registry.contains(&root) {
        return Err(AppError::new(
            ErrorCode::Unsupported,
            "Nexdir only disconnects connections it created itself. Windows network drives and other connections stay as they are.",
        )
        .with_path(&root));
    }
    let api = state.api.clone();
    let r = root.clone();
    Ok(match state.gate.run(SERVER_TIMEOUT, move || api.cancel_connection(&r)) {
        Ok(Ok(())) => {
            state.registry.remove(&root);
            StatusReport {
                status: NetworkStatus::NotConnected,
                message: "Disconnected.".into(),
                code: None,
            }
        }
        Ok(Err(status::ERROR_NOT_CONNECTED)) => {
            state.registry.remove(&root);
            status::from_code(status::ERROR_NOT_CONNECTED)
        }
        Ok(Err(code)) => status::from_code(code),
        Err(e) => gate_report(e),
    })
}

/// Releases every connection Nexdir created (at exit). Not forced and bounded in time: a
/// connection that is still in use, or a server that does not answer, is left to Windows.
pub fn release_managed(state: &NetworkState, timeout: Duration) {
    for root in state.registry.all() {
        let api = state.api.clone();
        let r = root.clone();
        if let Ok(Ok(())) = state.gate.run(timeout, move || api.cancel_connection(&r)) {
            state.registry.remove(&root);
        }
    }
}

#[cfg(test)]
mod samba_tests;
#[cfg(test)]
mod smb_tests;
#[cfg(test)]
mod tests;
