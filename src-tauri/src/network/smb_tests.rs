//! Real SMB tests through the Windows SMB client and server on this PC (loopback,
//! `\\localhost\C$`). They use only temporary folders with synthetic files and change no
//! settings, shares or firewall rules. Ignored by default because CI machines and standard
//! users may not reach the administrative share; run them explicitly:
//!
//!     cargo test --lib smb_ -- --ignored --test-threads=1
//!
//! They prove behaviour over a real SMB connection, not on NAS hardware.

use super::*;
use crate::filesystem;
use crate::operations::transfer::{self, ConflictPolicy, OperationReport, TransferKind, TransferRequest, NETWORK_MOVE_REFUSED};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

/// A temporary folder on C: and the same folder seen through the SMB loopback share.
struct Loopback {
    _tmp: tempfile::TempDir,
    local: PathBuf,
    unc: PathBuf,
}

fn loopback() -> Option<Loopback> {
    let tmp = tempfile::tempdir().ok()?;
    let local = tmp.path().to_path_buf();
    let s = local.display().to_string();
    let rest = s.strip_prefix(r"C:\").or_else(|| s.strip_prefix(r"c:\"))?;
    let unc = PathBuf::from(format!(r"\\localhost\C$\{rest}"));
    if fs::metadata(&unc).is_err() {
        eprintln!("SKIPPED: {} is not reachable over SMB", unc.display());
        return None;
    }
    Some(Loopback { _tmp: tmp, local, unc })
}

fn run(kind: TransferKind, sources: Vec<PathBuf>, destination: &Path, policy: ConflictPolicy) -> OperationReport {
    let req = TransferRequest {
        kind,
        sources,
        destination: destination.to_path_buf(),
        policy,
    };
    let cancel = AtomicBool::new(false);
    let mut seen = |_: &transfer::OperationProgress| {};
    transfer::run_for_test(&req, &cancel, &mut seen)
}

fn leftovers(dir: &Path) -> Vec<String> {
    fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with(crate::operations::safe_copy::TEMP_PREFIX))
        .collect()
}

#[test]
#[ignore]
fn smb_paths_are_recognised_as_network_and_validated() {
    let Some(lb) = loopback() else { return };
    assert!(is_remote(&lb.unc));
    assert!(!is_remote(&lb.local));
    assert!(crate::security::validate_path(&lb.unc.display().to_string()).is_ok());
}

#[test]
#[ignore]
fn smb_listing_unicode_names_and_properties() {
    let Some(lb) = loopback() else { return };
    fs::create_dir(lb.local.join("Ordner Ü 日本")).unwrap();
    fs::write(lb.local.join("Résumé 😀.txt"), "hello").unwrap();
    let listing = filesystem::list_dir(&lb.unc, false).unwrap();
    let names: Vec<_> = listing.entries.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"Ordner Ü 日本") && names.contains(&"Résumé 😀.txt"), "{names:?}");
    let file = listing.entries.iter().find(|e| e.name == "Résumé 😀.txt").unwrap();
    assert_eq!(file.size, Some(5));
    assert!(file.modified.is_some());
}

