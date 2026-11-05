//! Logic tests with a test double for the Windows network functions (no NAS needed).

use super::*;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

fn res(remote: &str, local: Option<&str>, kind: ResourceKind, container: bool) -> NetResource {
    NetResource {
        remote: remote.into(),
        local: local.map(str::to_string),
        kind,
        container,
        comment: None,
    }
}

#[derive(Default)]
struct FakeNet {
    connected: Mutex<Vec<NetResource>>,
    remembered: Vec<NetResource>,
    /// Children per container (`""` = top level).
    tree: BTreeMap<String, Result<Vec<NetResource>, u32>>,
    /// Result of add_connection without / with the sign-in dialog.
    connect_plain: Option<u32>,
    connect_prompt: Option<u32>,
    probe_error: Option<u32>,
    connected_error: Option<u32>,
    hang_probe: bool,
    add_calls: AtomicUsize,
    prompted: AtomicBool,
    cancelled: Mutex<Vec<String>>,
}

impl NetApi for FakeNet {
    fn connected(&self) -> Result<Vec<NetResource>, u32> {
        if let Some(code) = self.connected_error {
            return Err(code);
        }
        Ok(self.connected.lock().unwrap().clone())
    }
    fn remembered(&self) -> Result<Vec<NetResource>, u32> {
        Ok(self.remembered.clone())
    }
    fn enumerate(&self, container: Option<&NetResource>) -> Result<Vec<NetResource>, u32> {
        let key = container.map(|c| c.remote.to_lowercase()).unwrap_or_default();
        self.tree.get(&key).cloned().unwrap_or(Ok(vec![]))
    }
    fn add_connection(&self, share_root: &str, prompt_owner: Option<isize>) -> Result<(), u32> {
        self.add_calls.fetch_add(1, Ordering::SeqCst);
        let outcome = if prompt_owner.is_some() {
            self.prompted.store(true, Ordering::SeqCst);
            self.connect_prompt
        } else {
            self.connect_plain
        };
        match outcome {
            Some(code) => Err(code),
            None => {
                self.connected
                    .lock()
                    .unwrap()
                    .push(res(share_root, None, ResourceKind::Share, false));
                Ok(())
            }
        }
    }
    fn cancel_connection(&self, share_root: &str) -> Result<(), u32> {
        self.cancelled.lock().unwrap().push(share_root.to_string());
        Ok(())
    }
    fn probe(&self, _path: &str) -> Result<(), u32> {
        if self.hang_probe {
            std::thread::sleep(Duration::from_secs(3));
        }
        self.probe_error.map_or(Ok(()), Err)
    }
}

fn state(fake: FakeNet) -> (NetworkState, Arc<FakeNet>) {
    static GATE: Gate = Gate::new(16);
    let fake = Arc::new(fake);
    let s = NetworkState {
        api: fake.clone(),
        gate: &GATE,
        registry: Registry::default(),
    };
    (s, fake)
}

#[test]
fn merges_drives_connections_remembered_and_favorites_without_duplicates() {
    let connected = vec![
        res(r"\\NAS-WOHNZIMMER\Medien", Some("Z:"), ResourceKind::Share, false),
        res(r"\\nas-wohnzimmer\medien", None, ResourceKind::Share, false),
        res(r"\\backup-server\Backups", None, ResourceKind::Share, false),
    ];
    let remembered = vec![
        res(r"\\NAS-WOHNZIMMER\Medien", Some("Z:"), ResourceKind::Share, false),
        res(r"\\homeserver\Dokumente", Some("x:"), ResourceKind::Share, false),
    ];
    let favs = vec![
        favorites::Favorite {
            path: r"\\nas-wohnzimmer\Medien".into(),
            label: "Home NAS".into(),
        },
        favorites::Favorite {
            path: r"\\offline-nas\Archiv".into(),
            label: String::new(),
        },
    ];
    let list = merge(&connected, &remembered, &favs, &Registry::default());
    assert_eq!(list.len(), 4, "{list:#?}");
    let medien = list
        .iter()
        .find(|l| l.path.eq_ignore_ascii_case(r"\\NAS-WOHNZIMMER\Medien"))
        .unwrap();
    assert_eq!(medien.drive.as_deref(), Some("Z:"));
    assert_eq!(medien.label, "Home NAS");
    assert_eq!(medien.status, Some(NetworkStatus::Connected));
    assert!(medien.sources.contains(&Source::Drive) && medien.sources.contains(&Source::Favorite));
    let doks = list.iter().find(|l| l.path == r"\\homeserver\Dokumente").unwrap();
    assert_eq!(doks.status, Some(NetworkStatus::NotConnected));
    assert_eq!(doks.drive.as_deref(), Some("X:"));
    let offline = list.iter().find(|l| l.path == r"\\offline-nas\Archiv").unwrap();
    assert_eq!(offline.status, None, "favourites are not checked automatically");
    assert_eq!(offline.label, "Archiv");
    assert!(list.iter().all(|l| !l.managed));
}

