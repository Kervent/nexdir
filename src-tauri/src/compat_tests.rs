//! Windows compatibility checks through the app's own code paths (validation → listing →
//! transfer engine → rename), on synthetic data in temporary directories.

use crate::filesystem;
use crate::operations;
use crate::operations::transfer::{ConflictPolicy, Engine, OperationProgress, TransferKind, TransferRequest};
use crate::security::validate_path;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

fn run(kind: TransferKind, sources: Vec<PathBuf>, destination: &Path) -> (OperationProgress, usize) {
    let req = TransferRequest {
        kind,
        sources,
        destination: destination.to_path_buf(),
        policy: ConflictPolicy::Skip,
    };
    let cancel = AtomicBool::new(false);
    let mut on_update = |_: &OperationProgress| {};
    let (progress, report) = Engine::new(1, &req, &cancel, &mut on_update).run();
    (progress, report.failures.len())
}

/// Builds a folder whose files live deeper than MAX_PATH (260 characters).
fn deep_tree(root: &Path) -> PathBuf {
    let mut deep = root.join("deep");
    while deep.as_os_str().len() < 320 {
        deep.push("a_fairly_long_directory_name_segment");
    }
    fs::create_dir_all(&deep).unwrap();
    fs::write(deep.join("report.txt"), "deep").unwrap();
    deep
}

#[test]
fn long_paths_through_validation_listing_copy_and_move() {
    let dir = tempfile::tempdir().unwrap();
    let deep = deep_tree(dir.path());
    let checked = validate_path(&deep.to_string_lossy()).unwrap();
    assert_eq!(filesystem::list_dir(&checked, false).unwrap().entries.len(), 1);

    let copy_dest = dir.path().join("copy");
    fs::create_dir(&copy_dest).unwrap();
    let (p, failed) = run(TransferKind::Copy, vec![dir.path().join("deep")], &copy_dest);
    assert_eq!((p.done_files, failed), (1, 0));
    let copied = copy_dest.join(deep.strip_prefix(dir.path()).unwrap()).join("report.txt");
    assert!(copied.as_os_str().len() > 260);
    assert_eq!(fs::read_to_string(&copied).unwrap(), "deep");

    let move_dest = dir.path().join("moved");
    fs::create_dir(&move_dest).unwrap();
    let (p, failed) = run(TransferKind::Move, vec![dir.path().join("deep")], &move_dest);
    assert_eq!((p.done_files, failed), (1, 0));
    assert!(!dir.path().join("deep").exists());
    assert!(move_dest.join(deep.strip_prefix(dir.path()).unwrap()).join("report.txt").exists());
}

#[test]
fn unicode_and_unusual_names() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src");
    fs::create_dir(&src).unwrap();
    let names = [
        "Ünïcödé résumé.txt",
        "日本語のファイル.txt",
        "emoji 😀 file.txt",
        "e\u{301} combining.txt",
        ".hidden-style name",
        "name with  two spaces.txt",
        "no extension",
        "many.dots.in.name.tar.gz",
        "ﬁle ligature.txt",
    ];
    for name in names {
        fs::write(src.join(name), name).unwrap();
    }
    let listing = filesystem::list_dir(&validate_path(&src.to_string_lossy()).unwrap(), true).unwrap();
    assert_eq!(listing.entries.len(), names.len());

    let dest = dir.path().join("dest");
    fs::create_dir(&dest).unwrap();
    let (p, failed) = run(TransferKind::Copy, vec![src.clone()], &dest);
    assert_eq!((p.done_files as usize, failed), (names.len(), 0));
    for name in names {
        assert_eq!(fs::read_to_string(dest.join("src").join(name)).unwrap(), name);
    }

    let renamed = operations::rename_entry(&dest.join("src").join("emoji 😀 file.txt"), "umbenannt – 改名.txt").unwrap();
    assert_eq!(fs::read_to_string(renamed).unwrap(), "emoji 😀 file.txt");
}

#[test]
fn case_only_rename_keeps_the_file() {
    // NTFS is case-insensitive: renaming "photo.JPG" to "photo.jpg" must not be treated as a
    // conflict with itself, and must not lose the file.
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("photo.JPG");
    fs::write(&file, "img").unwrap();
    operations::rename_entry(&file, "photo.jpg").unwrap();
    let entries: Vec<String> = fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(entries, ["photo.jpg"]);
    assert_eq!(fs::read_to_string(dir.path().join("photo.jpg")).unwrap(), "img");
}
