# Network storage: tests

Status values: **PASS**, **FAIL**, **SKIPPED** (deliberately not run, with reason),
**NOT TESTED** (needs an environment that was not available). Results from 2026-11-05 on
Windows 11 Pro 10.0.26300, branch `feature/network-storage-smb`.

## What "real SMB" means here

There was no NAS or second computer available. Real SMB tests use the **Windows SMB client and
server on the same PC** through the built-in administrative share `\\localhost\C$` (loopback,
SMB 3.1.1), with temporary folders of synthetic files. No share was created, and no firewall,
sharing or discovery setting was changed. This proves Nexdir's behaviour over the Windows SMB
stack. It does **not** prove behaviour with NAS hardware, Samba, other SMB dialects, slow links or
real network failures. Those are listed as NOT TESTED.

## How to run

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib             # unit tests (test doubles)
cargo test --manifest-path src-tauri/Cargo.toml --lib smb_ -- --ignored --test-threads=1   # real SMB (loopback)
npx vitest run                                                     # frontend
```

The real SMB tests are `#[ignore]` by default: CI runners and standard user accounts may not reach
`\\localhost\C$`. They skip themselves if the share is not reachable.

## Unit tests (test doubles, run in CI)

| Area | Tests | Result |
|---|---|---|
| UNC parsing: server/share/folder, spellings (`//`, `\\?\UNC\`, `smb://`), Unicode, device namespaces, traversal, `IPC$`, invalid characters, lengths, IPv6 form, 3 leading backslashes | `network::unc` (7) | PASS |
| `validate_path` checks UNC server and share | `security::checks_server_and_share_of_network_paths` | PASS |
| Error codes equal the Windows SDK constants; mapping to statuses; Windows knows the codes | `network::status` (3) | PASS |
| Network errors in file operations get clear messages and codes | `error::maps_network_errors_with_clear_messages` | PASS |
| Gate: results in time, timeout without waiting for a hung call, no new threads when all slots hang, panic releases the slot | `network::pool` (4) | PASS |
| Favourites: only path and label stored, duplicates removed, invalid entries refused/dropped, bounded size and labels | `network::favorites` (4) | PASS |
| Overview: drive + UNC + remembered + favourite merged without duplicates; invalid/server-only entries ignored | `network::tests` | PASS |
| Connect: as Windows user, registry; existing connection reused and not disconnectable; sign-in only on request; conflict / cancelled / guest blocked / unreachable / not found; invalid input refused before any call | `network::tests` | PASS |
| Disconnect only own connections; release at exit only own connections | `network::tests` | PASS |
| Hanging server times out; probe maps errors | `network::tests` | PASS |
| Discovery: servers and real shares only (hidden `$` and duplicates removed, no invented shares on errors), no network list, cancellation | `network::tests` | PASS |
| `is_remote` for UNC and local paths | `network::tests` | PASS |
| Server name accepted with/without `\\` | `commands::network` (1) | PASS |
| Transfer engine: copy-and-delete moves across a share refused (nothing changed), merge moves refused, rename moves inside a share work, copies to a share work and never touch the source | `transfer::tests::network_*` (4) | PASS |
| Frontend: path helpers, move pre-check, status tones, Up from share root, session restore of network locations, Network view (no network calls on open, menu, check, search only on click, connect + sign-in flow, saving, empty state) | `src/features/network/network.test.tsx` (11) | PASS |

Totals on 2026-11-05: Rust 191 passed, 0 failed, 15 ignored (9 real SMB tests below + 6
pre-existing); frontend 163 passed in 12 files; `cargo clippy --all-targets -D warnings` clean;
`cargo fmt --check` clean; `tsc` clean; third-party notices up to date (no new dependencies).

## Real SMB integration tests (Windows loopback)

| Test | Result |
|---|---|
| Network paths recognised (`is_remote`) and accepted by `validate_path` | PASS |
| Listing with Unicode names (`Ordner Ü 日本`, `Résumé 😀.txt`), sizes, dates | PASS |
| New folder and rename on the share | PASS |
| Copy PC → share (incl. 24 MiB file, byte-exact), no `~nexdir-*.partial` leftovers | PASS |
| Conflicts on the share: *Replace* (verified swap), *Keep both* (`a (2).txt`) | PASS |
| Copy share → PC (byte-exact) | PASS |
| Move PC → share and share → PC refused, nothing changed; move inside the share works | PASS |
| Delete on the share refused, file kept | PASS |
| Status: share reachable / folder missing / share missing / unknown host (*Not reachable* after ~30 ms) | PASS |
| Connect as Windows user creates a Nexdir connection; disconnect removes it; connection list back to the previous state | PASS |
| Overview, share list of `localhost` (hidden shares filtered), discovery including the known server | PASS |
| Performance: listing 20,000 files: 14 ms local, 105 ms over SMB; 256 MiB copy PC → share (staged, verified, flushed) 0.7 s; share → PC 0.2 s | PASS (loopback, not representative of real network speed) |

## GUI tests (real app, isolated build)

Built from the branch with its own identifier (`org.nexdir.networktest`), so the owner's Nexdir
profile and settings were not used. Driven through WebView2 DevTools with real mouse/keyboard
events; results checked on disk. Harness kept outside the repository.

| Step | Result |
|---|---|
| Sidebar *Network* opens the Network view (empty state) | PASS |
| *Search the network* runs as a task and finishes | PASS |
| Connect dialog: local path refused, `\\localhost` recognised as server | PASS |
| Connect to a folder on the share as Windows user, save it, open it | PASS |
| Saved share appears in the sidebar | PASS |
| Unicode names listed on the share | PASS |
| Split view: local folder next to the share, copy share → local (no temp leftovers) | PASS |
| Copy local → share (5 MiB, verified on disk) | PASS |
| Move share → local: refused with an explanation before it starts; nothing changed | PASS |
| Delete on the share: refused (no Recycle Bin), file kept | PASS |
| Rename and new folder on the share | PASS |
| `\\localhost` in the address bar shows the server's shares | PASS |
| Unreachable server typed in the address bar: clear message after ~50 ms, UI responsive | PASS |
| Own connection disconnected from the Network view | PASS |
| Network view in dark theme | PASS |
| Restart restores a tab with a network folder (listed ~430 ms after start) | PASS |
| No SMB connection left behind after closing Nexdir | PASS |

Total: 17/17 PASS (final run). Earlier runs found and fixed: share list of a bare server name
failed; connect dialog overflowed with long paths; header buttons wrapped; connections created
by Nexdir stayed after exit; refused moves were only explained in the operation details.

## Regression (existing GUI suite of 0.5.0)

The release GUI suite (42 steps: navigation, tabs, split view, copy/move/rename/Recycle Bin,
conflicts, 20,000-file folder, duplicates, storage analysis, search, dialogs, help, updates
check) was run against the branch build, and for comparison against an unchanged build of tag
`v0.5.0-alpha.1`, both with isolated profiles:

| Build | Result | Failing steps |
|---|---|---|
| Branch | 40/42 | 2× *Report a Bug* (dialog flow) |
| Unchanged v0.5.0-alpha.1 | 38/42 | the same 2× *Report a Bug*, plus 2 first-start steps (its profile had already completed the first start) |

The two *Report a Bug* steps fail identically on the unchanged release build: harness/environment
issue, not caused by this feature. A storage-analysis step failed on the branch only at the
default window height (a notification covered the button the harness clicks); with the same
window size as the baseline it passes. Rust and frontend unit suites (including all pre-existing
tests) pass completely.

## NOT TESTED / SKIPPED

| Case | Status | Reason |
|---|---|---|
| NAS hardware (Synology, QNAP, TrueNAS), Samba on Linux, another Windows PC | NOT TESTED | No device available. |
| Sign-in with different credentials (Windows dialog), "remember credentials" | NOT TESTED | Needs a share that requires other credentials; the loopback share accepts the signed-in user. Flow tested with test doubles. |
| `ERROR_SESSION_CREDENTIAL_CONFLICT`, guest-blocked, signing-required servers | NOT TESTED (real) | Mapping and UI tested with test doubles. |
| Connection loss during a transfer (cable pulled, NAS switched off) | NOT TESTED | Needs a second device. Engine behaviour on failures is covered by the existing staged-copy tests (temporary file removed or reported, existing file intact). |
| Slow links, large files over a real network, many tabs on a slow NAS | NOT TESTED | Bounded waiting proven with test doubles and the gate tests. |
| Mapped drive letters (`Z:`) of real shares | NOT TESTED | No mapped drive on the test PC; merging and display tested with test doubles. |
| Windows 10 | NOT TESTED | Only Windows 11 available. |
| NSIS / MSI / portable packages of this branch | SKIPPED | No release planned yet; the branch was built with `--no-bundle`. Packaging is unchanged. |
| Creating a test share on the main PC | SKIPPED | Would change sharing settings; requires the owner's explicit consent. |
