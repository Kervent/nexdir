# MSI installation and update acceptance — Nexdir 0.5.0

Date: 2026-11-03. Status: **PASS** · **FAIL** · **SKIPPED** · **NOT TESTED**.

## Test environment

- Windows 11 Pro 26H2 (build 26300), x64, the owner's PC; account in the Administrators group,
  UAC at the default (`ConsentPromptBehaviorAdmin=5`, secure desktop).
- **Windows Sandbox was preferred but could not be used.** The optional feature was enabled with
  the owner's approval and the PC restarted twice, but Windows did not complete it
  (`Containers-DisposableClientVM` *Enabled*, `CmService` and `WindowsSandbox.exe` missing,
  component servicing still reporting a pending reboot — an update from 2026-10-29 has been
  pending since then as well). Repairing Windows servicing was not attempted.
- By the owner's decision the tests then ran **on this PC**. Nexdir was not installed before.
  Nexdir's user data (`%APPDATA%` and `%LOCALAPPDATA%\org.nexdir.desktop`) was moved aside
  before, a verified safety copy was taken, and the original folders were put back afterwards
  (checked by SHA-256 against the safety copy: 0 differences, original creation dates). Every
  per-machine MSI step needed the owner's UAC consent.
- Packages: the **published** `Nexdir-0.5.0-alpha.1.msi` (SHA-256 `65b852cb…`, equal to the
  release's `SHA256SUMS.txt`) and two **test builds** with the `updater-test` feature and a
  throwaway **test key**: A = 0.5.0-alpha.1 (MSI 0.5.0.1), B = 0.5.0-alpha.2 (MSI 0.5.0.2),
  served by a local feed on 127.0.0.1. Error packages were signed with test keys only. The
  production key and the GitHub secrets were not used.

## MSI metadata (published package)

| Property | Value |
|---|---|
| ProductName / Manufacturer | Nexdir / Nexdir contributors |
| ProductVersion | 0.5.0.1 |
| UpgradeCode | `{0C406486-4175-5369-A3EB-C50750955C8E}` (same as 0.4.3) |
| ProductCode / PackageCode | `{5F84A7EA-03C7-45E2-92E0-EDC86BA817DC}` / `{5351F7A0-643D-4529-B2FD-0C1E7AC51394}` |
| Scope | per machine (`ALLUSERS=1`), `C:\Program Files\Nexdir` |
| Upgrade table | all versions ≥ 0 of the UpgradeCode (major upgrade), `RemoveExistingProducts` right after `InstallInitialize` (inside the transaction, so a failure rolls back) |
| Shortcuts | Start menu `Nexdir`, desktop (`C:\Users\Public\Desktop`), "Uninstall Nexdir" |
| Custom actions | `LaunchApplication` (condition `AUTOLAUNCHAPP AND NOT Installed`), WebView2 bootstrapper (only if WebView2 missing), ARP settings |
| Registry / environment tables | none |
| In-app update command (Tauri updater) | `msiexec /i <file> /passive /promptrestart AUTOLAUNCHAPP=True LAUNCHAPPARGS="…"` |

## Results

### Published MSI 0.5.0-alpha.1

| Test | Status | Evidence |
|---|---|---|
| Hash equals the release checksum | PASS | `65b852cb…` |
| Installation (UAC consent) | PASS | exit 0; MSI log: elevation required → consent provided |
| Installed to `C:\Program Files\Nexdir`, one product under the UpgradeCode, version 0.5.0.1 | PASS | Windows Installer API |
| One "Apps" (uninstall) entry | PASS | entry "Nexdir 0.5.0.1" (the first automated check reported FAIL because of a counting bug in the test script, since fixed; its own detail shows exactly one entry) |
| Start menu and desktop shortcuts | PASS | |
| Starts, detected as **MSI** installation, version 0.5.0-alpha.1, can install updates (production key) | PASS | `update_info` |
| Default channel Beta (Preview); automatic checks off; no update request in the first 12 s | PASS | `lastAttempt` unset |
| Real check against GitHub, Preview: up to date | PASS | `latest 0.5.0-alpha.1` |
| Real check, Stable: no stable release | PASS | `noRelease: true` |
| Release build ignores the test feed variables | PASS | 0 requests to 127.0.0.1 |
| No download, no installation without action | PASS | |
| Uninstallation: files, shortcuts, registration removed; user settings kept (by design) | PASS | exit 0 |

### Test build A (MSI), error and security cases

| Test | Status |
|---|---|
| Installs as 0.5.0.1, detected as MSI, can install (test key) | PASS |
| Feed HTTP 404 / HTTP 500 / not JSON / invalid version / connection dropped | PASS (each reported with the right error) |
| Same version and an older version (0.4.9) are not offered (no downgrade) | PASS |
| Modified MSI download → signature rejected, install refused afterwards | PASS |
| MSI signed with another key → rejected | PASS |
| **Per-user setup offered to an MSI installation → rejected by the installer-type check** | PASS |
| Interrupted download → clean error, install refused | PASS |
| Two checks at once → one runs, one refused | PASS |
| Valid MSI downloads and verifies | PASS |
| Installation refused while a 2 GiB copy runs | PASS |
| Repeated download requests start no second download | PASS |

### In-app MSI update A → B

| Test | Status | Evidence |
|---|---|---|
| Update downloaded and verified, Nexdir closes and starts `msiexec` | PASS | |
| **Windows Installer installs 0.5.0.2 in place of 0.5.0.1** | PASS | events 1040 → 11707 "Installation completed successfully" → 1033 version 0.5.0.2, status 0 (14 s) |
| Same UpgradeCode, **exactly one product afterwards**, new ProductCode (`{88DDF56E…}` → `{F3C46D9C…}`), no second installation | PASS | Windows Installer API after the update |
| Administrator prompt **cancelled** → A stays, "not installed" message at next start | **NOT TESTED** | the step was meant to cancel the prompt; the prompt was confirmed (consent granted, the update installed). The automated checks for this step therefore recorded FAIL — they describe the test step, not an app defect. |
| Relaunch of Nexdir by the installer (`AUTOLAUNCHAPP`), settings and version after the update | NOT TESTED (not verified) | a Nexdir data folder was created at the exact second the installation finished (18:12:28), which indicates the relaunch, but the run was stopped before it was checked |
| Validly signed but broken package → Windows Installer fails, B stays | NOT TESTED | run stopped before this step |
| Uninstallation of B | PASS | done during cleanup: exit 0, no files, shortcuts, uninstall entry or registration left |

The NSIS path (per-user setup) was tested end to end on 2026-11-03 with the same updater code
(including relaunch, kept settings and the "updated" message), see
[release-readiness-0.5.0.md](release-readiness-0.5.0.md).

## Problems found

| Problem | Where | Status |
|---|---|---|
| Windows Sandbox could not be set up | this PC's Windows servicing | documented; not repaired |
| Processes started from the desktop app's terminal panel see a virtualized `AppData` (Nexdir folders invisible), so the first attempt moved nothing aside | test environment | test now runs in its own PowerShell window and refuses to start when the data is not visible |
| Test script: COM list from `RelatedProducts` and one-element arrays were mishandled (aborted runs, one false FAIL) | test script | fixed |
| **Test script: after the run was stopped, the restore moved the backup *into* the test data folder** (a folder moved onto an existing folder is nested). The user data was never lost: it was found nested, verified byte for byte against the safety copy and put back. | test script | fixed: test data is renamed aside, never deleted, nesting checked |
| No defect of Nexdir or its MSI package was found | | |

## Not tested / remaining

- Cancelled administrator prompt (expected: Windows Installer returns 1602/1625, Nexdir stays at
  the old version and reports "not installed" at the next start; the "not installed" message
  itself was tested with the NSIS build).
- Broken but validly signed MSI (Windows Installer failure and rollback).
- Verified relaunch and kept settings after an **MSI** in-app update.
- Windows 10.

These need about four UAC interactions; the fixed test script can repeat only these steps.
The published 0.5.0-alpha.1 release was not changed; no test package was published.
