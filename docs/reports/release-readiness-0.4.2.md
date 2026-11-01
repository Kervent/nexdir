# Release readiness — Nexdir 0.4.2

Date: 2026-10-31 · Machine: Windows 11 Pro (build 26300), NTFS SSD, Defender active ·
Toolchain: rustc 1.99.1, cargo 1.99.1, Node 22.23.2, npm 10.9.8

**Result: ALPHA READY** — suitable for a first public alpha for early testers (with backups).
Not yet BETA READY; reasons in §10.

Nothing was published, pushed, tagged or signed.

---

## 1. Version

**0.4.2** in `package.json`, `package-lock.json`, `src-tauri/Cargo.toml`,
`src-tauri/tauri.conf.json`, the exe/installer version resources and the About dialog
(checked in the running app: "Version 0.4.2").

## 2. Clean build

| Step | Result |
|---|---|
| `npm ci` from `package-lock.json` | 115 packages, ok |
| Remove `dist/` and the complete Rust release output (`cargo clean --release`, 3,866 files / 2.0 GiB) | ok; old `nexdir.exe` verified absent |
| `npm run app:build` (Vite + Rust release with LTO + NSIS + WiX) | ok, Rust release compile 2 min 09 s |
| `tools/release/package.ps1` | 4 release files, checksums written |

Artifact checks:
- All checksums in `SHA256SUMS.txt` verified independently.
- Version resources: `nexdir.exe` and `Setup.exe` 0.4.2; MSI extracted administratively (no
  installation) → contains `nexdir.exe` 0.4.2.
- The exe is from this build (timestamp), imports `SetFileInformationByHandle` and
  `FindFirstStreamW`, and no longer `CopyFileExW`/`ReplaceFileW` → the safe-copy and race fixes
  are compiled in.
- Embedded manifest (`asInvoker`, Windows 10/11, `longPathAware`) as in 0.4.0.
- Portable ZIP: `Nexdir.exe`, `LICENSE.txt`, `README.txt` — no debug files, no PDBs.
- `release-out/` contains only the four 0.4.2 files; no stale bundles in `target/release/bundle`.
- Not code-signed (Authenticode: NotSigned) — expected, documented.

## 3. Test results (actual counts)

| Suite | Result |
|---|---|
| TypeScript `tsc --noEmit` | clean |
| Frontend (Vitest) | **68 / 68** passed |
| `cargo fmt --check`, `cargo clippy --all-targets -D warnings` (forced re-lint) | clean |
| Rust unit tests | **100 / 100** passed (5 ignored by default, run separately below) |
| Rust, ignored: real Recycle Bin test | **1 / 1** passed (one synthetic file `nexdir-recycle-test.txt` is now in the Recycle Bin) |
| Rust, ignored: benchmarks | **4 / 4** passed (numbers in §7) |
| Repetition: safe_copy + transfer + Win32 primitives + compatibility tests | **20 runs × 55 tests**, 0 failures |
| `tools/discord-notify` (incl. workflow safety checks) | **70 / 70** passed |
| `tools/discord-setup` | **41 / 41** passed |
| GUI test of the built portable app (§5) | **27 / 27**, **26 / 27**, **27 / 27** in three full runs |
| Dependency audits | `npm audit` 0 vulnerabilities; `cargo audit` 0 vulnerabilities (2 warnings for Linux-only crates — verified not in the Windows dependency tree); `pip-audit` 0 known vulnerabilities for both Python tools |
| Secret scan of all 257 publishable files | 0 findings (scanner verified with planted fake secrets) |

The two `tests/performance.rs` benchmarks (superseded by `src/perf.rs`) were not run.

## 4. Security functions checked

| Area | How it was checked |
|---|---|
| Safe copy (temp file → BLAKE3 → handle rename) | unit tests A–G, 20× repetition, GUI copy with Replace, mark-of-the-web copy |
| Safe move (source deleted through its own handle, only if unchanged) | unit tests incl. swapped source path; simulated cross-drive moves |
| Replace with backup and rollback (replaces the earlier `ReplaceFileW` strategy) | swap failure, name taken during swap, failed restore → backup kept |
| RAII ownership / temp files | exclusive `CREATE_NEW` + share mode 0; taken names skipped; other processes cannot rename/replace/delete the temp file (tested from a separate process); cleanup only through the handle |
| Race conditions | name taken by another process right before creation; 16 parallel threads into one folder |
| Integrity (BLAKE3) | corrupted temp file and source changed during copy are detected |
| Cleanup after errors / cancellation | injected disk-full, cancel before start and mid-copy (64 MiB unit test; **3 GiB in the GUI: target folder empty after cancel**) |
| Name conflicts, locked files, permission errors | conflict dialog in the GUI; locked source/target, read-only target, `icacls` deny — refused, nothing changed |
| Symlinks and junctions | junction as source/target/inside copied folders refused or skipped; symlink tests **skipped** (need Developer Mode/admin) |
| Path validation | device-path spellings, alternate data streams, wildcards, control characters |
| Settings files (`store.rs`) | foreign `rules.tmp` untouched |