#[test]
fn ignores_invalid_or_server_only_entries_from_windows() {
    let connected = vec![
        res(r"\\.\pipe\x", None, ResourceKind::Share, false),
        res(r"\\server", None, ResourceKind::Server, true),
    ];
    assert!(merge(&connected, &[], &[], &Registry::default()).is_empty());
}

#[test]
fn connects_as_the_windows_user_and_registers_the_connection() {
    let (s, f) = state(FakeNet::default());
    let out = connect(&s, r"\\NAS\Medien\Filme", None).unwrap();
    assert_eq!(out.path, r"\\NAS\Medien\Filme");
    assert_eq!(out.share_root, r"\\NAS\Medien");
    assert_eq!(out.report.status, NetworkStatus::Connected);
    assert!(out.managed && !out.existing);
    assert!(!f.prompted.load(Ordering::SeqCst), "no sign-in dialog unless asked for");
    assert!(s.registry.contains(r"\\nas\medien"));
}

#[test]
fn reuses_an_existing_windows_connection_without_touching_it() {
    let f = FakeNet::default();
    f.connected
        .lock()
        .unwrap()
        .push(res(r"\\nas\medien", Some("Z:"), ResourceKind::Share, false));
    let (s, f) = state(f);
    let out = connect(&s, r"\\NAS\Medien", None).unwrap();
    assert!(out.existing && !out.managed);
    assert_eq!(f.add_calls.load(Ordering::SeqCst), 0);
    // Nexdir did not create it, so it refuses to disconnect it.
    let err = disconnect(&s, r"\\NAS\Medien").unwrap_err();
    assert!(err.message.contains("only disconnects connections it created"));
    assert!(f.cancelled.lock().unwrap().is_empty());
}

#[test]
fn reports_sign_in_required_and_then_uses_the_windows_dialog() {
    let (s, f) = state(FakeNet {
        connect_plain: Some(status::ERROR_LOGON_FAILURE),
        ..Default::default()
    });
    let out = connect(&s, r"\\NAS\Privat", None).unwrap();
    assert_eq!(out.report.status, NetworkStatus::SignInRequired);
    assert!(!out.managed);
    assert!(!s.registry.contains(r"\\NAS\Privat"));
    let out = connect(&s, r"\\NAS\Privat", Some(1)).unwrap();
    assert!(f.prompted.load(Ordering::SeqCst));
    assert_eq!(out.report.status, NetworkStatus::Connected);
    assert!(out.managed);
}

#[test]
fn explains_conflicts_cancelled_sign_in_and_policy_blocks() {
    for (code, expected) in [
        (status::ERROR_SESSION_CREDENTIAL_CONFLICT, NetworkStatus::CredentialConflict),
        (status::ERROR_CANCELLED, NetworkStatus::Cancelled),
        (status::ERROR_SMB_GUEST_LOGON_BLOCKED, NetworkStatus::BlockedByPolicy),
        (status::ERROR_BAD_NETPATH, NetworkStatus::Unreachable),
        (status::ERROR_BAD_NET_NAME, NetworkStatus::NotFound),
    ] {
        let (s, _f) = state(FakeNet {
            connect_plain: Some(code),
            connect_prompt: Some(code),
            ..Default::default()
        });
        let out = connect(&s, r"\\NAS\x", Some(1)).unwrap();
        assert_eq!(out.report.status, expected, "code {code}");
        assert_eq!(out.report.code, Some(code));
        assert!(!out.managed && s.registry.managed.lock().unwrap().is_empty());
    }
}

