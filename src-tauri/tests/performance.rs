//! Performance measurements on synthetic data in temporary directories.
//! Ignored by default because they take a few seconds; run with:
//!   cargo test --release --manifest-path src-tauri/Cargo.toml --target-dir target-perf --test performance -- --ignored --nocapture
//! More benchmarks that use the app's internal modules are in src/perf.rs.

use std::fs;
use std::time::Instant;

/// Lists a 20,000-entry directory through the same code path the app uses
/// (std::fs::read_dir + per-entry metadata from the enumeration).
#[test]
#[ignore]
fn list_20k_entries() {
    let dir = tempfile::tempdir().unwrap();
    for i in 0..20_000 {
        fs::write(dir.path().join(format!("file{i:05}.txt")), b"x").unwrap();
    }
    let mut best = u128::MAX;
    for _ in 0..3 {
        let start = Instant::now();
        let mut n = 0;
        for entry in fs::read_dir(dir.path()).unwrap() {
            let _ = entry.unwrap().metadata().unwrap();
            n += 1;
        }
        assert_eq!(n, 20_000);
        best = best.min(start.elapsed().as_millis());
    }
    println!("PERF list_dir 20,000 entries: best of 3 = {best} ms");
}

/// Copies a 512 MiB file. std::fs::copy uses CopyFileExW on Windows, the same API the
/// transfer engine calls, so this is the throughput ceiling for a single file.
#[test]
#[ignore]
fn copy_512_mib_file() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("big.bin");
    fs::write(&src, vec![0x5Au8; 512 * 1024 * 1024]).unwrap();
    let dst = dir.path().join("copy.bin");
    let start = Instant::now();
    fs::copy(&src, &dst).unwrap();
    let ms = start.elapsed().as_millis().max(1);
    println!("PERF copy 512 MiB: {ms} ms ({:.0} MiB/s)", 512_000.0 / ms as f64);
    assert_eq!(fs::metadata(&dst).unwrap().len(), 512 * 1024 * 1024);
}