#[test]
#[ignore]
fn smb_create_rename_and_copy_to_and_from_the_share() {
    let Some(lb) = loopback() else { return };
    let src = lb.local.join("src");
    fs::create_dir_all(src.join("docs/deep")).unwrap();
    fs::write(src.join("docs/a.txt"), "alpha").unwrap();
    let big: Vec<u8> = (0..(24 * 1024 * 1024)).map(|i| (i % 251) as u8).collect();
    fs::write(src.join("docs/deep/big.bin"), &big).unwrap();
    let share_dst = lb.unc.join("on-share");
    let folder = crate::operations::create_folder(&lb.unc, "on-share").unwrap();
    assert_eq!(folder, share_dst);

    // Local → share.
    let r = run(TransferKind::Copy, vec![src.join("docs")], &share_dst, ConflictPolicy::Skip);
    assert!(r.failures.is_empty(), "{r:?}");
    assert_eq!(fs::read(lb.local.join("on-share/docs/deep/big.bin")).unwrap(), big);
    assert!(leftovers(&lb.local.join("on-share/docs/deep")).is_empty());

    // Conflict on the share: Replace swaps in a verified copy, KeepBoth adds a numbered name.
    fs::write(src.join("docs/a.txt"), "alpha v2").unwrap();
    let r = run(
        TransferKind::Copy,
        vec![src.join("docs/a.txt")],
        &share_dst.join("docs"),
        ConflictPolicy::Replace,
    );
    assert!(r.failures.is_empty(), "{r:?}");
    assert_eq!(fs::read_to_string(lb.local.join("on-share/docs/a.txt")).unwrap(), "alpha v2");
    let r = run(
        TransferKind::Copy,
        vec![src.join("docs/a.txt")],
        &share_dst.join("docs"),
        ConflictPolicy::KeepBoth,
    );
    assert!(r.failures.is_empty(), "{r:?}");
    assert!(lb.local.join("on-share/docs/a (2).txt").exists());
    assert!(leftovers(&lb.local.join("on-share/docs")).is_empty());

    // Share → local.
    let back = lb.local.join("back");
    fs::create_dir(&back).unwrap();
    let r = run(TransferKind::Copy, vec![share_dst.join("docs")], &back, ConflictPolicy::Skip);
    assert!(r.failures.is_empty(), "{r:?}");
    assert_eq!(fs::read(back.join("docs/deep/big.bin")).unwrap(), big);

    // Rename on the share.
    let renamed = crate::operations::rename_entry(&share_dst.join("docs/a.txt"), "renamed.txt").unwrap();
    assert!(renamed.ends_with("renamed.txt"));
    assert!(lb.local.join("on-share/docs/renamed.txt").exists());
}

#[test]
#[ignore]
fn smb_moves_across_the_share_boundary_are_refused_and_change_nothing() {
    let Some(lb) = loopback() else { return };
    let local_src = lb.local.join("local-src");
    fs::create_dir_all(&local_src).unwrap();
    fs::write(local_src.join("keep.txt"), "original").unwrap();
    fs::create_dir(lb.local.join("target")).unwrap();
    // C:\… → \\localhost\C$\…: different devices for Windows, so this would be copy + delete.
    let r = run(
        TransferKind::Move,
        vec![local_src.join("keep.txt")],
        &lb.unc.join("target"),
        ConflictPolicy::Skip,
    );
    assert_eq!(r.failures.len(), 1, "{r:?}");
    assert_eq!(r.failures[0].message, NETWORK_MOVE_REFUSED);
    assert_eq!(fs::read_to_string(local_src.join("keep.txt")).unwrap(), "original");
    assert!(!lb.local.join("target/keep.txt").exists());

    // Share → local is refused the same way.
    let r = run(
        TransferKind::Move,
        vec![lb.unc.join("local-src/keep.txt")],
        &lb.local.join("target"),
        ConflictPolicy::Skip,
    );
    assert_eq!(r.failures[0].message, NETWORK_MOVE_REFUSED);
    assert!(local_src.join("keep.txt").exists());

    // Inside the share a move is a rename and works.
    let r = run(
        TransferKind::Move,
        vec![lb.unc.join("local-src/keep.txt")],
        &lb.unc.join("target"),
        ConflictPolicy::Skip,
    );
    assert!(r.failures.is_empty(), "{r:?}");
    assert_eq!(fs::read_to_string(lb.local.join("target/keep.txt")).unwrap(), "original");
}

