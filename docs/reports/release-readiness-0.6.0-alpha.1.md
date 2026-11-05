# Release readiness — Nexdir 0.6.0-alpha.1

Date: 2026-11-06. Status values: **PASS** · **FAIL** · **SKIPPED** (deliberately not run, with
reason) · **NOT TESTED** (needs an environment or input that was not available). A test that was
not run is never reported as PASS.

## Summary

| | |
|---|---|
| Version | 0.6.0-alpha.1 (MSI version 0.6.0.1, UpgradeCode unchanged) |
| Previous release | 0.5.0-alpha.1 (tag `v0.5.0-alpha.1`, commit `ccc93d3`) — not changed |
| Starting point | `main` at `f0fd887` (network storage merged via Kervent/nexdir#7) |
| Release branch | `release/0.6.0-alpha.1` |
| Release commit | set when the release PR is merged; the tag must point to that merge commit |
| Main change | Native network shares and NAS (SMB) through Windows' own SMB client |
| Result | **Ready for the release PR and a signed draft release.** No open defect blocks it. Not tested: NAS hardware, Windows 10, physical network loss (see the end). |

## Changes since 0.5.0-alpha.1

- Network area: network drives and connections, saved shares (rename), a server's shares, network
  search on request, connection by path with the Windows account or Windows' own sign-in dialog,
  status checks, context menu; shares in tabs and split view.
- Verified copies to and from shares with write-through (real progress and Cancel on slow links);
  moves across a share boundary and deleting on shares are refused.
- Bounded network calls (6 threads, timeouts); plain-English network errors.
- Fixes: notifications no longer cover dialog buttons (also affected 0.5.0); *Sign in…* offered
  again after Windows' sign-in dialog was cancelled (found in this release test).
- Docs: user guide, technical design, test report, acceptance report, release notes, help topic.

21 commits, 78 files changed since `v0.5.0-alpha.1`. No new dependencies (third-party notices
unchanged).

## GitHub state before the release preparation

| Check | Result |
|---|---|
| PR #7 merged by rebase, all commits by Kervent, no co-author lines | PASS |
| PR checks (12: Frontend, Backend, Security, Dependency review, Windows build, Discord ×2, CodeQL ×5) | PASS |
| CI and CodeQL on `main` after the merge (`f0fd887`) | PASS |
| Open CodeQL alerts / open Dependabot alerts | 0 / 0 |
| Ruleset *Protect main*: PR required, 7 required checks, linear history, no force push, no deletion, no bypass actors | active |
| Ruleset *Protect release tags* (`v*`: no update, deletion or force push) | active |
| Environment `release`: required reviewer, branch policy; secrets `TAURI_SIGNING_PRIVATE_KEY`, `…_PASSWORD` present | PASS |
| Release workflow: manual start with an existing tag, tag must equal the version, refuses existing releases, signing key required, signatures verified, **draft** + pre-release | checked |
| Update-feed workflow: only published releases with valid signatures; pre-releases never in Stable | checked |

## Test environment

- Windows 11 Pro 10.0.26300 (the owner's PC), x64. No VM was available (Hyper-V VMs need
  administrator rights to list; Windows Sandbox is not installed). By the owner's decision the
  installer tests ran on this PC.
- **Test isolation finding:** processes started by the coding assistant run inside the desktop
  app's MSIX container, where `AppData` and `HKCU` writes are redirected to a private copy. An
  installer run from there installs only into that copy. All installer and update tests were
  therefore started through Explorer, outside the container, and checked that they ran there.
  The one installation that had landed in the container was removed again.
- The owner's real Nexdir profile (`%APPDATA%` and `%LOCALAPPDATA%\org.nexdir.desktop`, 2 + 220
  files) was backed up outside the container, verified by SHA-256, and **restored exactly** after
  the tests (0 differences); the test state was moved aside, not deleted.
- Samba 4.22.11 in the dedicated WSL distro `nexdir-samba-test` (Debian 13), listening only on the
  host-internal WSL address `172.29.37.90:445`; SMB3 minimum, signing mandatory, no guest, no
  NetBIOS. Temporary test accounts only; their passwords never appear in the repository, logs,
  reports or screenshots.
