# Acceptance test: network storage (SMB) with a real Samba server

Date: 2026-11-05 · Branch: `feature/network-storage-smb` · Version unchanged (0.5.0-alpha.1).
Status values: **PASS**, **FAIL**, **SKIPPED**, **NOT TESTED**. This is a **virtual network
integration test** with Samba, not a test with NAS hardware.

## 1. Test environment

| Item | Value |
|---|---|
| Windows | Windows 11 Pro 10.0.26300.9711, standard (not elevated) user session |
| Linux | Dedicated WSL distro `nexdir-samba-test` (Debian 13 "trixie"), WSL 3.0.3, kernel 6.18.54.1; the owner's existing Ubuntu distro was not used |
| Samba | 4.22.11 (Debian package), own configuration ([tools/network-test/smb.conf](../../tools/network-test/smb.conf)) |
| Protocol | SMB 3.1.1 (`SMB3_11`), signing AES-128-GMAC (`smbstatus`), server minimum SMB3, signing required, no guest, no NetBIOS |
| Network | WSL default NAT: server `172.29.37.90/20` on the host-internal virtual switch, Windows side `vEthernet (WSL (Hyper-V firewall))` `172.29.32.1`. Samba bound only to that interface, port 445 only |
| Client | Windows' own SMB client (redirector) through Nexdir's normal code paths; not a mounted WSL folder |
| Isolation | Not reachable from other LAN devices (host-internal NAT, no port forwarding: `netsh interface portproxy` empty). Windows' own SMB server on the host's port 445 is unaffected (separate VM). Windows client policy unchanged (signing enabled, insecure guest logons off) |
| Shares | `PublicTest` (rw: nxtest, nxprivate), `PrivateTest` (nxprivate only), `ReadOnlyTest` (read-only), `LargeFiles` (nxtest) |
| Accounts | Temporary `nxtest`, `nxprivate`, `nxother` (no shell, no home), random 24-character passwords kept only in a scratch file outside the repository |
| Credentials in Windows | Session-only Credential Manager entry for target `172.29.37.90`, written through the Win32 API by the test helper (never on a command line, never printed) |
| App | Isolated build of the branch (`org.nexdir.networktest`); the owner's Nexdir profile was not used |
| Reproducible with | [tools/network-test/](../../tools/network-test/README.md) (`setup.ps1`, `provision.sh`, `credhelper.ps1`, `cleanup.ps1`) |

Note: installing the Samba package started Samba briefly with Debian's default configuration
(all interfaces of the VM, ports 139/445) before the test configuration was applied. This was
only within the host-internal virtual network, with no test accounts or data yet.

## 2. Methodology

- Backend acceptance tests (`src-tauri/src/network/samba_tests.rs`, ignored by default) call
  Nexdir's real network and transfer code against the server. Integrity is checked with SHA-256
  on Windows **and on the server's disk** (`sha256sum` inside the VM), so client caches cannot hide
  errors.
- Outages: `systemctl stop smbd` inside the VM at chosen points of a transfer.
- Slow links: `tc netem` on the VM's interface **in both directions** (egress on `eth0`, ingress
  through an `ifb` device). Only the test VM was shaped; no Windows firewall or network settings
  were changed.
- GUI tests drive the real app through WebView2 DevTools with real input events; outcomes are
  verified on disk and on the server.
- Final run: all phases executed again on the final code (section 9).

## 3. Discovery and connections

| Test | Result |
|---|---|
| Existing connection and mapped drive shown once each in the Network view | PASS |
| Network search finds the server (from known connections) with its 4 shares; hidden shares left out | PASS |
| Windows' own network list (WNet global enumeration) announces the Samba server | NOT TESTED — not expected: NetBIOS is off and Windows 11 has no SMB1 browser; the server is found through known connections. Manual connection works. |
| Search can be stopped; search with the server down reports *Not reachable* (14 ms) | PASS |
| Refresh of the Network view (local query, no server contact) | PASS |
| Manual connection by IP (`\\172.29.37.90\PublicTest`), server-only input shows share list | PASS |
| Invalid / device / traversal paths, `IPC$`, server alone refused before any network call | PASS |
| Share that does not exist → *Not available*; folder that does not exist → *Not available* | PASS |
| Unknown host name → *Not reachable* in 35 ms; address without a host → stops waiting after 20 s (gate) | PASS |
| Server name instead of IP | NOT TESTED — no name resolution for the VM without changing hosts/DNS settings |
| Long paths (614 characters) and Unicode names | PASS |

