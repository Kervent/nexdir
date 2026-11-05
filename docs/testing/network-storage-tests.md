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

Totals on 2026-11-05 (final): Rust 193 passed, 0 failed, 26 ignored (9 loopback SMB tests and 11
Samba tests below, run separately + 6 pre-existing), plus 10 integration tests; frontend 164
passed in 12 files; `cargo clippy --all-targets -D warnings` clean; `cargo fmt --check` clean;
`tsc` clean; third-party notices up to date (no new dependencies); npm audit 0 vulnerabilities;
cargo audit only the 2 pre-existing allowed warnings.

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

## Samba server on a virtual network (acceptance, 2026-11-05)

A real Samba 4.22 server in a dedicated WSL distro, reached by Windows' SMB client over WSL's
host-internal NAT network (SMB 3.1.1, signing). Setup: [tools/network-test](../../tools/network-test/README.md).
Full results: [network-storage-acceptance.md](../reports/network-storage-acceptance.md).

| Area | Result |
|---|---|
| Discovery, shares, manual connection, invalid paths, unreachable hosts | PASS |
| Authentication: Credential Manager, no permission, existing connection, conflict 1219 | PASS |
| File operations both ways, share → share, read-only share, conflicts, drive letter, SHA-256 on server | PASS |
| Outages early/late/during Replace, listing/rename/status during outage, recovery | PASS |
| Slow links (100/20 Mbit/s, latency, loss): progress, Cancel, integrity | PASS |
| Samba GUI test, 17 steps (tabs, split view local↔NAS and NAS↔NAS, favourites incl. rename and offline, outage in split view, slow copy with Cancel) | 17/17 PASS |
| Windows' sign-in dialog with typed credentials | NOT TESTED (manual check below) |

### Manual check: Windows sign-in dialog (needs a person, about 5 minutes)

Nexdir never types credentials into Windows' dialog. To check it with the test server:

1. Remove stored credentials: `tools/network-test/credhelper.ps1 -Action remove-cred`, and make sure
   `net use` lists no connection to the test server.
2. In Nexdir: *Network → Add network share* → `\\<server>\PrivateTest` → *Connect* → shows
   *Sign-in required* → *Sign in…*.
3. Windows' dialog appears (owned by Nexdir). Check: **Cancel** → *Sign-in cancelled*, nothing
   connected; **wrong password** → Windows asks again or Nexdir shows *Sign-in required*; **empty
   user name** → Windows does not accept it; **nxprivate + correct password** (from
   `tools/network-test/secrets/creds.json`) → the share opens.
4. Optional: tick "Remember my credentials" → Credential Manager entry for the server exists;
   remove it afterwards with `credhelper.ps1 -Action remove-cred`.

## Regression (existing GUI suite of 0.5.0)

The release GUI suite (42 steps: navigation, tabs, split view, copy/move/rename/Recycle Bin,
conflicts, 20,000-file folder, duplicates, storage analysis, search, dialogs, help, updates
check), both builds with isolated profiles in the same 1240×780 window:

| Build | Result | Failing steps |
|---|---|---|
| Branch (final) | 42/42 | — |
| Unchanged v0.5.0-alpha.1 | 41/42 | Help topics (the new topic *Network shares and NAS* is not in 0.5.0, expected) |

Root cause of the two *Report a Bug* failures in earlier runs (also on the unchanged release
build): notifications from previous steps lay over the dialog's primary button in this window
size. Fixed in the product (notifications stay beneath modal dialogs). The harness now waits for
an auto-dismissed notification over page content, as a user would; other obstructions still fail.

## NOT TESTED / SKIPPED

| Case | Status | Reason |
|---|---|---|
| NAS hardware (Synology, QNAP, TrueNAS), Samba on a separate physical machine | NOT TESTED | No device available; Samba tested on a virtual network. |
| Windows' sign-in dialog with typed credentials, "remember credentials" | NOT TESTED | Needs a person (manual check above). Cancel and the error mapping are covered. |
| Guest-blocked and signing-incompatible servers | NOT TESTED (real) | The test server enforces signing and refuses guests; mapping tested with test doubles. |
| Server names (name resolution of the test server) | NOT TESTED | Would need hosts/DNS changes. |
| Physical connection loss (cable, Wi-Fi) | NOT TESTED | Simulated by stopping Samba. |
| Windows 10 | NOT TESTED | Only Windows 11 available. |
| NSIS / MSI / portable packages of this branch | SKIPPED | No release planned; branch built with `--no-bundle`. Packaging unchanged. |
| Creating a test share on the main PC | SKIPPED | Not needed (Samba VM); would change sharing settings. |