- Test builds: 0.5.0-alpha.1 (from its tag) and 0.6.0-alpha.1 (release branch), both with the
  `updater-test` feature and signed with a **temporary test key**, file names prefixed `TEST-`.
  They were never published. The production key was not used and is not able to verify them.

## Windows sign-in dialog (manual input by the owner)

The automation stopped at each Windows dialog; the owner typed the credentials. No password was
read, typed, recorded or captured by the test tooling; no screenshot was taken while a dialog was
open.

| Case | How | Result |
|---|---|---|
| Protected share without a Windows session → *Sign-in required* and *Sign in…* | automatic | PASS (Windows error 86) |
| Cancel in Windows' dialog → *Sign-in was cancelled* (1223), nothing connected | manual + automatic check | PASS |
| Wrong password → Windows asks again; after Cancel nothing is connected | manual + automatic check | PASS |
| Other test account without permission for the share → Windows asks again; nothing connected | manual + automatic check | PASS |
| Correct credentials → dialog closes, the share opens, Windows connection exists | manual + automatic check | PASS |
| "Remember my credentials" not ticked → no Credential Manager entry | automatic | PASS |
| Second account on the same server: *Access denied* as the current session, then *Other account in use* (1219); the existing session is untouched | manual + automatic check | PASS |
| Nexdir releases its own connection when it closes | automatic | PASS |
| No test password in any file of the test profile, logs or screenshots (315 files, UTF-8 and UTF-16) | automatic | PASS |

Finding fixed during this test: after cancelling the dialog, *Sign in…* was not offered again
(only *Try again*). Fixed, with a regression test.