#[test]
fn refuses_servers_and_invalid_paths_before_any_network_call() {
    let (s, f) = state(FakeNet::default());
    for input in [r"\\NAS", r"C:\Windows", r"\\.\pipe\x", r"\\nas\share\..\x", r"\\nas\IPC$"] {
        assert!(connect(&s, input, None).is_err(), "{input}");
    }
    assert_eq!(f.add_calls.load(Ordering::SeqCst), 0);
}

#[test]
fn does_not_claim_a_connection_when_windows_cannot_list_connections() {
    // Found in the security review: an unknown state must not make Nexdir the owner.
    let (s, f) = state(FakeNet {
        connected_error: Some(1222),
        ..Default::default()
    });
    let out = connect(&s, r"\\NAS\Medien", None).unwrap();
    assert_eq!(out.report.status, NetworkStatus::Connected);
    assert!(!out.managed);
    assert!(s.registry.all().is_empty());
    assert!(disconnect(&s, r"\\NAS\Medien").is_err());
    release_managed(&s, Duration::from_secs(1));
    assert!(f.cancelled.lock().unwrap().is_empty());
}

#[test]
fn disconnects_only_its_own_connections() {
    let (s, f) = state(FakeNet::default());
    connect(&s, r"\\NAS\Medien", None).unwrap();
    let r = disconnect(&s, r"\\nas\medien\sub").unwrap();
    assert_eq!(r.status, NetworkStatus::NotConnected);
    assert_eq!(*f.cancelled.lock().unwrap(), vec![r"\\nas\medien".to_string()]);
    assert!(disconnect(&s, r"\\NAS\Medien").is_err(), "already removed from the registry");
}

#[test]
fn releases_only_its_own_connections_at_exit() {
    let f = FakeNet::default();
    f.connected
        .lock()
        .unwrap()
        .push(res(r"\\other\drive", Some("Y:"), ResourceKind::Share, false));
    let (s, f) = state(f);
    connect(&s, r"\\NAS\A", None).unwrap();
    connect(&s, r"\\NAS\B\x", None).unwrap();
    release_managed(&s, Duration::from_secs(1));
    let mut cancelled = f.cancelled.lock().unwrap().clone();
    cancelled.sort();
    assert_eq!(cancelled, vec![r"\\nas\a".to_string(), r"\\nas\b".to_string()]);
    assert!(s.registry.all().is_empty());
}

#[test]
fn a_hanging_server_times_out_instead_of_blocking() {
    static SLOW: Gate = Gate::new(4);
    let s = NetworkState {
        api: Arc::new(FakeNet {
            hang_probe: true,
            ..Default::default()
        }),
        gate: &SLOW,
        registry: Registry::default(),
    };
    // probe() uses SERVER_TIMEOUT; run the call through a short gate wait instead.
    let api = s.api.clone();
    let t0 = std::time::Instant::now();
    let r = s.gate.run(Duration::from_millis(200), move || api.probe(r"\\nas\x"));
    assert_eq!(r, Err(GateError::TimedOut));
    assert!(t0.elapsed() < Duration::from_secs(2));
    assert_eq!(gate_report(GateError::TimedOut).status, NetworkStatus::TimedOut);
}

#[test]
fn probe_maps_windows_errors() {
    let (s, _f) = state(FakeNet {
        probe_error: Some(status::ERROR_ACCESS_DENIED),
        ..Default::default()
    });
    assert_eq!(probe(&s, r"\\nas\x").unwrap().status, NetworkStatus::AccessDenied);
    assert!(probe(&s, r"\\nas").is_err());
}