## 4. Authentication

| Test | Result |
|---|---|
| Windows account unknown to the server → *Sign-in required* (Windows code 86), nothing registered | PASS |
| Anonymous share listing refused by the server (Access denied) | PASS |
| Credentials stored in Windows Credential Manager are used without a dialog (`nxtest`) | PASS |
| Account without permission (`nxtest` on `PrivateTest`) → *Access denied* (5) | PASS |
| Existing connection (as `nxprivate`) reused, reported as existing, never disconnected by Nexdir, survives Nexdir's exit | PASS |
| Second account on the same server → Windows error 1219 (*Other account in use*) | PASS (observed through the Windows API) |
| Sign-in through Windows' own dialog: correct password, wrong password, empty user name, cancel | NOT TESTED — entering credentials into the operating-system dialog was not automated; needs a person (5-minute manual check described in the test document) |
| After a failed sign-in, credentials added later are only used once Windows drops the failed session (observed during the final run; Windows behaviour) | Documented |

## 5. File operations (Windows SMB client ↔ Samba)

| Test | Result |
|---|---|
| List folders, sizes, dates, Unicode names | PASS |
| New folder, rename file and folder on the share | PASS |
| Windows → Samba (folder with 32 MiB file and Unicode names), SHA-256 equal on the server | PASS |
| Samba → Windows, SHA-256 equal | PASS |
| Share A → share B (PublicTest → LargeFiles) and folder A → folder B in a share | PASS |
| 256 MiB both ways, 1,000 small files to Samba, 20,000-file listing, 30-level deep tree | PASS |
| Conflicts on the share: Skip keeps, Keep both adds `(2)`, Replace swaps in the verified copy | PASS |
| No `~nexdir-*.partial` left after successful copies | PASS |
| Move inside a share (rename) works | PASS |
| Move between shares, share ↔ Windows: refused, nothing changed | PASS |
| Delete on the share: refused (no Recycle Bin), file kept | PASS |
| Read-only share: list, read, copy to PC (SHA-256 equal) | PASS |
| Read-only share: new folder, rename, overwrite (Replace), copy into → refused with a clear message; share content hashes unchanged, no partial files | PASS |
| Drive letter `X:` → PublicTest: shown once (merged with the saved share), network drive, list, verified copy, rename-move | PASS |
| Drive letter ↔ UNC of the same share: the engine renames (Windows sees one share); the UI refuses it up front | PASS (safe; UI more conservative than needed) |
| Drive letter removed → entry disappears; mapped again → works | PASS |

## 6. Outages (smbd stopped inside the VM)

| Scenario | Result |
|---|---|
| Copy, early (38 %): fails after ~17 s with "Not reachable …"; source intact; nothing under the final name; leftover temporary file named in the message | PASS |
| Copy, late (12 s into a 15 s transfer, while Windows writes the last part): same as above | PASS |
| Replace of an existing file: fails; the old version on the server is intact | PASS |
| Listing while down: error after ~6 s; rename while down: refused at once; status *Not reachable*; discovery reports it | PASS |
| After restart, the share is usable again (5–32 s, Windows reconnect) | PASS |
| GUI: NAS pane shows the error, the local pane keeps working (10–253 ms), *Try again* recovers | PASS |
| Saved share with the server off: visible after restart (sidebar 421 ms after start), *Not reachable*, then *Connected* when back | PASS |
| Abort during a rename (server stopped exactly between rename steps) | NOT TESTED — rename is a single server request; the outage tests cover "before" (refused) and listing after |
| Physical cable pull / Wi-Fi loss | NOT TESTED (virtual network only) |

## 7. Slow and unstable links (netem, both directions)

| Profile | Nexdir 20 MiB | Windows CopyFile | Cancel |
|---|---|---|---|
| 100 Mbit/s | 1.9 s (11.2 MB/s) | 1.7 s | ≈ 2 ms, nothing left |
| 20 Mbit/s | 8.7 s (2.4 MB/s) | 8.4 s | ≈ 2 ms |
| 40 ms one-way delay | 8.1 s | 1.8 s | ≈ 0.16 s |
| 20 Mbit/s, 40 ± 10 ms, 1 % loss | 15.1 s | 9.7 s | ≈ 0.16 s |
| 80 ± 30 ms jitter (reorders packets) | 44–79 s (TCP-limited, noisy) | — | ≈ 0.3–0.4 s |

