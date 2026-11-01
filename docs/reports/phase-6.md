# Phase 6 report — Optimisation and release

Date: 2026-10-31 · Version **0.4.0** · Status: **release candidate built and verified locally;
not published** (publishing needs your approval and a GitHub repository)

## 1. Implemented

| Area | Result |
|---|---|
| Performance | Benchmarks on synthetic data: 50,000-file tree (storage analysis, live search, index build/query, duplicates), 20,000-entry listing, 5,000-file copy with a plain-copy baseline, 100,000-entry sort/filter. Optimised: filter no longer re-sorts the folder (~330 → ~4 ms per keystroke at 100k items); sort keys computed once per entry (type sort 388 → 289 ms). |
| Security review | Two path-validation gaps fixed (device-path spellings, alternate data streams/wildcards/control characters); explicit `asInvoker` manifest; tag/version check in the Release workflow; `npm audit` and `cargo audit` clean. Report: `docs/security/review-2026-10.md`. |
| Full test runs | All suites run (see §4). |
| UI/UX | *About Nexdir* dialog with the version (needed for bug reports, the bug form now says where to find it); file-operations button uses an activity icon instead of a misleading gear. |
| Windows compatibility | Application manifest (Windows 10/11, `longPathAware`, `asInvoker`); app-level tests for >260-character paths (validate → list → copy → move), Unicode/emoji/combining/ligature names, case-only rename; debug test binary runs again on this machine. |
| Installer, portable, checksums | `tools/release/package.ps1` builds Setup.exe, MSI, Portable.zip and `SHA256SUMS.txt` from the Tauri bundles; used locally and by `release.yml`; refuses stale bundles. |
| Release documentation | Release process updated, new smoke-test checklist, portable README with the real data locations, README (system requirements, version), changelog 0.4.0. |
| Code signing preparation | `code-signing.md`: where signing plugs in (`signCommand`, before checksums), options compared, step-by-step plan, verification, "never" list. |

## 2. Files

New: `src-tauri/src/perf.rs`, `src-tauri/src/compat_tests.rs`,
`src-tauri/windows-app-manifest.xml`, `src/features/settings/AboutDialog.tsx`, `src/env.d.ts`,
`src/lib/sort.perf.test.ts`, `tools/release/package.ps1`, `docs/security/review-2026-10.md`,
`docs/release/checklist.md`.
Changed: `security/mod.rs`, `build.rs`, `lib.rs`, `lib/sort.ts`, `useWorkspace.ts`, `App.tsx`,
`App.test.tsx`, `global.css`, `vite.config.ts`, `release.yml`, version 0.4.0 in `package.json`,
`package-lock.json`, `Cargo.toml`, `tauri.conf.json`; docs listed in §1.

## 3. Architecture decisions

- Benchmarks live inside the crate (`#[cfg(test)]`, `#[ignore]`) so they exercise the real
  internal code paths without making modules public.
- Path validation decides on the parsed `Prefix` kind instead of string prefixes — new
  spellings cannot be forgotten.
- One packaging script for local and CI builds, so a locally tested release and the CI release
  contain the same files.
- The app version reaches the UI through a Vite build constant from `package.json`, so it also
  works in tests without the Tauri runtime.

## 4. Test results

| Suite | Result |
|---|---|
| Rust (debug profile, as in CI) | 64 passed, 0 failed, 6 ignored (benchmarks and the real Recycle Bin test) |
| Rust benchmarks (`--ignored`, release) | 3 + 2 passed, numbers in `docs/development/performance.md` |
| `cargo fmt --check`, `cargo clippy -D warnings` | clean |
| Frontend (Vitest) | 68 passed (incl. About dialog, 100k sort/filter budget) |
| `tsc` | clean |
| `tools/discord-notify` | 70 passed |
| `tools/discord-setup` | 40 passed |
| `npm audit` / `cargo audit` | 0 vulnerabilities (2 Linux-only warnings, see review) |

New automated coverage: device-path spellings, alternate data streams, wildcards and control
characters in paths; long paths through the transfer engine; Unicode and unusual names; case-only
rename; About dialog shows the version; sort/filter performance budget.

**Manually tested:**
- Release build, packaging script, independent checksum verification (all 3 match), ZIP content
  (Nexdir.exe, LICENSE.txt, README.txt), version resources (0.4.0 in exe, installer, MSI).
- Embedded manifest content (asInvoker, supportedOS, longPathAware) read from the exe.
- Portable build: extracted from the ZIP and started with a synthetic test folder — window
  opened, folder with a Unicode file name listed, dark theme, new toolbar buttons visible; closed
  normally.
