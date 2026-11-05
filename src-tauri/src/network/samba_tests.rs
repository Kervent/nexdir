//! Acceptance tests against a real Samba server in a dedicated, isolated WSL distro
//! (see docs/testing/network-storage-tests.md, "Samba test server"). Ignored by default and
//! skipped unless `NEXDIR_SAMBA_SERVER` names the server. Credentials never pass through these
//! tests: Windows supplies them from its Credential Manager (set by the test operator), exactly as
//! for a user who saved them in Windows' sign-in dialog.
//!
//!     $env:NEXDIR_SAMBA_SERVER = "172.29.37.90"
//!     cargo test --lib samba_<phase> -- --ignored --test-threads=1 --nocapture
//!
//! Phases: `samba_nocred_*` (no stored credentials), `samba_user_*` (credentials of `nxtest`),
//! `samba_outage_*` (stops/starts smbd in the test distro), `samba_slow_*` (netem in the distro).
//! Integrity is checked with SHA-256 on the client and, independently, on the server's disk.

use super::*;
use crate::filesystem;
use crate::operations::transfer::{
    self, ConflictPolicy, OperationProgress, OperationReport, TransferKind, TransferRequest, NETWORK_MOVE_REFUSED,
};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

const DISTRO: &str = "nexdir-samba-test";

fn server() -> Option<String> {
    let s = std::env::var("NEXDIR_SAMBA_SERVER").ok().filter(|s| !s.is_empty());
    if s.is_none() {
        eprintln!("SKIPPED: NEXDIR_SAMBA_SERVER is not set");
    }
    s
}

fn share(srv: &str, name: &str) -> PathBuf {
    PathBuf::from(format!(r"\\{srv}\{name}"))
}

/// Runs a command as root inside the test distro (and only there).
fn in_vm(cmd: &str) -> String {
    let out = Command::new("wsl.exe")
        .args(["-d", DISTRO, "-u", "root", "--", "bash", "-c", cmd])
        .output()
        .expect("wsl.exe");
    String::from_utf8_lossy(&out.stdout).replace('\0', "").trim().to_string()
}

fn server_sha256(path_in_vm: &str) -> String {
    in_vm(&format!("sha256sum '{path_in_vm}' | cut -d' ' -f1"))
}

fn local_sha256(path: &Path) -> String {
    let out = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!("(Get-FileHash -Algorithm SHA256 -LiteralPath '{}').Hash.ToLower()", path.display()),
        ])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn run(kind: TransferKind, sources: Vec<PathBuf>, destination: &Path, policy: ConflictPolicy) -> OperationReport {
    run_with(kind, sources, destination, policy, &AtomicBool::new(false), &mut |_| {})
}

fn run_with(
    kind: TransferKind,
    sources: Vec<PathBuf>,
    destination: &Path,
    policy: ConflictPolicy,
    cancel: &AtomicBool,
    on_update: &mut dyn FnMut(&OperationProgress),
) -> OperationReport {
    let req = TransferRequest {
        kind,
        sources,
        destination: destination.to_path_buf(),
        policy,
    };
    transfer::run_for_test(&req, cancel, on_update)
}

fn random_file(path: &Path, bytes: usize, seed: u8) {
    let mut data = vec![0u8; bytes];
    let mut x: u32 = 0x9e37_79b9 ^ seed as u32;
    for b in data.iter_mut() {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        *b = x as u8;
    }
    fs::write(path, data).unwrap();
}

fn partials(dir_in_vm: &str) -> String {
    in_vm(&format!("ls -A '{dir_in_vm}' | grep '^~nexdir-' || true"))
}