GUI at 20 Mbit/s: speed shown 2.34 MB/s, UI answers in 0–1 ms, local navigation 259 ms during the
copy, Cancel 295 ms, no partial file, RAM 157 → 159 MB. **PASS**.

## 8. Bugs found and fixed

| # | Found by | Problem | Fix (commit) |
|---|---|---|---|
| 1 | Samba outage test | A lost server mid-copy produced the localised system text ("Der Netzwerkpfad wurde nicht gefunden") | Copy errors use the shared explanations (`d217580`), regression test |
| 2 | Samba slow-link test | SMB client write-behind: progress jumped, then stood still during the final flush; Cancel ignored (20 Mbit/s: 27 s) | Write-through for temporary files on shares (`d217580`) |
| 3 | Requirements check | Saved shares could not be renamed | *Rename…* for saved locations (`03deef9`), test |
| 4 | GUI regression analysis | Notifications covered the primary button of open dialogs in smaller windows (root cause of the 2 failing *Report a Bug* steps, also on 0.5.0) | Notifications stay beneath modal dialogs (`133d7e9`) |
| 5 | Security review | If Windows could not list connections, Nexdir registered the new connection as its own | Ownership only when Windows confirms no connection existed (`8b71a99`), test |
| 6 | Microsoft docs review | Enumeration buffer growth on `ERROR_MORE_DATA` was not bounded | Always grows, capped at 1 MiB (`8b71a99`) |

## 9. Final run on the final code

| Phase | Result |
|---|---|
| No credentials (2 tests) | PASS |
| Credentials of `nxtest` (4 tests) | PASS — first attempt in the scripted run failed because Windows reused the failed session of the previous phase, a second attempt right after a VM restart hit the network start-up; the clean re-run passed |
| Outages (2 tests, 3 copy scenarios) | PASS |
| Slow links (4 profiles) | PASS |
| Existing connection (1 test), credential conflict 1219 | PASS |
| Drive letter (1 test) | PASS |
| Samba GUI test (17 steps) | 17/17 PASS |
| Loopback SMB tests (9) | PASS |
| Existing GUI regression suite (42 steps) | 42/42 PASS; unchanged 0.5.0 build with the same harness 41/42 (only the new help topic missing, as expected) |
| Rust 193 + 10 integration, frontend 164, fmt, clippy, tsc, build, notices, npm audit (0), cargo audit (2 pre-existing allowed warnings) | PASS |

Performance on the virtual network (final code): listing 1,000 / 10,000 / 20,000 files 16 / 125 /
236 ms; 256 MiB Windows → Samba 1.7 s (write-through), Samba → Windows 0.2 s; 1,000 small files
8.8 s.

## 10. Security review

Checked: UNC validation (device namespaces, `\\?\`, `\??\`, traversal, `IPC$`, invalid names,
three leading backslashes), path validation of every command, symlinks/reparse points (never
followed by the engine; server-side links are resolved by Samba within the share, `wide links`
off by default), credential handling (no command takes credentials; favourites store path + name
only; verified on disk), thread limits (gate, 6 slots), connection ownership (only own,
non-forced, unknown state not claimed), DoS by slow servers (bounded waits; local work unaffected),
no automatic discovery. WNet usage verified against Microsoft Learn (WNetAddConnection2W/3W,
WNetCancelConnection2W, WNetOpenEnumW, WNetEnumResourceW, CreateFileW). Not documented there and
verified empirically: error 1219, write-through behaviour over SMB.

Residual risks: connecting to an untrusted server lets Windows authenticate to it (as with
Explorer); a temporary `.partial` file can remain after an outage; on high-latency links copies
are slower than Explorer; Windows merges deviceless connections to the same share, so Nexdir's
exit cleanup could release a connection the user made to the same share without a drive letter
(never forced).

## 11. Not tested

NAS hardware (Synology, QNAP, TrueNAS), Samba on a separate physical machine, Windows' sign-in
dialog with typed credentials (needs a person), server names (name resolution), physical network
loss, Windows 10, NSIS/MSI/portable packages of this branch (no release build), guest-blocked and
signing-incompatible servers.

## 12. Release recommendation

**Ready for code review and a pull request; not yet for a public release.** Data safety held in
every scenario (no partial file under a final name, no lost source, no damaged existing file,
no permanent deletion). Before a release: the manual sign-in check, at least one NAS device, and
the release packages.