Notes: Windows' dialog handles a wrong password or a refused account itself by asking again, so
Nexdir only sees the final result (here: cancelled). "Remember my credentials" ticked was not
tested (it is Windows' own Credential Manager feature).

## Installer tests (test builds, outside the container)

### Per-user setup (NSIS)

| Test | Result |
|---|---|
| Pre-checks: no Nexdir installed, not running, profile backup present, outside the container | PASS |
| `Setup.exe /S`: exit 0, one uninstall entry in HKCU, version 0.6.0-alpha.1, `%LOCALAPPDATA%\Nexdir\nexdir.exe` | PASS |
| Start menu shortcut | PASS |
| Starts with the existing profile; *Network* in the sidebar; update settings unchanged | PASS |
| Network GUI scenario against Samba with the installed build (17 steps: existing connection and X: drive shown once, search with Stop, connect and save, tabs, split view local↔NAS and NAS↔NAS, copy both ways with SHA-256, rename, read-only share, refused move and delete, outage during a copy in split view, slow copy with Cancel, favourites incl. rename and offline, removal) | PASS (17/17) |
| Restart: all settings keys of the profile present | PASS |
| `uninstall.exe /S`: entry, program files and shortcuts removed; user profile kept; no SMB connection left | PASS |

### MSI (per machine, UAC confirmed by the owner)

| Test | Result |
|---|---|
| 0.5.0-alpha.1 test MSI installs per machine to `C:\Program Files\Nexdir`, one product under UpgradeCode `{0C406486-4175-5369-A3EB-C50750955C8E}` | PASS |
| ProductCode 0.5.0 `{1942A7D6-…}` → 0.6.0 `{7D04E625-…}`, ProductVersion 0.5.0.1 → 0.6.0.1, same UpgradeCode | PASS |
| In-app update (below) → exactly one product afterwards | PASS |
| Uninstall: entry, files, shortcuts removed; no product under the UpgradeCode; profile kept | PASS |

**Finding (not a regression, documented):** the per-user setup leaves
`HKCU\Software\Nexdir contributors\Nexdir` (its install folder) after uninstalling. Tauri's MSI
template deliberately reads this value as `INSTALLDIR` so that switching from the setup to the
MSI keeps the folder. With the setup already uninstalled, a later MSI then installs per machine
into `%LOCALAPPDATA%\Nexdir` (observed in the first MSI run). The installer templates are
unchanged since 0.5.0, so this is not new in 0.6.0. Workaround: delete that key before installing the MSI. A fix (an NSIS
uninstall hook that removes the key) needs its own installer test and is planned for a later
version. For the clean MSI test the key was exported, set aside and restored exactly afterwards.

### Portable ZIP

| Test | Result |
|---|---|
| Contents: `Nexdir.exe`, `LICENSE.txt`, `THIRD-PARTY-NOTICES.txt`, `README.txt` | PASS |
| Starts without installation, detected as portable, cannot install updates itself, *Network* present | PASS |
| A newer version (test entry 0.6.0-alpha.2) is announced only; nothing downloaded | PASS |
| Network GUI scenario (17 steps) with the portable build | PASS (17/17) |
| No uninstall entry, program folder or shortcut; nothing written next to the exe | PASS |

## Update simulation 0.5.0-alpha.1 → 0.6.0-alpha.1 (test key, local feed)

Both builds trust only the temporary test key; the feed was served on `127.0.0.1`. **A published
0.5.0-alpha.1 installation would reject these test packages** (verified: the 0.6.0 test package
fails verification against the production key).

| Test | NSIS | MSI |
|---|---|---|
| 0.5.0 detected with the right installer type, can install, channel Beta (Preview) | PASS | PASS |
| Opening *Check for Updates* finds 0.6.0-alpha.1, shows date, channel, size and the release notes | PASS | PASS |
| Download, signature *Verified*, *Install Update…* → *Close Nexdir and Install* | PASS | PASS |
| Nexdir closes (≈0.26 s); installer replaces 0.5.0 by 0.6.0 in place (≈2 s) | PASS | PASS (UAC) |
| Installer restarts Nexdir | PASS | PASS |
| One-time message *Nexdir was updated to 0.6.0-alpha.1* | PASS (1.0 s after closing) | NOT TESTED (restarted by Windows Installer without the test's debugging connection) |
| After the update: version 0.6.0-alpha.1, same installer type, result reported only once | PASS | PASS |
| *Network* present; all settings keys, channel and automatic-check settings kept | PASS | PASS |
| Download progress percentages | NOT TESTED (3–5 MB from localhost finished between two samples; covered by `updater_signatures` tests) | same |

Signature checks of the test packages (with `tools/release/update_feed.py --verify`): all four
installers verify against the test key for their own version (PASS); rejected with the production
key, with another file's signature, and when a 0.5.0 package is presented as 0.6.0 (PASS).

The real path — published 0.6.0-alpha.1 signed with the production key, offered by the real
preview feed to an installed 0.5.0-alpha.1 — can only be tested after publication (see *After
publication*).

## Regression

| Suite | Result |
|---|---|
| `cargo fmt --check`, `cargo clippy --all-targets -D warnings` | PASS |
| Rust unit tests | PASS (193 passed, 26 ignored: real SMB/Samba tests run separately below, 6 pre-existing) |
| Rust integration tests (`updater_signatures` and others) | PASS (10) |
| TypeScript `tsc --noEmit` | PASS |
| Frontend tests (vitest) | PASS (165, 12 files; +1 for the sign-in fix) |
| Frontend production build | PASS |
| Third-party notices and license index up to date | PASS |
| `npm audit` | PASS (0 vulnerabilities) |
| `cargo audit` | PASS (the 2 pre-existing, explicitly allowed warnings: `proc-macro-error` unmaintained, `glib` unsound) |
| Release tool tests (`tools/release/tests`, feed generator and signature checks) | PASS (11) |
| Samba integration tests on the release code: without credentials (2), with credentials (4), outages (2), slow links at 100 / 20 Mbit/s, 40 ms delay, 20 Mbit/s + delay + 1% loss (4), existing connection (1), drive letter (1) | PASS (14/14; see note) |
| GUI regression suite of 0.5.0 on the release build (42 steps: navigation, tabs, split view, copy/move/rename/Recycle Bin, conflicts, 20,000-file folder, duplicates, storage analysis, search, dialogs, help, update check) | PASS (42/42) |
| Network GUI scenario against Samba (17 steps), installed per-user build and portable build | PASS (17/17 each) |
| Installer builds (NSIS, MSI) with updater artifacts | PASS (test key) |

Note on the Samba run: the first runs failed in the phases that need a particular Windows
session state, not in Nexdir: (1) an idle Windows session as `nxprivate`, left by the installer
tests minutes earlier, made the no-credentials test connect and the `nxtest` tests get *Access
denied*; (2) after the no-credentials phase Windows kept using the refused anonymous logon for
this server (1326), even after Samba was restarted, although the stored test credential was
valid (checked on the server). The test script now restarts Samba in the test VM between phases;
the credential phase passed 4/4 when run after the client state had cleared (as in the
acceptance run). Every other phase passed in the same complete run.

## Security review

- No credentials pass through Nexdir; the sign-in test confirmed no password in any file.
- No change to Windows security settings (SMB1, guest, signing, firewall, discovery) by Nexdir or
  the tests; the test server is reachable only on the host-internal WSL network.
- Test connections, the test drive letter and the session credential were removed after each
  test; `net use` and `cmdkey` show nothing for the test server.
- Signing: the production key was not used locally and not printed; release signatures are
  created only by the release workflow in the protected `release` environment. The `.sig`
  format binds each signature to its version (`requireSignedVersion`), so signatures of
  0.5.0-alpha.1 cannot be reused for 0.6.0-alpha.1.
- Windows Authenticode code signing is a separate mechanism and is still not in place
  (documented in the release notes).

## Known limitations (assessed)

### `~nexdir-….partial` after an outage

Nexdir writes each file under a temporary name and deletes it only through its own open handle;
an outage invalidates that handle, so the temporary file can remain on the share. A clean-up
function was assessed and **not** added to this release:

- The name pattern identifies Nexdir's temporary files, but not *which* copy they belong to:
  another Nexdir window or another PC may be writing one right now.
- Shares have no Recycle Bin; removing the file is a permanent delete, which Nexdir never does
  without a verified, explicit decision.
- A safe design would only list leftovers on a reconnected share, show their age and size, prove
  that no other client has them open (exclusive open), and delete only after confirmation. That
  needs its own tests and is planned for a later version (ROADMAP). Until then the error message
  names the file, and the user deletes it in Explorer.

### Speed on high-latency links

Cause: copies to a share use write-through and write one 1 MiB chunk at a time, so exactly one
write is in flight; each waits a full round trip for the server's acknowledgement. Explorer's
`CopyFile` keeps several writes in flight without write-through. On bandwidth-limited links the
throughput is the same (20 MiB at 20 Mbit/s: 8.7 s vs 8.4 s); at 40 ms one-way delay Nexdir needs
8.1 s vs 1.8 s for 20 MiB.

Options that keep integrity, progress, Cancel and existing-file protection: overlapped writes
with a small fixed number in flight (still write-through, progress counted on completion, Cancel
waits for at most those writes). Larger chunks (4 and 8 MiB) were tried during the acceptance tests
and reverted. Not changed for this release: a new write path in the copy engine
needs its own outage and integrity tests. Planned for a later version.

### Others

- Windows' network list is often empty; known servers and manual paths always work.
- Drive letters cannot be assigned from Nexdir; moves between a drive letter and the UNC path of
  the same share are refused (copy instead).
- MSI after an uninstalled per-user setup installs into the old setup folder (see above).

## Not tested

| Area | Status | Reason |
|---|---|---|
| NAS hardware (Synology, QNAP, TrueNAS), Windows file shares of another PC over a physical network | NOT TESTED | No device available |
| Windows 10 | NOT TESTED | Only Windows 11 available |
| Server names (name resolution), physical cable/Wi-Fi loss | NOT TESTED | Would need DNS/hosts changes or another machine; outages were simulated by stopping Samba |
| "Remember my credentials" in Windows' dialog | NOT TESTED | Windows feature; not needed for Nexdir's behaviour |
| One-time "updated" message after an MSI update | NOT TESTED | See above; same code passed in the NSIS path |
| Real update path with the production key and the real preview feed | NOT TESTED yet | Only possible after publication |
| Cancelled UAC prompt during an MSI update, broken but validly signed MSI | NOT TESTED | Carried over from 0.5.0 |

## After publication (separate test)

1. Check the published files against `SHA256SUMS.txt` and verify both `.sig` files against the
   production public key for version 0.6.0-alpha.1 (`update_feed.py --verify`).
2. Check the *Update feed* run: `preview.json` names 0.6.0-alpha.1, `stable.json` still has no
   release.
3. On a test installation of the **published** 0.5.0-alpha.1 (NSIS, and MSI if possible): *Check
   for Updates* finds 0.6.0-alpha.1, download, verify, install, restart, settings kept.
4. The owner's own installation is not updated by the tests.