/// Fresh, empty working folder on the server under a unique name. It is created through SMB
/// (like a user would), so the Windows client's directory cache knows it; old runs are removed
/// inside the VM, never through Nexdir.
fn server_dir(srv: &str, share_name: &str, vm_root: &str, name: &str) -> (PathBuf, String) {
    let unique = format!(
        "{name}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    );
    in_vm(&format!("rm -rf '{vm_root}/{name}'-*"));
    let unc = share(srv, share_name).join(&unique);
    fs::create_dir(&unc).unwrap_or_else(|e| panic!("create {}: {e}", unc.display()));
    (unc, format!("{vm_root}/{unique}"))
}

// ---------------------------------------------------------------------------------------------
// Phase: no stored credentials (Windows tries the signed-in account, which the server rejects).

#[test]
#[ignore]
fn samba_nocred_sign_in_is_required_and_explained() {
    let Some(srv) = server() else { return };
    let state = NetworkState::windows();
    let out = connect(&state, &format!(r"\\{srv}\PublicTest"), None).unwrap();
    eprintln!("connect without credentials: {:?} code {:?}", out.report.status, out.report.code);
    assert!(
        matches!(out.report.status, NetworkStatus::SignInRequired | NetworkStatus::AccessDenied),
        "{out:?}"
    );
    assert!(!out.managed && state.registry.all().is_empty());
    let p = probe(&state, &format!(r"\\{srv}\PrivateTest")).unwrap();
    assert!(
        matches!(p.status, NetworkStatus::SignInRequired | NetworkStatus::AccessDenied),
        "{p:?}"
    );
    match list_shares(&state, &srv) {
        Ok(s) => eprintln!("anonymous share list: {} shares (server allows it)", s.len()),
        Err(r) => eprintln!("anonymous share list refused: {:?} code {:?}", r.status, r.code),
    }
}

#[test]
#[ignore]
fn samba_nocred_unknown_hosts_and_invalid_paths() {
    let Some(srv) = server() else { return };
    let state = NetworkState::windows();
    // An address in the same virtual subnet without a host.
    let prefix = srv.rsplit_once('.').map(|(a, _)| a.to_string()).unwrap();
    let t0 = Instant::now();
    let r = probe(&state, &format!(r"\\{prefix}.254\PublicTest")).unwrap();
    eprintln!("no host at {prefix}.254: {:?} code {:?} after {:?}", r.status, r.code, t0.elapsed());
    assert!(matches!(r.status, NetworkStatus::Unreachable | NetworkStatus::TimedOut), "{r:?}");
    assert!(t0.elapsed() < SERVER_TIMEOUT + Duration::from_secs(2), "bounded by the gate");
    let t0 = Instant::now();
    let r = probe(&state, r"\\nexdir-no-such-host.invalid\x").unwrap();
    eprintln!("unknown name: {:?} after {:?}", r.status, t0.elapsed());
    assert_eq!(r.status, NetworkStatus::Unreachable);
    for bad in [
        format!(r"\\{srv}"),
        format!(r"\\{srv}\IPC$"),
        format!(r"\\{srv}\PublicTest\..\x"),
        r"\\.\pipe\x".to_string(),
    ] {
        assert!(connect(&state, &bad, None).is_err(), "{bad}");
    }
}

// ---------------------------------------------------------------------------------------------
// Phase: Windows has the credentials of `nxtest` (Credential Manager, session-only).

#[test]
#[ignore]
fn samba_user_connect_shares_status_and_discovery() {
    let Some(srv) = server() else { return };
    let state = NetworkState::windows();
    let out = connect(&state, &format!(r"\\{srv}\PublicTest"), None).unwrap();
    eprintln!(
        "connect PublicTest: {:?} managed={} existing={}",
        out.report.status, out.managed, out.existing
    );
    assert_eq!(out.report.status, NetworkStatus::Connected);
    let shares = list_shares(&state, &srv).expect("share list");
    let names: Vec<_> = shares.iter().map(|s| s.name.as_str()).collect();
    eprintln!("shares: {names:?}");
    for n in ["PublicTest", "PrivateTest", "ReadOnlyTest", "LargeFiles"] {
        assert!(names.contains(&n), "{n} missing");
    }
    assert!(!names.iter().any(|n| n.ends_with('$')));
    // nxtest is not allowed on PrivateTest.
    let p = probe(&state, &format!(r"\\{srv}\PrivateTest")).unwrap();
    eprintln!("PrivateTest as nxtest: {:?} code {:?}", p.status, p.code);
    assert!(
        matches!(p.status, NetworkStatus::AccessDenied | NetworkStatus::SignInRequired),
        "{p:?}"
    );
    assert_eq!(
        probe(&state, &format!(r"\\{srv}\NoSuchShare")).unwrap().status,
        NetworkStatus::NotFound
    );
    assert_eq!(
        probe(&state, &format!(r"\\{srv}\PublicTest\no-such-folder")).unwrap().status,
        NetworkStatus::NotFound
    );
    // Discovery with the server known from the connection.
    let overview = overview(&state, &[]);
    assert!(
        overview
            .locations
            .iter()
            .any(|l| l.path.eq_ignore_ascii_case(&format!(r"\\{srv}\PublicTest"))),
        "{overview:?}"
    );
    let cancel = AtomicBool::new(false);
    let emit = |_: &crate::tasks::TaskProgress| {};
    let mut ctx = TaskContext::new(1, crate::tasks::TaskKind::NetworkDiscovery, &cancel, &emit);
    let known: Vec<String> = overview.locations.iter().map(|l| l.server.clone()).collect();
    let d = discover(&state, &known, &mut ctx);
    eprintln!(
        "discovery: network list {}, servers {:?}",
        d.network_list_available,
        d.servers
            .iter()
            .map(|s| (&s.name, s.announced, s.shares.as_ref().map(|x| x.len())))
            .collect::<Vec<_>>()
    );
    assert!(d
        .servers
        .iter()
        .any(|s| s.name == srv && s.shares.as_ref().is_some_and(|x| x.len() >= 4)));
    let r = disconnect(&state, &format!(r"\\{srv}\PublicTest")).unwrap();
    assert_eq!(r.status, NetworkStatus::NotConnected, "{r:?}");
}

#[test]
#[ignore]
fn samba_user_file_operations_with_sha256_on_both_sides() {
    let Some(srv) = server() else { return };
    let (dir, vm) = server_dir(&srv, "PublicTest", "/srv/nexdir-test/public", "ops");
    let local = tempfile::tempdir().unwrap();
    let src = local.path().join("src");
    fs::create_dir_all(src.join("tree/sub 日本")).unwrap();
    random_file(&src.join("tree/data.bin"), 32 * 1024 * 1024, 1);
    random_file(&src.join("tree/sub 日本/Résumé 😀.bin"), 300_000, 2);
    fs::write(src.join("small.txt"), "hello samba").unwrap();

    // Listing with Unicode and metadata.
    let listing = filesystem::list_dir(&share(&srv, "PublicTest"), false).unwrap();
    assert!(
        listing.entries.iter().any(|e| e.name == "Ünïcödé 日本 😀"),
        "{:?}",
        listing.entries.iter().map(|e| &e.name).collect::<Vec<_>>()
    );

    // New folder and rename (file and folder) on the share.
    let created = crate::operations::create_folder(&dir, "Neuer Ordner").unwrap();
    crate::operations::rename_entry(&created, "Umbenannt Ü").unwrap();
    assert_eq!(in_vm(&format!("test -d '{vm}/Umbenannt Ü' && echo yes")), "yes");

    // Windows -> Samba (folder with a 32 MiB file and Unicode names).
    let r = run(TransferKind::Copy, vec![src.join("tree")], &dir, ConflictPolicy::Skip);
    assert!(r.failures.is_empty(), "{r:?}");
    assert_eq!(
        server_sha256(&format!("{vm}/tree/data.bin")),
        local_sha256(&src.join("tree/data.bin"))
    );
    assert_eq!(
        server_sha256(&format!("{vm}/tree/sub 日本/Résumé 😀.bin")),
        local_sha256(&src.join("tree/sub 日本/Résumé 😀.bin"))
    );
    assert_eq!(partials(&format!("{vm}/tree")), "", "no temporary files left");
    crate::operations::rename_entry(&dir.join("tree").join("data.bin"), "data renamed.bin").unwrap();

    // Samba -> Windows.
    let back = local.path().join("back");
    fs::create_dir(&back).unwrap();
    let r = run(TransferKind::Copy, vec![dir.join("tree")], &back, ConflictPolicy::Skip);
    assert!(r.failures.is_empty(), "{r:?}");
    assert_eq!(
        local_sha256(&back.join("tree/data renamed.bin")),
        local_sha256(&src.join("tree/data.bin"))
    );

    // Share A -> share B (PublicTest -> LargeFiles) and folder A -> folder B inside a share.
    let (large_dir, large_vm) = server_dir(&srv, "LargeFiles", "/srv/nexdir-test/large", "from-public");
    let r = run(TransferKind::Copy, vec![dir.join("tree")], &large_dir, ConflictPolicy::Skip);
    assert!(r.failures.is_empty(), "{r:?}");
    assert_eq!(
        server_sha256(&format!("{large_vm}/tree/data renamed.bin")),
        local_sha256(&src.join("tree/data.bin"))
    );
    let inner = crate::operations::create_folder(&dir, "B").unwrap();
    let r = run(TransferKind::Copy, vec![dir.join("tree")], &inner, ConflictPolicy::Skip);
    assert!(r.failures.is_empty(), "{r:?}");

    // Conflicts: Skip keeps, KeepBoth adds a name, Replace swaps in a verified copy.
    fs::write(src.join("small.txt"), "version 2").unwrap();
    run(TransferKind::Copy, vec![src.join("small.txt")], &dir, ConflictPolicy::Skip);
    fs::write(src.join("small.txt"), "version 3").unwrap();
    let r = run(TransferKind::Copy, vec![src.join("small.txt")], &dir, ConflictPolicy::Skip);
    assert_eq!(r.skipped.len(), 1);
    assert_eq!(in_vm(&format!("cat '{vm}/small.txt'")), "version 2");
    run(TransferKind::Copy, vec![src.join("small.txt")], &dir, ConflictPolicy::KeepBoth);
    assert_eq!(in_vm(&format!("cat '{vm}/small (2).txt'")), "version 3");
    let r = run(TransferKind::Copy, vec![src.join("small.txt")], &dir, ConflictPolicy::Replace);
    assert!(r.failures.is_empty(), "{r:?}");
    assert_eq!(in_vm(&format!("cat '{vm}/small.txt'")), "version 3");
    assert_eq!(partials(&vm), "");

    // Moves: inside the share = rename (allowed); across shares or to/from Windows = refused.
    let r = run(TransferKind::Move, vec![dir.join("small (2).txt")], &inner, ConflictPolicy::Skip);
    assert!(r.failures.is_empty(), "{r:?}");
    assert_eq!(in_vm(&format!("cat '{vm}/B/small (2).txt'")), "version 3");
    let r = run(TransferKind::Move, vec![dir.join("small.txt")], &large_dir, ConflictPolicy::Skip);
    assert_eq!(r.failures[0].message, NETWORK_MOVE_REFUSED);
    let r = run(TransferKind::Move, vec![dir.join("small.txt")], &back, ConflictPolicy::Skip);
    assert_eq!(r.failures[0].message, NETWORK_MOVE_REFUSED);
    let r = run(TransferKind::Move, vec![src.join("small.txt")], &dir, ConflictPolicy::Replace);
    assert_eq!(r.failures[0].message, NETWORK_MOVE_REFUSED);
    assert!(src.join("small.txt").exists());
    assert_eq!(in_vm(&format!("cat '{vm}/small.txt'")), "version 3");

    // Delete: refused, nothing removed.
    let rr = crate::operations::recycle_entries(&[dir.join("small.txt")], None).unwrap();
    assert!(rr.recycled.is_empty() && rr.failed[0].message.contains("no Recycle Bin"));
    assert_eq!(in_vm(&format!("test -f '{vm}/small.txt' && echo yes")), "yes");
}

#[test]
#[ignore]
fn samba_user_read_only_share_refuses_writes_cleanly() {
    let Some(srv) = server() else { return };
    let ro = share(&srv, "ReadOnlyTest");
    let listing = filesystem::list_dir(&ro, false).unwrap();
    assert!(listing.entries.iter().any(|e| e.name == "random-3MiB.bin"));
    let local = tempfile::tempdir().unwrap();
    let r = run(
        TransferKind::Copy,
        vec![ro.join("random-3MiB.bin")],
        local.path(),
        ConflictPolicy::Skip,
    );
    assert!(r.failures.is_empty(), "{r:?}");
    assert_eq!(
        local_sha256(&local.path().join("random-3MiB.bin")),
        server_sha256("/srv/nexdir-test/readonly/random-3MiB.bin")
    );
    let before = in_vm("cd /srv/nexdir-test/readonly && find . -type f -exec sha256sum {} + | sort");

    let e = crate::operations::create_folder(&ro, "nope").unwrap_err();
    eprintln!("create folder: {:?} {}", e.code, e.message);
    assert_eq!(e.code, crate::error::ErrorCode::PermissionDenied);
    let e = crate::operations::rename_entry(&ro.join("docs").join("file1.txt"), "x.txt").unwrap_err();
    eprintln!("rename: {:?} {}", e.code, e.message);
    fs::write(local.path().join("file1.txt"), "overwrite attempt").unwrap();
    let r = run(
        TransferKind::Copy,
        vec![local.path().join("file1.txt")],
        &ro.join("docs"),
        ConflictPolicy::Replace,
    );
    eprintln!("overwrite: {:?}", r.failures);
    assert_eq!(r.failures.len(), 1);
    fs::write(local.path().join("new.txt"), "new").unwrap();
    let r = run(TransferKind::Copy, vec![local.path().join("new.txt")], &ro, ConflictPolicy::Skip);
    eprintln!("copy into read-only share: {:?}", r.failures);
    assert_eq!(r.failures.len(), 1);
    let after = in_vm("cd /srv/nexdir-test/readonly && find . -type f -exec sha256sum {} + | sort");
    assert_eq!(before, after, "read-only share unchanged, no partial files");
}

#[test]
#[ignore]
fn samba_user_large_and_many_files_and_deep_long_paths() {
    let Some(srv) = server() else { return };
    let (dir, vm) = server_dir(&srv, "LargeFiles", "/srv/nexdir-test/large", "perf");
    // Many small files and a deep tree with long names are created on the server side.
    for n in [1_000, 10_000, 20_000] {
        in_vm(&format!("mkdir -p '{vm}/n{n}' && cd '{vm}/n{n}' && seq -w 1 {n} | xargs -I{{}} sh -c 'echo x > f{{}}.txt' && chown -R nxtest '{vm}/n{n}'"));
        let t0 = Instant::now();
        let l = filesystem::list_dir(&dir.join(format!("n{n}")), false).unwrap();
        eprintln!("PERF list {n} files over SMB (virtual network): {} ms", t0.elapsed().as_millis());
        assert_eq!(l.entries.len(), n);
    }
    let long_name = format!("{}.txt", "L".repeat(200));
    let deep: String = (0..30).map(|i| format!("ebene-{i:02}-Ü")).collect::<Vec<_>>().join("/");
    in_vm(&format!(
        "mkdir -p '{vm}/deep/{deep}' && echo deep > '{vm}/deep/{deep}/{long_name}' && chown -R nxtest '{vm}/deep'"
    ));
    let unc_deep = dir.join("deep").join(deep.replace('/', "\\")).join(&long_name);
    eprintln!("deep path length: {} characters", unc_deep.as_os_str().len());
    assert!(unc_deep.as_os_str().len() > 260);
    let local = tempfile::tempdir().unwrap();
    let r = run(TransferKind::Copy, vec![dir.join("deep")], local.path(), ConflictPolicy::Skip);
    assert!(r.failures.is_empty(), "{r:?}");
    // Large file both ways (256 MiB).
    random_file(&local.path().join("big.bin"), 256 * 1024 * 1024, 7);
    let t0 = Instant::now();
    let r = run(TransferKind::Copy, vec![local.path().join("big.bin")], &dir, ConflictPolicy::Skip);
    let up = t0.elapsed().as_secs_f64();
    assert!(r.failures.is_empty(), "{r:?}");
    assert_eq!(server_sha256(&format!("{vm}/big.bin")), local_sha256(&local.path().join("big.bin")));
    let back = local.path().join("back");
    fs::create_dir(&back).unwrap();
    let t0 = Instant::now();
    let r = run(TransferKind::Copy, vec![dir.join("big.bin")], &back, ConflictPolicy::Skip);
    let down = t0.elapsed().as_secs_f64();
    assert!(r.failures.is_empty(), "{r:?}");
    assert_eq!(local_sha256(&back.join("big.bin")), local_sha256(&local.path().join("big.bin")));
    eprintln!(
        "PERF 256 MiB Windows -> Samba {up:.1} s ({:.0} MB/s), Samba -> Windows {down:.1} s ({:.0} MB/s)",
        256.0 / up,
        256.0 / down
    );
    // 1,000 small files Windows -> Samba.
    let many = local.path().join("many");
    fs::create_dir(&many).unwrap();
    for i in 0..1_000 {
        fs::write(many.join(format!("klein {i:04}.txt")), format!("{i}")).unwrap();
    }
    let t0 = Instant::now();
    let r = run(TransferKind::Copy, vec![many], &dir, ConflictPolicy::Skip);
    assert!(r.failures.is_empty(), "{r:?}");
    eprintln!("PERF 1,000 small files Windows -> Samba: {:.1} s", t0.elapsed().as_secs_f64());
    assert_eq!(in_vm(&format!("ls '{vm}/many' | wc -l")), "1000");
}

// ---------------------------------------------------------------------------------------------
// Phase: an existing connection as `nxprivate` (made by the test operator, not by Nexdir).

#[test]
#[ignore]
fn samba_existing_connection_is_reused_and_never_disconnected() {
    let Some(srv) = server() else { return };
    let state = NetworkState::windows();
    let private = format!(r"\\{srv}\PrivateTest");
    let out = connect(&state, &private, None).unwrap();
    eprintln!(
        "PrivateTest with the existing nxprivate connection: {:?} existing={} managed={}",
        out.report.status, out.existing, out.managed
    );
    assert_eq!(out.report.status, NetworkStatus::Connected);
    assert!(out.existing && !out.managed);
    let err = disconnect(&state, &private).unwrap_err();
    assert!(err.message.contains("only disconnects connections it created"));
    assert!(
        state
            .api
            .connected()
            .unwrap()
            .iter()
            .any(|r| r.remote.eq_ignore_ascii_case(&private)),
        "still connected"
    );
    // Windows uses the same account for the whole server: PublicTest opens as nxprivate.
    let out = connect(&state, &format!(r"\\{srv}\PublicTest"), None).unwrap();
    eprintln!("PublicTest on the same server: {:?} managed={}", out.report.status, out.managed);
    assert_eq!(out.report.status, NetworkStatus::Connected);
    release_managed(&state, Duration::from_secs(3));
    assert!(
        state
            .api
            .connected()
            .unwrap()
            .iter()
            .any(|r| r.remote.eq_ignore_ascii_case(&private)),
        "the operator's connection survives Nexdir's exit"
    );
}

/// Phase: a drive letter (`NEXDIR_SAMBA_DRIVE`, e.g. `X`) mapped to PublicTest by the operator.
#[test]
#[ignore]
fn samba_drive_letter_is_shown_once_and_works() {
    let Some(srv) = server() else { return };
    let Some(letter) = std::env::var("NEXDIR_SAMBA_DRIVE").ok() else {
        eprintln!("SKIPPED: NEXDIR_SAMBA_DRIVE not set");
        return;
    };
    let root = PathBuf::from(format!(r"{letter}:\"));
    let unc = share(&srv, "PublicTest");
    let state = NetworkState::windows();
    let o = overview(
        &state,
        &[favorites::Favorite {
            path: unc.display().to_string(),
            label: "Saved public".into(),
        }],
    );
    let entries: Vec<_> = o
        .locations
        .iter()
        .filter(|l| l.path.eq_ignore_ascii_case(&unc.display().to_string()))
        .collect();
    eprintln!(
        "overview entries for PublicTest: {:?}",
        entries.iter().map(|l| (&l.label, &l.drive, &l.sources)).collect::<Vec<_>>()
    );
    assert_eq!(entries.len(), 1, "drive, connection and favourite are one entry");
    assert_eq!(entries[0].drive.as_deref(), Some(format!("{letter}:").as_str()));
    assert!(is_remote(&root));
    let drives = crate::filesystem::drives::list_drives();
    let d = drives.iter().find(|d| d.letter == letter).expect("drive listed");
    eprintln!(
        "drive {letter}: kind {:?}, available {}, free {:?}",
        d.kind, d.available, d.free_bytes
    );
    assert_eq!(d.kind, crate::filesystem::drives::DriveKind::Network);

    let (dir, vm) = server_dir(&srv, "PublicTest", "/srv/nexdir-test/public", "drive");
    let via_letter = root.join(dir.file_name().unwrap());
    let listing = filesystem::list_dir(&via_letter, false).unwrap();
    assert!(listing.entries.is_empty());
    let local = tempfile::tempdir().unwrap();
    random_file(&local.path().join("x.bin"), 5_000_000, 5);
    let r = run(
        TransferKind::Copy,
        vec![local.path().join("x.bin")],
        &via_letter,
        ConflictPolicy::Skip,
    );
    assert!(r.failures.is_empty(), "{r:?}");
    assert_eq!(server_sha256(&format!("{vm}/x.bin")), local_sha256(&local.path().join("x.bin")));
    // Rename-move on the drive works. Drive letter <-> UNC of the same share: Windows treats it
    // as the same share, so the engine can rename (nothing deleted); the UI refuses it up front
    // (conservative). Either way the file must exist exactly once.
    let sub = crate::operations::create_folder(&via_letter, "sub").unwrap();
    let r = run(TransferKind::Move, vec![via_letter.join("x.bin")], &sub, ConflictPolicy::Skip);
    assert!(r.failures.is_empty(), "{r:?}");
    let r = run(TransferKind::Move, vec![sub.join("x.bin")], &dir, ConflictPolicy::Skip);
    eprintln!(
        "move drive letter -> UNC of the same share: {:?}",
        r.failures.iter().map(|f| &f.message).collect::<Vec<_>>()
    );
    assert!(
        sub.join("x.bin").exists() ^ dir.join("x.bin").exists(),
        "exactly one copy, nothing lost or duplicated"
    );
    if r.failures.is_empty() {
        assert_eq!(server_sha256(&format!("{vm}/x.bin")), local_sha256(&local.path().join("x.bin")));
    }
}

// ---------------------------------------------------------------------------------------------
// Phase: outages (smbd is stopped and started inside the test distro).

/// Shapes both directions of the VM's link (egress on eth0, ingress through ifb0). Only the
/// test distro's own interfaces are touched.
fn shape(netem: &str) {
    unshape();
    in_vm(&format!(
        "modprobe ifb numifbs=1; ip link add ifb0 type ifb 2>/dev/null; ip link set ifb0 up;          tc qdisc add dev eth0 root netem {netem}; tc qdisc add dev eth0 handle ffff: ingress;          tc filter add dev eth0 parent ffff: matchall action mirred egress redirect dev ifb0; tc qdisc add dev ifb0 root netem {netem}"
    ));
}

fn unshape() {
    in_vm(
        "tc qdisc del dev eth0 root 2>/dev/null; tc qdisc del dev eth0 ingress 2>/dev/null; tc qdisc del dev ifb0 root 2>/dev/null; true",
    );
}

fn smbd(action: &str) {
    in_vm(&format!("systemctl {action} smbd"));
}

/// After a restart, waits until the Windows client reaches the share again (at most 60 s).
fn wait_until_reachable(srv: &str) -> Duration {
    let t0 = Instant::now();
    while t0.elapsed() < Duration::from_secs(60) {
        if filesystem::list_dir(&share(srv, "LargeFiles"), false).is_ok() {
            return t0.elapsed();
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    panic!("share not reachable again within 60 s");
}

/// Copies `bytes` to the share and stops smbd once `at` (0..1) of the data was sent.
/// `at` < 1: stop smbd when that share of the data was handed to Windows; `at` >= 1: stop it
/// after `at` seconds (the late phase, while Windows flushes buffered data to the server).
fn copy_with_outage(srv: &str, name: &str, at: f64, existing: Option<&str>) -> (OperationReport, PathBuf, String, String) {
    wait_until_reachable(srv);
    let (dir, vm) = server_dir(srv, "LargeFiles", "/srv/nexdir-test/large", name);
    let local = tempfile::tempdir().unwrap().keep();
    let src = local.join("big.bin");
    random_file(&src, 512 * 1024 * 1024, 9);
    if let Some(content) = existing {
        in_vm(&format!("printf '{content}' > '{vm}/big.bin' && chown nxtest '{vm}/big.bin'"));
    }
    let total = 512.0 * 1024.0 * 1024.0;
    // Throttle the VM's link so the chosen point is really reached during the data transfer.
    shape("rate 300mbit");
    let mut stopped = at >= 1.0;
    let t0 = Instant::now();
    let timer = (at >= 1.0).then(|| {
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs_f64(at));
            eprintln!("  stopping smbd after {at} s (time-based)");
            smbd("stop");
        })
    });
    let r = run_with(
        TransferKind::Copy,
        vec![src.clone()],
        &dir,
        if existing.is_some() {
            ConflictPolicy::Replace
        } else {
            ConflictPolicy::Skip
        },
        &AtomicBool::new(false),
        &mut |p| {
            if !stopped && p.done_bytes as f64 >= total * at && (p.done_bytes as f64) < total {
                stopped = true;
                eprintln!(
                    "  stopping smbd at {:.0} % after {:?}",
                    100.0 * p.done_bytes as f64 / total,
                    t0.elapsed()
                );
                smbd("stop");
            }
        },
    );
    eprintln!(
        "  copy ended after {:?}: failures {:?}",
        t0.elapsed(),
        r.failures.iter().map(|f| &f.message).collect::<Vec<_>>()
    );
    if let Some(t) = timer {
        t.join().unwrap();
    }
    unshape();
    smbd("start");
    (r, src, vm, local.display().to_string())
}

#[test]
#[ignore]
fn samba_outage_during_copy_keeps_sources_and_never_completes_a_partial_file() {
    let Some(srv) = server() else { return };
    for (name, at) in [("outage-early", 0.3), ("outage-late", 12.0)] {
        eprintln!("outage {name}");
        let (r, src, vm, local) = copy_with_outage(&srv, name, at, None);
        assert!(
            src.exists() && fs::metadata(&src).unwrap().len() == 512 * 1024 * 1024,
            "source intact"
        );
        let final_file = in_vm(&format!("test -f '{vm}/big.bin' && echo present || echo absent"));
        eprintln!("  final name on server: {final_file}; leftovers: {:?}", partials(&vm));
        if r.failures.is_empty() {
            // Finished before the server went away: then it must be complete and correct.
            wait_until_reachable(&srv);
            assert_eq!(
                server_sha256(&format!("{vm}/big.bin")),
                local_sha256(&src),
                "reported as copied, so it must be intact"
            );
        } else {
            assert!(r.completed.is_empty());
            assert_eq!(final_file, "absent", "no incomplete file under the final name");
        }
        fs::remove_dir_all(&local).unwrap();
    }
    // Existing destination + Replace: the old version must survive the outage.
    eprintln!("outage during Replace");
    let (r, src, vm, local) = copy_with_outage(&srv, "outage-replace", 0.5, Some("old version"));
    if r.failures.is_empty() {
        wait_until_reachable(&srv);
        assert_eq!(server_sha256(&format!("{vm}/big.bin")), local_sha256(&src));
    } else {
        assert_eq!(in_vm(&format!("cat '{vm}/big.bin'")), "old version", "existing file intact");
    }
    fs::remove_dir_all(&local).unwrap();
    // After the server is back, the share works again.
    eprintln!("share reachable again after {:?}", wait_until_reachable(&srv));
}

#[test]
#[ignore]
fn samba_outage_during_listing_rename_and_status() {
    let Some(srv) = server() else { return };
    let (dir, _) = server_dir(&srv, "PublicTest", "/srv/nexdir-test/public", "outage-misc");
    fs::write(dir.join("a.txt"), "a").unwrap();
    let state = NetworkState::windows();
    smbd("stop");
    let t0 = Instant::now();
    let l = filesystem::list_dir(&dir, false);
    eprintln!(
        "listing while down: {:?} after {:?}",
        l.as_ref().err().map(|e| (&e.code, &e.message)),
        t0.elapsed()
    );
    assert!(l.is_err());
    let t0 = Instant::now();
    let rn = crate::operations::rename_entry(&dir.join("a.txt"), "b.txt");
    eprintln!(
        "rename while down: {:?} after {:?}",
        rn.as_ref().err().map(|e| &e.message),
        t0.elapsed()
    );
    assert!(rn.is_err());
    let t0 = Instant::now();
    let p = probe(&state, &format!(r"\\{srv}\PublicTest")).unwrap();
    eprintln!("status while down: {:?} after {:?}", p.status, t0.elapsed());
    assert_ne!(p.status, NetworkStatus::Connected);
    let cancel = AtomicBool::new(false);
    let emit = |_: &crate::tasks::TaskProgress| {};
    let mut ctx = TaskContext::new(1, crate::tasks::TaskKind::NetworkDiscovery, &cancel, &emit);
    let t0 = Instant::now();
    let d = discover(&state, std::slice::from_ref(&srv), &mut ctx);
    eprintln!(
        "discovery while down: {:?} after {:?}",
        d.servers.first().and_then(|s| s.status.as_ref()).map(|s| s.status),
        t0.elapsed()
    );
    assert!(d.servers[0].shares.is_none());
    smbd("start");
    std::thread::sleep(Duration::from_secs(2));
    let t0 = Instant::now();
    let mut ok = false;
    for _ in 0..30 {
        if filesystem::list_dir(&dir, false).is_ok() {
            ok = true;
            break;
        }
        std::thread::sleep(Duration::from_secs(1));
    }
    eprintln!("access again after restart: {ok} after {:?}", t0.elapsed());
    assert!(ok);
    assert!(dir.join("a.txt").exists(), "rename during the outage did not happen");
}

// ---------------------------------------------------------------------------------------------
// Phase: slow and lossy links (netem on the VM's interface only).

#[test]
#[ignore]
fn samba_slow_links_copy_cancel_and_listing() {
    let Some(srv) = server() else { return };
    let profile = std::env::var("NEXDIR_SAMBA_NETEM").unwrap_or_else(|_| "none".into());
    if profile != "none" {
        shape(&profile);
    }
    struct Unshape;
    impl Drop for Unshape {
        fn drop(&mut self) {
            unshape();
        }
    }
    let _restore = Unshape;
    let (dir, vm) = server_dir(&srv, "LargeFiles", "/srv/nexdir-test/large", "slow");
    let local = tempfile::tempdir().unwrap();
    random_file(&local.path().join("f.bin"), 20 * 1024 * 1024, 3);
    let t0 = Instant::now();
    let l = filesystem::list_dir(&share(&srv, "ReadOnlyTest").join("docs"), false).unwrap();
    eprintln!(
        "[{profile}] list small folder: {} entries in {} ms",
        l.entries.len(),
        t0.elapsed().as_millis()
    );
    let mut speeds = vec![];
    let mut last = (Instant::now(), 0u64);
    let t0 = Instant::now();
    let r = run_with(
        TransferKind::Copy,
        vec![local.path().join("f.bin")],
        &dir,
        ConflictPolicy::Skip,
        &AtomicBool::new(false),
        &mut |p| {
            let dt = last.0.elapsed().as_secs_f64();
            if dt > 0.5 {
                speeds.push((p.done_bytes - last.1) as f64 / dt / 1e6);
                last = (Instant::now(), p.done_bytes);
            }
        },
    );
    let secs = t0.elapsed().as_secs_f64();
    assert!(r.failures.is_empty(), "{r:?}");
    assert_eq!(server_sha256(&format!("{vm}/f.bin")), local_sha256(&local.path().join("f.bin")));
    eprintln!(
        "[{profile}] 20 MiB copy: {secs:.1} s ({:.1} MB/s), progress updates in MB/s: {:?}",
        20.0 * 1.048576 / secs,
        speeds.iter().map(|s| (s * 10.0).round() / 10.0).collect::<Vec<_>>()
    );
    // Reference: Windows' own CopyFile (as Explorer uses it) on the same link.
    let t0 = Instant::now();
    fs::copy(local.path().join("f.bin"), dir.join("f-windows-copyfile.bin")).unwrap();
    eprintln!("[{profile}] reference CopyFile 20 MiB: {:.1} s", t0.elapsed().as_secs_f64());
    // Cancel during a transfer.
    random_file(&local.path().join("g.bin"), 64 * 1024 * 1024, 4);
    let cancel = AtomicBool::new(false);
    let t0 = Instant::now();
    let mut asked = None;
    let r = run_with(
        TransferKind::Copy,
        vec![local.path().join("g.bin")],
        &dir,
        ConflictPolicy::Skip,
        &cancel,
        &mut |p| {
            if asked.is_none() && p.done_bytes > 4 * 1024 * 1024 {
                asked = Some(t0.elapsed());
                cancel.store(true, Ordering::SeqCst);
            }
        },
    );
    eprintln!(
        "[{profile}] cancel requested after {:?}, operation ended after {:?}; completed {}",
        asked,
        t0.elapsed(),
        r.completed.len()
    );
    assert!(r.completed.is_empty());
    assert_eq!(in_vm(&format!("test -f '{vm}/g.bin' && echo present || echo absent")), "absent");
    assert_eq!(partials(&vm), "", "cancelled copy leaves no temporary file");
}
