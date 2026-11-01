# Safe file copies and replacements

Implementation: [`src-tauri/src/operations/safe_copy.rs`](../../src-tauri/src/operations/safe_copy.rs),
handle primitives in [`src-tauri/src/platform/win32.rs`](../../src-tauri/src/platform/win32.rs).

History:
- **v0.4.1** — staged copy (temp file → verify → swap with backup) replaced writing directly into
  existing files, which lost the existing file on cancel or error.
- **v0.4.2** — the staged copy itself is now bound to file handles; the path-based race
  described below is fixed. (0.4.1 was superseded before publication.)

## 1. The race condition (fixed in 0.4.2)

The 0.4.1 staged copy chose a temporary name by **checking** whether the path existed
(`symlink_metadata`) and only later let `CopyFileExW` **create** it. On every failure it called
`fs::remove_file(temp_path)`. If another process created a file under that name in between,
`CopyFileExW` failed with "file exists" and Nexdir then **deleted the other process's file**.

The same pattern — decide by path, act by path later — also existed in four other places:

| Where | What could go wrong |
|---|---|
| Temp file between verification and `ReplaceFileW` | another program could rename or swap it; Nexdir would adopt a file it never verified |
| Removing the backup after `ReplaceFileW` | `fs::remove_file(backup_path)` would hit whatever had that name by then |
| Deleting the source after a move | `fs::remove_file(src)`: a newer file put at the source path after the copy would be deleted |
| Same-file and link checks | done on paths that were opened again later |
| `store.rs` (settings) | wrote to the fixed name `rules.tmp` with `fs::write`, truncating any existing file of that name |

## 2. New strategy: ownership is an open handle

Nexdir now never deletes, renames or adopts anything **by path** during a copy. Every such step
goes through a handle that Nexdir opened itself, which keeps referring to the same file even if
someone renames, deletes or replaces paths in the meantime.

### 2.1 Atomic, exclusive creation

`OwnedTemp::create` opens `~nexdir-<random>.partial` with `OpenOptions::create_new(true)`
(`CreateFileW` + `CREATE_NEW`), access read/write/delete and **share mode 0**.

- `CREATE_NEW` never opens an existing file. If the name is taken — by anyone, at any moment —
  creation fails with "file exists" and Nexdir tries a new random name (up to 32 times, then a
  controlled failure). The taken name is never opened, changed or deleted.
- Share mode 0: while Nexdir holds the handle, no other program can open, write, rename,
  replace or delete the file (verified in test F, also from another process).

### 2.2 Copy without `CopyFileExW`

`CopyFileExW` always opens the destination itself, so it cannot write into a file Nexdir holds
exclusively; with "fail if exists" it would fail on the reserved file, and without it the
reservation would be meaningless. Nexdir therefore copies the data itself:

1. The source is opened **once** (`FILE_FLAG_OPEN_REPARSE_POINT`, so a link swapped in is not
   followed; links are only followed for an explicit top-level "copy" of a file link).
2. 1 MiB chunks are read, written into the temp handle and hashed with BLAKE3 on the fly;
   progress and cancellation per chunk.
3. Size and modification time of the source are read from its handle before and after: a
   source that changed during the copy fails the operation.
4. Alternate data streams (e.g. `Zone.Identifier`, the "downloaded from the internet" mark that
   SmartScreen uses) are copied into the temp file (tested: this works while the main stream is
   held exclusively).
5. Before *Replace* and *Move* the temp file is flushed to disk; then it is **read back through
   the same handle** and compared (length and BLAKE3). A corrupted temp file is discarded.

### 2.3 Putting it in place (handle-bound renames)

Renames use `SetFileInformationByHandle(FileRenameInfo)` with `ReplaceIfExists = FALSE`: the
file behind the handle gets the new name, atomically, and only if the name is free.

- **New destination:** temp handle → destination. If someone created the destination
  meanwhile, the rename fails and that file stays untouched.
- **Existing destination (*Replace*):**
  1. The destination is opened with `FILE_FLAG_OPEN_REPARSE_POINT`, DELETE access and share
     mode *read only* — no new writers, renames or deletes during the swap. Links, folders,
     read-only files and "the same file as the source" (volume serial + file index, catches 8.3
     names and hard links) are refused at this point, on the handle.
  2. Destination handle → `~nexdir-<random>.backup`.
  3. Temp handle → destination.
  4. On failure: backup handle → destination again. If even that fails (the name was taken by
     another program in the meantime), the backup is **kept** and reported (`BackupKept`).
  5. On success: the backup is deleted **through its handle** — only the previous version itself
     can be removed.

This replaces `ReplaceFileW`, which is path-based for all three files and would require
releasing the exclusive handle first. The error states that `ReplaceFileW` documents
(1175/1176/1177) map to steps 2–4 above and are handled explicitly.

### 2.4 Cleanup

- Temporary files are removed only through their handle (`FileDispositionInfo`), so cleanup
  can only ever affect the file Nexdir created. `OwnedTemp` does this automatically on drop
  (RAII), and explicitly — with reporting — on every error path.
- If removal through the handle fails, the file is left in place and its path is reported.
  It is never retried by path.
- An adopted file is no longer owned: the destructor cannot delete it.

### 2.5 Moves

The source is deleted through the handle it was copied from, and only if its size and
modification time are still those that were copied. If another program renamed the source and
put a new file at its path, the new file is left alone (test
`create_new_and_move_delete_the_source_only_through_its_handle`).

