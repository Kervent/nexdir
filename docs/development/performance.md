# Performance measurements

Synthetic data in temporary directories only — never personal files. Reproduce with:

```bash
# Backend (release build, separate target dir so target/release/nexdir.exe is not overwritten)
cargo test --release --manifest-path src-tauri/Cargo.toml --target-dir target-perf --lib perf -- --ignored --nocapture --test-threads=1
cargo test --release --manifest-path src-tauri/Cargo.toml --target-dir target-perf --test performance -- --ignored --nocapture

# Frontend sorting/filtering (runs with the normal test suite)
npx vitest run src/lib/sort.perf.test.ts --silent=false
```

Machine: Windows 11 Pro (build 26300), NTFS on SSD, warm file-system cache, Microsoft
Defender active.

## Backend (2026-10-31, v0.4.0)

| Test | Result |
|---|---|
| `list_dir` (app code path) on 20,000 files, best of 3 | 13 ms |
| Storage analysis, 50,000 files in 500 folders | 78–96 ms |
| Live search `*.log` in 50,000 files (6,252 hits) | 97–104 ms |
| Search index build, 50,000 files | 631 ms |
| Index query "report 42" over 50,000 files, best of 5 | 6.1 ms |
| Duplicate scan, 50,000 small files (min size 1 byte, almost all candidates) | 2.9 s |
| Copy folder with 5,000 small files (transfer engine) | 2.1–2.3 s |
| Same 5,000 files with plain `std::fs::copy`, no checks (baseline) | 2.1 s |
| Copy one 512 MiB file (`CopyFileExW`), 2026-10-30 | 131 ms (~3.9 GB/s) |

The transfer engine adds about 8 % over a bare copy loop for many small files (conflict checks,
link detection, progress); the rest is Windows file-creation cost. Progress events are throttled
(about 20 for this copy), so the UI is not flooded.

**0.4.3 (2026-11-01):** the duplicate scan reads each candidate's file identity (volume and
file index) from the handle it already opened for hashing, so hard links are recognised and read
only once. Cost: 50,000 small files (almost all candidates) 3.4–3.8 s in three runs, against
2.6–2.9 s before; for large files hashing dominates and the difference is negligible. A first
version that opened every file a second time took 5.2 s. Other benchmarks unchanged within
run-to-run variation (list 20,000 entries 12 ms, storage analysis 107–111 ms, live search
121–125 ms, index build 666–791 ms, index query 6.6–7.5 ms).

**After the safe-copy change (2026-10-31, [safe-copy.md](../security/safe-copy.md)):**

| Test | Result |
|---|---|
| Copy folder with 5,000 small files to new names (temp file + rename, size check) | 3.8 s (baseline 2.0 s in the same run) |
| Replace 1,000 existing small files (flush, BLAKE3, `ReplaceFileW`, backup removal) | 5.1 s |
| Replace one 256 MiB file | 0.30 s (plain `std::fs::copy`: 0.06 s, warm cache) |

A first version that flushed and hashed every copy took 11.7–19.9 s for the 5,000 small files;
flushing and hashing are now limited to cases where data disappears afterwards (replace, move).

**After the handle-bound copy (TOCTOU fix, 2026-10-31):** own read/write loop with on-the-fly
BLAKE3 and read-back through the same handle (no reopening).

| Test | Result |
|---|---|
| Copy 5,000 small files to new names | 4.8 s (baseline 2.3 s in the same run) |
| Replace 1,000 existing small files | 2.8 s |
| Replace one 256 MiB file | 0.28 s (plain copy 0.07 s) |
| Other benchmarks (listing, storage, search, index, duplicates) | unchanged within noise |

## Frontend (Node 22 / V8 with ICU, 100,000 entries)

| Operation | Before Phase 6 | v0.4.0 |
|---|---|---|
| Sort by name (natural order, `Intl.Collator`) | 332 ms | 336 ms |
| Sort by type | 388 ms | 289 ms |
| Sort by modified | 59 ms | 30 ms |
| Sort by size | 21 ms | 17 ms |
| Filter as you type | 4.3 ms | 3.9 ms |

Changes in Phase 6:

- Sort keys (folder flag, type label) are computed once per entry instead of once per comparison.
- **Typing in the filter box no longer re-sorts the folder.** The list is sorted once per listing
  and sort order; filtering works on the sorted list. For a 100,000-entry folder a keystroke now
  costs ~4 ms instead of ~330 ms.

Name sorting is bound by the collator (natural, case- and accent-insensitive comparison, like
Explorer). It happens once when a folder is opened or the sort order changes.

## Notes

- All numbers are with a warm cache. Cold-cache, HDD and network numbers will be much lower;
  these show that Nexdir is not the bottleneck, not real-world disk speed.
- The UI renders only visible rows (`@tanstack/react-virtual`), so the DOM size does not grow
  with folder size.
- Not measured: UI frame times in the real window with very large folders (no input automation
  on this machine), cold-cache behaviour, network drives.