Neighbouring operations reviewed for data loss:
- **Rename / undo rename / new folder**: atomic "fail if exists" calls; undo uses the same
  no-replace rename — no overwrite possible.
- **Recycle Bin**: only fixed drives; Windows asks before anything would be deleted permanently.
  The check and the shell operation are path-based; in the worst case a file swapped in between
  goes to the Recycle Bin (recoverable). Real test passed.
- **Duplicate cleanup**: "never all copies of a group" is enforced in the UI only, and the
  scan result can be outdated if files change afterwards. Removal goes to the Recycle Bin
  (recoverable). Listed as a risk.
- **Organize rules**: run through the same transfer engine (safe copy/move).
- **Search index**: deletes its own fixed files by path in Nexdir's private folder (SQLite needs
  paths). Low risk, documented.

## 5. GUI test of the compiled application

The portable `Nexdir.exe` from the release ZIP was started with synthetic data and controlled
through the WebView2 DevTools protocol (remote-debugging port set only via an environment
variable for the test process): real mouse/keyboard events, real Rust backend, console and
page-load monitoring, CPU/memory sampling. The test script and its screenshots are kept outside the
repository.

| Check | Result (last run) |
|---|---|
| Start with a folder argument | window ready **~450–580 ms** after process start |
| Light / dark / system theme | switches; light bg `rgb(246,247,251)`, dark `rgb(15,18,32)`; screenshots readable |
| Navigation (double-click, back, up), details/icons view, tabs (new, Ctrl+W), split view | pass |
| 20,000-file folder | listed in **85 ms**; **40** rows in the DOM (virtualised); scroll to end; sort descending **19 ms**; filter **10 ms** after typing |
| 10× leave/reopen the 20,000-file folder | **79–94 ms** each |
| Preview | text and PNG shown |
| Error handling | reserved name "CON" → inline error, Rename disabled, Cancel closes; missing folder in address bar → clear message |
| New folder; copy + paste with conflict → Replace | file replaced, no temporary files left |
| 3 GiB copy | UI usable during the copy (new tab in **7–9 ms**, F1 dialog opens); **cancel → target folder empty** |
| Search ("notes in <folder>"), duplicate finder, storage analysis (3.01 GB, 20,010 files) | pass |
| Shortcuts (F1), About (shows 0.4.2) | pass |
| Idle CPU over 10 s | **0.00–0.23 CPU-seconds** |
| Memory | `nexdir.exe` 28 → 29 MB; with WebView2 child 127 → 158 MB; JS heap 2.7 → 8.2 MB after 10 reopenings (after GC) → ~10 MB at the end — no unbounded growth |
| Console | no errors/exceptions in 2 of 3 runs; **one warning in 1 run** (see §6) |
| Page reloads after start | none |
| Crash / exit | no crash; closes cleanly (exit code 0) |

Not covered by the GUI test (see `docs/release/checklist.md` for the manual steps): drag and
drop, clipboard exchange with Explorer through the GUI (tested earlier with `Set-Clipboard`),
Recycle Bin through the GUI, organize rules, search index build, context menus, keyboard-only
use at the OS level, high-DPI scaling, Windows 10, screen readers.

## 6. Problems found and fixed

During this release check:
- `OWNER` placeholders replaced with `https://github.com/Kervent/nexdir` (issue templates,
  Discord setup links, notifier default); repository URL added to `package.json`/`Cargo.toml`.
- `SECURITY.md`: direct advisory link; safe-copy model described.
- README: alpha notice, link to the releases page; About dialog: concrete issues link.

Earlier on the way to 0.4.2 (all fixed and tested):
- **High** — *Replace* could lose the existing file on cancel/error (0.4.1).
- **High** — temp-file race: cleanup could delete another program's file; path-based backup
  and source deletion (0.4.2).