### 2.6 Neighbouring code

- `store.rs` (settings, organize rules): same model — exclusive temp file under a fresh name,
  rename through the handle onto Nexdir's own settings file, delete through the handle on
  failure. Regression test: a foreign `rules.tmp` stays untouched.
- `search/index.rs`: deletes and renames fixed file names (`index.sqlite`, `-wal`, `-shm`,
  `.building`) by path, because SQLite opens these files by path itself. They live only in
  Nexdir's private app-data folder and never come from user input. Not changed; listed as a
  remaining limitation.
- Recycle Bin, rename, new folder, same-volume move: atomic Windows calls that never overwrite
  (`MoveFileExW` without replace, `IFileOperation`); no temporary files involved.

## 3. Tests

All tests run on Windows 11 (NTFS), in new temporary folders with synthetic files.

Commands:

```bash
cargo test --release --manifest-path src-tauri/Cargo.toml --target-dir target-perf
cargo test --release --manifest-path src-tauri/Cargo.toml --target-dir target-perf --lib safe_copy -- --nocapture --test-threads=1
```

(`--release --target-dir target-perf` because Windows on the development machine blocks the
freshly built debug test executable with "os error 32"; see `docs/development/setup.md`.)

| Test | Method | Result |
|---|---|---|
| **A** taken temp name | foreign file placed under the first name Nexdir will try | next name used; foreign file unchanged and present |
| **A** all names taken | every attempt returns the taken name | controlled failure "could not reserve"; nothing changed |
| **B** concurrent creation | a separate **process** (`cmd`) creates the exact name right before Nexdir's create call | Nexdir skips it; that file is unchanged |
| **C** write error | injected "disk full" at chunk 2 of a 3 MiB copy | destination, source and a foreign `~nexdir-…` file unchanged; no own temp file left |
| **C** source changes during copy | another writer appends to the source mid-copy | detected, nothing changed |
| **C** corrupted temp file | one byte changed through the handle after the copy | detected by BLAKE3, discarded |
| **D** cancel | before start, and after 2 MiB of a 64 MiB copy | destination and foreign files unchanged; no own temp file left |
| **E** parallel | 16 threads behind a barrier: 8 create files from same-named sources in one folder, 8 replace the same file | all 8 creates correct; replaces: in every run 1 of 8 succeeds, the other 7 are refused as "in use" (the destination is locked during a swap); the file always holds one complete version; no `.partial`/`.backup` left. 20 repeated runs of E + 10 runs of the whole module: no failure |
| **F** swap the temp file | after the copy, rename/delete/overwrite tried from the test process and from a separate `cmd` process | all 5 attempts fail; only the verified copy is adopted |
| **G** successful replace | hooks record the destination content after the copy and right before the swap | old content until the swap, new content after; no leftovers |
| Name taken during the swap | another program creates the destination between backup and adopt | its file is kept, the previous version survives as reported backup |
| Failed swap / failed restore | injected | previous version restored / backup kept and reported |
| Move deletes only through the handle | source path swapped by another file after the copy | new file kept; copied original deleted |
| Mark of the web | `Zone.Identifier` stream on the source | present on the copy |
| Locked destination / source, read-only destination, no write permission (`icacls /deny`), hard link to itself, junction as destination or source | real | refused; nothing changed |
| Destination symlink | real | **skipped**: creating symlinks needs Developer Mode or admin rights here (junction test covers the protection) |
| Handle primitives (`win32.rs`) | rename never replaces; exclusive handle blocks other processes; delete-by-handle never hits a file that took the name; streams on an exclusive file; hard-link/junction detection | pass |
| `store.rs` | foreign `rules.tmp` present | untouched; no temp files left |

Full suite: **100 passed, 0 failed, 5 ignored** (benchmarks and the real Recycle Bin test, as
before); `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` clean.

## 4. Cost

| Case (same run, warm cache) | Result |
|---|---|
| Copy 5,000 small files to new names | 4.8 s (plain `std::fs::copy`: 2.3 s) |
| Replace 1,000 existing small files | 2.8 s (was 5.1 s with `ReplaceFileW` and reopening for the hash) |
| Replace one 256 MiB file | 0.28 s (plain copy: 0.07 s) |

## 5. Remaining limitations — not proven

- **Not crash-proof.** If Nexdir is killed or the PC loses power, `~nexdir-*.partial` or
  `~nexdir-*.backup` files can remain (a handle's delete-on-close only runs if the process closes
  it). The design never leaves the destination half-written, but this was not tested with real
  power loss.
- During *Replace* there is a short moment between steps 2 and 3 in which the destination name
  does not exist (as with `ReplaceFileW`). A program creating that name exactly then makes the
  swap fail; the previous version is then kept as a reported backup, never lost.
- The destination **folder** path is resolved by name when the temp file is created and when
  renaming. A folder replaced by a junction at that moment would redirect the copy (the
  engine checks folders before; the window is not closed completely).
- Content written to the source by a program that already had it open could, in theory, keep
  the size and modification time unchanged and go undetected.
- Same-user malware is out of scope: it can change any file the user can.
- Only NTFS was tested. FAT/exFAT have no alternate data streams (silently not kept, as with
  Explorer); network shares were not tested.
- Copying streams and timestamps/attributes is best effort; failures there are not reported.
- `search/index.rs` still cleans up its own fixed file names by path (see 2.6).