fn discovery_net() -> FakeNet {
    let mut tree = BTreeMap::new();
    tree.insert(
        String::new(),
        Ok(vec![res("Microsoft Windows Network", None, ResourceKind::Network, true)]),
    );
    tree.insert(
        "microsoft windows network".into(),
        Ok(vec![res("WORKGROUP", None, ResourceKind::Domain, true)]),
    );
    tree.insert(
        "workgroup".into(),
        Ok(vec![
            res(r"\\NAS-WOHNZIMMER", None, ResourceKind::Server, true),
            res(r"\\PRINTER-BOX", None, ResourceKind::Server, true),
        ]),
    );
    tree.insert(
        r"\\nas-wohnzimmer".into(),
        Ok(vec![
            res(r"\\NAS-WOHNZIMMER\Medien", None, ResourceKind::Share, false),
            res(r"\\NAS-WOHNZIMMER\Backups", None, ResourceKind::Share, false),
            res(r"\\NAS-WOHNZIMMER\ADMIN$", None, ResourceKind::Share, false),
            res(r"\\NAS-WOHNZIMMER\medien", None, ResourceKind::Share, false),
        ]),
    );
    tree.insert(r"\\printer-box".into(), Err(status::ERROR_ACCESS_DENIED));
    tree.insert(
        r"\\homeserver".into(),
        Ok(vec![res(r"\\homeserver\Dokumente", None, ResourceKind::Share, false)]),
    );
    FakeNet {
        tree,
        ..Default::default()
    }
}

fn run_discovery(s: &NetworkState, known: &[String], cancel_after_start: bool) -> Discovery {
    let cancel = AtomicBool::new(cancel_after_start);
    let emit = |_: &crate::tasks::TaskProgress| {};
    let mut ctx = TaskContext::new(1, crate::tasks::TaskKind::NetworkDiscovery, &cancel, &emit);
    discover(s, known, &mut ctx)
}

#[test]
fn discovers_servers_and_lists_only_real_shares() {
    let (s, _f) = state(discovery_net());
    let d = run_discovery(&s, &["homeserver".into(), "nas-wohnzimmer".into()], false);
    assert!(d.network_list_available && !d.cancelled);
    assert_eq!(d.servers.len(), 3, "{:#?}", d.servers);
    let nas = d.servers.iter().find(|s| s.name.eq_ignore_ascii_case("NAS-WOHNZIMMER")).unwrap();
    assert!(nas.announced);
    let names: Vec<_> = nas.shares.as_ref().unwrap().iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, vec!["Backups", "Medien"], "hidden $ shares and duplicates are left out");
    let printer = d.servers.iter().find(|s| s.name == "PRINTER-BOX").unwrap();
    assert!(printer.shares.is_none(), "no invented shares when the list cannot be read");
    assert_eq!(printer.status.as_ref().unwrap().status, NetworkStatus::AccessDenied);
    let home = d.servers.iter().find(|s| s.name == "homeserver").unwrap();
    assert!(!home.announced);
    assert_eq!(home.shares.as_ref().unwrap()[0].path, r"\\homeserver\Dokumente");
}

#[test]
fn discovery_without_a_network_list_still_checks_known_servers() {
    let mut net = discovery_net();
    net.tree.remove("");
    let (s, _f) = state(net);
    let d = run_discovery(&s, &["homeserver".into()], false);
    assert!(!d.network_list_available);
    assert_eq!(d.servers.len(), 1);
}

#[test]
fn discovery_stops_when_cancelled() {
    let (s, _f) = state(discovery_net());
    let d = run_discovery(&s, &["homeserver".into()], true);
    assert!(d.cancelled);
    assert!(d.servers.is_empty());
}

#[test]
fn remote_detection_for_unc_and_local_paths() {
    assert!(is_remote(Path::new(r"\\nas\share\x")));
    assert!(is_remote(Path::new(r"\\?\UNC\nas\share\x")));
    let system = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
    assert!(!is_remote(Path::new(&format!(r"{system}\Windows"))));
    assert!(!is_remote(Path::new(r"\\.\pipe\x")));
}