- **Medium** — device-path spellings passed validation (0.4.0).

Observed, not fixed:
- **Low** — In 2 of 4 GUI runs (one before reload tracking existed, one after) the console showed
  `[TAURI] Couldn't find callback id … app is reloaded while Rust is running an asynchronous
  operation`, always during startup, before the first folder was shown. No reload after start
  was recorded, the UI worked normally in that run, and Nexdir's code does not reload the page.
  Likely the WebView's initial page load replacing a document whose first backend call was
  still pending; possibly influenced by the attached debugger. Not reproducible on demand.

## 7. Performance (this machine, warm cache)

| Measurement | Result |
|---|---|
| List 20,000 entries (backend) | 13 ms |
| Storage analysis, 50,000 files | 76 ms |
| Live search, 50,000 files | 89 ms |
| Index build / query, 50,000 files | 499 ms / 5.2 ms |
| Duplicate scan, 50,000 small files | 2.6 s |
| Copy 5,000 small files (safe copy) | 4.3 s (plain `std::fs::copy`: 2.0 s) |
| Replace 1,000 small files | 2.6 s |
| Replace one 256 MiB file | 275 ms (plain copy 69 ms) |
| GUI: 3 GiB copy speed shown | ~570 MB/s (earlier run; cancelled after a few seconds) |

## 8. Release files (`release-out/`)

| File | Size |
|---|---|
| `Nexdir-0.4.2-Setup.exe` | 2,476,107 bytes |
| `Nexdir-0.4.2.msi` | 3,407,872 bytes |
| `Nexdir-0.4.2-Portable.zip` | 3,095,507 bytes |
| `SHA256SUMS.txt` | 264 bytes |

## 9. SHA-256

```text
93ea5fd384e3c9c1fcbd1b3fd8a01148c702d42a84a71b8329b9762599fe506c  Nexdir-0.4.2.msi
ffd3b11dc0b70e8ca84aa289fb7944994c196beb61fa6e2c4c87a5a4d771840f  Nexdir-0.4.2-Portable.zip
8b4f5af719eb53959fe3ebbf3b0cefa8e7b55c377eb1bf7d7f0c98712016a089  Nexdir-0.4.2-Setup.exe
```

`nexdir.exe` (in `src-tauri/target/release`): `349bcdb1a9cbee49c408bf405ae9392369221196ec2ef46fd5ea97d055c62a5a`

The files under `release-out/` are local builds for testing. The public release should be the
draft built by the Release workflow from the `v0.4.2` tag (its checksums will differ: different
build machine and timestamps).

## 10. Recommendation and remaining risks

**ALPHA READY.**

Why alpha is justified:
- No known open data-loss bug; the three data-safety issues found so far are fixed, covered by
  regression tests that pass repeatedly, and the main ones were also checked in the built app.
- Clean build from scratch; all automated suites green; dependency audits clean; no secrets.
- The GUI ran through all main areas without crash, freeze, leak or functional failure, including
  a large folder and a cancelled 3 GiB copy.

Why not beta yet:
- Tested on one machine only: Windows 11, NTFS, local SSD. **Not tested:** Windows 10, FAT/exFAT
  and USB media, network shares, high-DPI, other Windows languages, cold-cache/HDD performance.
- The 0.4.2 **installer was not run** (only extracted/inspected); the install/uninstall test was
  done with 0.4.0 (unchanged installer apart from the version).
- **Not code-signed**: SmartScreen warnings; users must verify checksums.
- Not crash-proof: after a killed process or power loss `~nexdir-*` files can remain (the
  destination is designed never to be half-written, not proven with real power loss).
- Duplicate cleanup safety ("keep one copy") is enforced in the UI only, on possibly outdated
  scan results (Recycle Bin, recoverable).
- Sporadic startup console warning (§6).
- Symbolic-link tests skipped (need Developer Mode); junction tests cover the protection.
- All reviews were done by the project's development assistant; **no external security audit**.

Before publishing the alpha (needs your decision and action):
1. Create `github.com/Kervent/nexdir`, commit and push (approval required).
2. Apply the repository settings in `docs/release/process.md`, run *Sync labels*.
3. Tag `v0.4.2` → Release workflow creates a **draft**; check it with `docs/release/checklist.md`
   (ideally on a Windows 10 VM), mark it as pre-release, publish.
4. Optional: Discord webhooks for announcements; post the info texts now that links are final.
