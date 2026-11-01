//! Performance measurements on synthetic data in temporary directories. Ignored by default;
//! run with (separate target dir so the release app is not overwritten):
//!   cargo test --release --manifest-path src-tauri/Cargo.toml --target-dir target-perf --lib perf -- --ignored --nocapture --test-threads=1

use crate::analytics::duplicates::{find_duplicates, DuplicateOptions};
use crate::analytics::storage;
use crate::filesystem;
use crate::operations::transfer::{ConflictPolicy, Engine, OperationProgress, TransferKind, TransferRequest};
use crate::search::index::{self, IndexSettings};
use crate::search::{self, SearchKind, SearchQuery};
use crate::tasks::testing::run_now;
use crate::tasks::TaskKind;
use std::fs;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

const EXTENSIONS: [&str; 8] = ["txt", "jpg", "pdf", "log", "mp4", "docx", "rs", "zip"];

/// `folders` × `per_folder` small files with varied names, extensions and sizes.
fn synthetic_tree(root: &Path, folders: usize, per_folder: usize) -> usize {
    let mut n = 0;
    for f in 0..folders {
        let dir = root.join(format!("project {f:03}")).join("assets");
        fs::create_dir_all(&dir).unwrap();
        for i in 0..per_folder {
            let ext = EXTENSIONS[(f + i) % EXTENSIONS.len()];
            let size = (i % 17) * 64;
            fs::write(dir.join(format!("report_{f}_{i}.{ext}")), vec![b'a' + (i % 26) as u8; size]).unwrap();
            n += 1;
        }
    }
    n
}

fn timed<R>(label: &str, f: impl FnOnce() -> R) -> R {
    let start = Instant::now();
    let r = f();
    println!("PERF {label}: {} ms", start.elapsed().as_millis());
    r
}

#[test]
#[ignore]
fn list_dir_20k_entries() {
    let dir = tempfile::tempdir().unwrap();
    for i in 0..20_000 {
        fs::write(dir.path().join(format!("file{i:05}.txt")), b"x").unwrap();
    }
    let mut best = u128::MAX;
    for _ in 0..3 {
        let start = Instant::now();
        let listing = filesystem::list_dir(dir.path(), false).unwrap();
        assert_eq!(listing.entries.len(), 20_000);
        best = best.min(start.elapsed().as_millis());
    }
    println!("PERF list_dir (app code path) 20,000 entries: best of 3 = {best} ms");
}

#[test]
#[ignore]
fn tools_on_50k_files() {
    let dir = tempfile::tempdir().unwrap();
    let count = timed("create 50,000 files in 500 folders", || synthetic_tree(dir.path(), 500, 100));
    assert_eq!(count, 50_000);

    let (report, _) = timed("storage analysis 50,000 files", || {
        run_now(TaskKind::StorageScan, |ctx| storage::analyze(dir.path(), ctx))
    });
    assert_eq!(report.unwrap().file_count, 50_000);

    let query = SearchQuery {
        root: dir.path().to_string_lossy().into_owned(),
        recursive: true,
        include_hidden: false,
        text: Some("*.log".into()),
        extensions: vec![],
        categories: vec![],
        min_size: None,
        max_size: None,
        modified_after: None,
        modified_before: None,
        kind: SearchKind::Files,
        limit: Some(50_000),
    };
    let (found, _) = timed("live search *.log in 50,000 files", || {
        run_now(TaskKind::Search, |ctx| search::search(dir.path(), &query, ctx))
    });
    let expected = (0..500)
        .map(|f| (0..100).filter(|i| EXTENSIONS[(f + i) % EXTENSIONS.len()] == "log").count())
        .sum::<usize>();
    assert_eq!(found.unwrap().entries.len(), expected);

    let db = dir.path().join("index.sqlite");
    let settings = IndexSettings {
        folders: vec![dir.path().to_string_lossy().into_owned()],
        include_content: false,
        update_on_startup: false,
    };
    let (status, _) = timed("index build 50,000 files", || {
        run_now(TaskKind::IndexBuild, |ctx| index::build(&db, &settings, ctx))
    });
    assert!(status.unwrap().files >= 50_000);
    let words = SearchQuery {
        text: Some("report 42".into()),
        ..query.clone()
    };
    let mut best = u128::MAX;
    for _ in 0..5 {
        let start = Instant::now();
        let r = index::query(&db, &words).unwrap();
        assert!(!r.entries.is_empty());
        best = best.min(start.elapsed().as_micros());
    }
    println!(
        "PERF index query 'report 42' over 50,000 files: best of 5 = {:.1} ms",
        best as f64 / 1000.0
    );

    let (dups, _) = timed("duplicate scan 50,000 files (min size 1 byte)", || {
        run_now(TaskKind::Duplicates, |ctx| {
            find_duplicates(
                &DuplicateOptions {
                    roots: vec![dir.path().to_path_buf()],
                    min_size: 1,
                    include_hidden: false,
                },
                ctx,
            )
        })
    });
    assert!(!dups.unwrap().groups.is_empty());
}