- MSI: administrative extraction (no installation) contains `nexdir.exe` 0.4.0.
- **NSIS installer on this PC (with your approval, 2026-10-31), silent mode (`/S`):**
  - install: exit code 0 in ~1 s, **no administrator prompt**; files in
    `%LOCALAPPDATA%\Nexdir` (`nexdir.exe` 0.4.0, `uninstall.exe`); Start-menu and desktop
    shortcut; per-user uninstall entry (version 0.4.0, publisher "Nexdir contributors"); no
    machine-wide (HKLM) entries;
  - the installed app started on a test folder; *About Nexdir* (opened through Windows UI
    Automation in the real window) showed **Version 0.4.0**; *New folder → Create* created the
    folder on disk and the list updated;
  - uninstall: exit code 0; program folder, shortcuts and uninstall entry removed; user settings
    (`%APPDATA%`/`%LOCALAPPDATA%\org.nexdir.desktop`) and the test files untouched. The
    uninstaller leaves `HKCU\Software\Nexdir contributors\Nexdir` (install path, Tauri default so
    a reinstall reuses the folder); removed by hand after the test.

**Not tested:**
- The installer's interactive pages (only silent mode was run), the uninstaller's "delete app
  data" option, upgrading over an older version, installing the MSI (per-machine, needs admin).
- Windows 10, other DPI settings, other languages of Windows.
- UI frame times with huge folders in the real window (no input automation possible here);
  the About dialog was covered by the automated test, not clicked in the real window.
- Network drives, cold-cache performance.

## 5. Build results

| Artifact | Size | SHA-256 |
|---|---|---|
| `Nexdir-0.4.0-Setup.exe` | 2.47 MB | `56abd725400880fc2b448fdc787d2f0d2dd371081c8d14a68ebb1c922d93f351` |
| `Nexdir-0.4.0.msi` | 3.40 MB | `0241d5a6681dce73d97863996efd83403657d9fb024040f74635a67f48bd5d8c` |
| `Nexdir-0.4.0-Portable.zip` | 3.08 MB | `28cf02540c83d01c10d8d8bb6c2905e8c8fd19323fb9f259ed0517a97b41936f` |
| `nexdir.exe` (in `src-tauri/target/release`) | 8.3 MB | `0d49563beb9153e812608234ce813ca0c4cdd7b4c4944726324ef7eaf28d194d` |

In `release-out/` (ignored by Git). Unsigned. These local files are for testing; the public
release should be the one built by the Release workflow from the tag.

## 6. Security findings

See `docs/security/review-2026-10.md`: 1 medium (device-path spellings) and 1 low (alternate
data streams etc.) — both fixed and tested; three hardening items done.

## 7. Known limitations

- **Superseded build (2026-10-31, later the same day):** a follow-up review found that
  *Replace* could lose the existing file on cancel or error. Fixed in the source
  (`docs/security/safe-copy.md`); the v0.4.0 files listed in §5 were built **before** the fix
  and must not be published. They were replaced by **0.4.1** (see §10).

- Releases are unsigned until a signing option is chosen; SmartScreen will warn.
- The NSIS installer's "Company" version field is empty (the MSI and exe carry the publisher).
- The installer always creates a desktop shortcut in silent mode; the uninstaller keeps one
  registry key with the install path (both Tauri defaults).
- Portable ZIP is not bit-for-bit reproducible (file timestamps in the archive).
- Name sorting of 100,000 items still takes ~0.3 s once per folder load/sort change (collator-bound).
- Carried over from earlier phases: see `docs/ROADMAP.md`.

## 8. Acceptance criteria (master prompt §24)

| Area | Status |
|---|---|
| Desktop: Windows build, navigation, reliable file operations, understandable errors, smooth UI, dark/light, relevant tests, no known critical data-loss bugs | Met as far as testable here (§4 lists what was not tested manually) |
| Branding: official logo, Windows icons, Discord variant, documented colours | Met |
| Discord setup: tool, detection, dry run, roles, channels, safe permissions, idempotent, logo, report, tests | Met against a simulated server; **live run pending your token and approval** |
| Open source: documented code, build guide, license, contribution guide, security policy, reproducible build steps, release process | Met; repository not yet published |

## 9. Next steps (need your decision)

1. Optional: run the interactive installer and the rest of `docs/release/checklist.md` on a
   Windows 10 PC or VM (silent install/uninstall was tested here).
2. GitHub: account/organisation name (replaces `OWNER`), then — with your approval — create the
   repository, commit, push, apply the repository settings, tag `v0.4.0` (draft release only).