#[test]
#[ignore]
fn smb_items_are_never_deleted_permanently() {
    let Some(lb) = loopback() else { return };
    fs::write(lb.local.join("victim.txt"), "keep me").unwrap();
    let r = crate::operations::recycle_entries(&[lb.unc.join("victim.txt")], None).unwrap();
    assert!(r.recycled.is_empty());
    assert!(r.failed[0].message.contains("no Recycle Bin"), "{:?}", r.failed);
    assert!(lb.local.join("victim.txt").exists());
}

#[test]
#[ignore]
fn smb_status_of_reachable_missing_and_unknown_locations() {
    let state = NetworkState::windows();
    if fs::metadata(r"\\localhost\C$").is_err() {
        eprintln!("SKIPPED: loopback share not reachable");
        return;
    }
    assert_eq!(probe(&state, r"\\localhost\C$").unwrap().status, NetworkStatus::Connected);
    assert_eq!(
        probe(&state, r"\\localhost\C$\nexdir-does-not-exist-7f3a").unwrap().status,
        NetworkStatus::NotFound
    );
    assert_eq!(
        probe(&state, r"\\localhost\nexdir-no-such-share-7f3a").unwrap().status,
        NetworkStatus::NotFound
    );
    let t0 = std::time::Instant::now();
    let unknown = probe(&state, r"\\nexdir-test-host.invalid\share").unwrap();
    assert!(
        matches!(unknown.status, NetworkStatus::Unreachable | NetworkStatus::TimedOut),
        "{unknown:?}"
    );
    eprintln!("unknown host: {:?} after {:?}", unknown.status, t0.elapsed());
}

#[test]
#[ignore]
fn smb_connect_and_disconnect_without_touching_other_connections() {
    let state = NetworkState::windows();
    if fs::metadata(r"\\localhost\C$").is_err() {
        eprintln!("SKIPPED: loopback share not reachable");
        return;
    }
    let before = state.api.connected().unwrap_or_default();
    let out = connect(&state, r"\\localhost\C$\Windows", None).unwrap();
    eprintln!("connect: {:?} managed={} existing={}", out.report.status, out.managed, out.existing);
    assert_eq!(out.report.status, NetworkStatus::Connected);
    if out.existing {
        // Windows already had this connection: Nexdir must not disconnect it.
        assert!(disconnect(&state, r"\\localhost\C$").is_err());
    } else {
        assert!(out.managed);
        let after = state.api.connected().unwrap_or_default();
        assert!(after.iter().any(|r| r.remote.eq_ignore_ascii_case(r"\\localhost\C$")), "{after:?}");
        let r = disconnect(&state, r"\\localhost\C$").unwrap();
        assert_eq!(r.status, NetworkStatus::NotConnected, "{r:?}");
        let end = state.api.connected().unwrap_or_default();
        assert_eq!(end.len(), before.len(), "back to the connections that existed before");
    }
}

#[test]
#[ignore]
fn smb_overview_discovery_and_shares_of_this_pc() {
    let state = NetworkState::windows();
    let o = overview(&state, &[]);
    eprintln!("overview: {} locations, warning {:?}", o.locations.len(), o.warning);
    match list_shares(&state, "localhost") {
        Ok(shares) => {
            eprintln!("visible shares on localhost: {}", shares.len());
            assert!(shares.iter().all(|s| !s.name.ends_with('$')), "hidden shares are left out");
        }
        Err(r) => eprintln!("share list not available: {:?} {:?}", r.status, r.code),
    }
    let cancel = AtomicBool::new(false);
    let emit = |_: &crate::tasks::TaskProgress| {};
    let mut ctx = TaskContext::new(1, crate::tasks::TaskKind::NetworkDiscovery, &cancel, &emit);
    let t0 = std::time::Instant::now();
    let d = discover(&state, &["localhost".into()], &mut ctx);
    eprintln!(
        "discovery: network list available={}, servers={}, took {:?}",
        d.network_list_available,
        d.servers.len(),
        t0.elapsed()
    );
    assert!(d.servers.iter().any(|s| s.name.eq_ignore_ascii_case("localhost")));
}