/// The strict path: every file replaces an existing one (temp copy, flush, BLAKE3 check,
/// ReplaceFileW with backup, backup removal).
#[test]
#[ignore]
fn replace_1000_small_files_and_one_large_file() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src");
    synthetic_tree(&src, 10, 100);
    let dst = dir.path().join("dst");
    fs::create_dir(&dst).unwrap();
    let copy = TransferRequest {
        kind: TransferKind::Copy,
        sources: vec![src.clone()],
        destination: dst.clone(),
        policy: ConflictPolicy::Skip,
    };
    let replace = TransferRequest {
        policy: ConflictPolicy::Replace,
        ..copy.clone()
    };
    let cancel = AtomicBool::new(false);
    let mut on_update = |_: &OperationProgress| {};
    Engine::new(1, &copy, &cancel, &mut on_update).run();
    let (p, r) = timed("replace 1,000 existing small files (Replace policy)", || {
        Engine::new(2, &replace, &cancel, &mut on_update).run()
    });
    assert_eq!((p.done_files, r.failures.len()), (1_000, 0));

    let big = dir.path().join("big.bin");
    fs::write(&big, vec![0x5Au8; 256 * 1024 * 1024]).unwrap();
    let big_dst = dir.path().join("bigdst");
    fs::create_dir(&big_dst).unwrap();
    fs::write(big_dst.join("big.bin"), b"old").unwrap();
    let req = TransferRequest {
        kind: TransferKind::Copy,
        sources: vec![big.clone()],
        destination: big_dst.clone(),
        policy: ConflictPolicy::Replace,
    };
    timed("replace one 256 MiB file (Replace policy)", || {
        Engine::new(3, &req, &cancel, &mut on_update).run()
    });
    let plain = dir.path().join("plain.bin");
    timed("baseline std::fs::copy of the 256 MiB file", || fs::copy(&big, &plain).unwrap());
}

#[test]
#[ignore]
fn copy_5000_small_files() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src");
    synthetic_tree(&src, 50, 100);
    let dst = dir.path().join("dst");
    fs::create_dir(&dst).unwrap();
    let req = TransferRequest {
        kind: TransferKind::Copy,
        sources: vec![src.clone()],
        destination: dst.clone(),
        policy: ConflictPolicy::Skip,
    };
    let cancel = AtomicBool::new(false);
    let mut updates = 0u32;
    let mut on_update = |_: &OperationProgress| updates += 1;
    let (progress, report) = timed("copy folder with 5,000 small files (transfer engine)", || {
        Engine::new(1, &req, &cancel, &mut on_update).run()
    });
    assert_eq!(progress.done_files, 5_000);
    assert!(report.failures.is_empty());
    println!("PERF   progress events: {updates}");

    // Baseline: the same files with plain std::fs::copy (CopyFileExW) and no checks.
    let base = dir.path().join("baseline");
    timed("baseline std::fs::copy of the same 5,000 files", || {
        for entry in fs::read_dir(&src).unwrap() {
            let folder = entry.unwrap().path().join("assets");
            let target = base.join(folder.strip_prefix(&src).unwrap());
            fs::create_dir_all(&target).unwrap();
            for file in fs::read_dir(&folder).unwrap() {
                let file = file.unwrap().path();
                fs::copy(&file, target.join(file.file_name().unwrap())).unwrap();
            }
        }
    });
}