3. Discord: bot token for the setup tool's dry run; webhooks for announcements.
4. Choose a code-signing option when the project is public (SignPath Foundation is free for OSS).

## 10. Rebuild as 0.4.1 (data-safety fix)

Date: 2026-10-31. Version 0.4.1 contains the staged copy (`docs/security/safe-copy.md`).

**Tests before the build:** frontend 68 passed, `tsc` clean; Rust 87 passed (5 benchmarks
ignored), `cargo fmt --check` and `clippy -D warnings` clean; `tools/discord-notify` 70 passed;
`tools/discord-setup` 41 passed.

| Artifact | Size | SHA-256 |
|---|---|---|
| `Nexdir-0.4.1-Setup.exe` | 2.36 MiB | `68509279494cf8d013b3b788487f7633282ff9ad5e469be9809bbf24cf859f35` |
| `Nexdir-0.4.1.msi` | 3.25 MiB | `85278d37db34454065537fc999be04841c2d099927c976d4ed763bdc5075e224` |
| `Nexdir-0.4.1-Portable.zip` | 2.9 MB | `28314dccfc8f22f4b0443f5af456aec78188524133eb1ad3b16777ed18e24fdc` |
| `nexdir.exe` (in `src-tauri/target/release`) | — | `8bb9238eb5702f16600ca49377cb3d4831847bbf5721d3ebaedbaf937c38b625` |

**Verified:** all checksums match; ZIP contains `Nexdir.exe`, `LICENSE.txt`, `README.txt`; version
0.4.1 in the exe and the installer; manifest (`asInvoker`, `longPathAware`) embedded; the exe
contains the safe-copy code (`~nexdir-` temp prefix, `ReplaceFileW` import).

**End-to-end in the built portable app (isolated test folders):** a file was put on the Windows
clipboard, *Paste* into a folder with a file of the same name, conflict dialog → *Replace*
(clicked through Windows UI Automation). Result: the destination has the new content, the other
file in the folder is untouched, no `~nexdir-*` files remain.

**Not done for 0.4.1:** installer run (the silent install/uninstall test was done with 0.4.0;
the installer itself is unchanged apart from the version), cancel/error cases in the GUI (covered
by the automated tests), code signing. `Nexdir-Source-Review.zip` on the Desktop still contains
the source before the fix.

## 11. Rebuild as 0.4.2 (race-condition fix)

Date: 2026-10-31. Version 0.4.2 contains the handle-bound staged copy
(`docs/security/safe-copy.md`); 0.4.1 was superseded before publication.

**Tests before the build:** frontend 68 passed, `tsc` clean; Rust 100 passed (5 ignored),
`cargo fmt --check` and `clippy -D warnings` clean; `tools/discord-notify` 70 passed;
`tools/discord-setup` 41 passed.

| Artifact | SHA-256 |
|---|---|
| `Nexdir-0.4.2-Setup.exe` (2.36 MiB) | `1e96633f7080ca4cdb6661aa464371b93ab6a2669d498b1425b31bbd87ddab11` |
| `Nexdir-0.4.2.msi` (3.25 MiB) | `fa4435057fe38921ec3c66bee8f60f8dadd337ca903acab6e543cc33548aa06d` |
| `Nexdir-0.4.2-Portable.zip` (3.0 MB) | `c2fa59f3ac6e1cdc4b5ae712437c3bfe44e60c5adb2949e4541699c1a4ad4b82` |
| `nexdir.exe` | `9148ca8783a1cf1510975bb62ae7cb089a2df1dd9660d16cadfd5d2328b2a2d2` |

**Verified:** checksums match; ZIP contents; version 0.4.2 in exe and installer; the exe imports
`SetFileInformationByHandle` and `FindFirstStreamW` and no longer `CopyFileExW`/`ReplaceFileW`.

**End-to-end in the built portable app (isolated folders):** two files on the clipboard,
*Paste* into a folder with a same-named file, *Replace* (via Windows UI Automation): the existing
file was replaced, the second file was copied **with its `Zone.Identifier` stream** (ZoneId=3),
the unrelated file was untouched, no `~nexdir-*` files remained.

**Review archive:** `Desktop\Nexdir-Source-Review.zip` recreated from the 0.4.2 source:
257 files, 0.82 MB, SHA-256 `26437e56301b996feebcd5110f36a7d35bab7d68f36c45239dd46cbd595df48e`;
secret scan 0 findings; every entry byte-identical to the project.

**Not done for 0.4.2:** installer run (unchanged apart from the version; tested with 0.4.0),
code signing, publishing.
