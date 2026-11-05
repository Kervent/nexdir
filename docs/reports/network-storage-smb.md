# Report: native NAS and SMB share integration

Date: 2026-11-05 · Branch: `feature/network-storage-smb` (based on `main` `cac596c`) · Version
unchanged: 0.5.0-alpha.1 (a later release, possibly 0.6.0, is not decided). Nothing pushed or
published.

## 1. Summary

Nexdir gets a **Network** area: it shows the network drives and connections Windows already has,
saved shares (visible offline), the shares of a server, and an on-demand network search with
progress and Stop. Shares are connected manually by path (`\\NAS\Media`, also `//…`, `smb://…`)
with the signed-in Windows user; if a share needs other credentials, Windows' own sign-in dialog
is used. Nexdir never handles passwords. Shares open in tabs and split view; listing, opening,
new folder, rename, properties and verified copies in all directions work. Permanent deletion and
copy-and-delete moves across a share boundary are refused with an explanation, because shares
have no Recycle Bin and the durability of a copy on the server cannot be proven. All network calls
are bounded in time and in the number of threads that can block.

## 2. New components

Backend: `network::{unc, wnet, pool, status, favorites}`, `network` (overview, probe, discovery,
connect/disconnect, release at exit), `commands::network` (9 commands with permissions).
Frontend: `features/network/{NetworkView, ConnectDialog, networkPaths}`, `types/network.ts`, a
Network location type, a sidebar section, a help topic. Details:
[technical/smb-integration.md](../technical/smb-integration.md).

## 3. Changed files

- Backend: `Cargo.toml` (feature `Win32_NetworkManagement_WNet` of the existing `windows`
  crate; no new crates), `build.rs`, `capabilities/main.json`, `lib.rs` (handlers, release at
  exit), `commands/mod.rs` (gate for network listings/previews/address resolution), `error.rs`,
  `security/mod.rs`, `store.rs`, `tasks.rs`, `operations/safe_copy.rs`,
  `operations/transfer.rs`, new `network/*`, `commands/network.rs`, generated permission files.
- Frontend: `app/App.tsx`, `ExplorerPane.tsx`, `AddressBar.tsx`, `Sidebar.tsx`, `TabBar.tsx`,
  `workspace.ts`, `services/api.ts`, `styles/global.css`, `HelpCenter.tsx`, new
  `features/network/*`, `types/network.ts`.
- Docs: `docs/features/network-storage.md`, `docs/technical/smb-integration.md`,
  `docs/testing/network-storage-tests.md`, this report, `CHANGELOG.md` (Unreleased).

## 4. Windows APIs

`WNetOpenEnumW`, `WNetEnumResourceW`, `WNetCloseEnum` (connected, remembered, network list, shares
of a server), `WNetAddConnection2W` (signed-in user, temporary, no drive letter),
`WNetAddConnection3W` (Windows sign-in dialog on request), `WNetCancelConnection2W` (not forced,
own connections only), `GetVolumePathNameW`/`GetDriveTypeW` (network drive detection), the
existing handle-based file APIs over the SMB client. Error codes verified against the Windows SDK
bindings.

## 5. Supported SMB functions

Detect mapped drives and connections · show remembered (disconnected) drives · show the shares of
a server · network search (Windows' list + known servers) · manual connection by path · sign-in
through Windows · saved shares · status checks · open in tab / other pane · list, preview, open,
properties · new folder, rename · verified copies PC↔share and share↔share · move (rename) inside
a share · disconnect own connections · release own connections at exit.

## 6. Test environment

Windows 11 Pro 10.0.26300, one PC, no NAS. Real SMB via the Windows SMB client and server on the
same PC (`\\localhost\C$`, temporary synthetic folders; no share created, no settings changed).
GUI tests with an isolated build (`org.nexdir.networktest`); the owner's profile was not used.

## 7. Unit tests

Rust: 191 passed, 0 failed (41 new network-related tests, plus 15 ignored = 9 real SMB tests run
separately + 6 pre-existing). Frontend: 163 passed (11 new). clippy, rustfmt, tsc clean; license
notices unchanged. **PASS**

## 8. Integration tests

Real SMB (loopback): 9/9 **PASS** (listing with Unicode, create/rename, copies both ways incl.
24 MiB and 256 MiB byte-exact, Replace/Keep both on the share, refused moves and deletes with
nothing changed, statuses, connect/disconnect, overview/discovery, performance).
GUI (real app): 17/17 **PASS** in the final run (after fixing 5 issues the GUI test found).

## 9. Security tests

**PASS**: device namespaces and `\\?\` / `\??\` / `\\.\UNC\` spellings refused; traversal (`..`)
refused; `IPC$` refused; invalid server/share/folder names refused (also in `validate_path`);
favourites store no credentials; no command takes credentials; existing Windows connections are
never changed or disconnected; no permanent deletion on shares; no copy-and-delete across shares;
no unbounded thread growth on hung servers; no network access when opening the overview.
**NOT TESTED** on real hardware: guest-blocked and signing-required servers, credential
conflicts (covered by test doubles).

## 10. Impact on existing file operations

- Copies to a network location are now always flushed before they count as finished.
- Moves that would copy and delete across a share boundary (or merge into an existing folder on a
  share) are refused; previously the engine fell back to copy + delete. Local behaviour is
  unchanged.
- Listings, previews and address resolution of network paths run through the bounded network gate
  (30 s timeout) instead of the general blocking pool.
- `validate_path` refuses invalid UNC server/share names and `IPC$` (previously accepted).
- Network error codes produce English messages instead of the localised system text.
- Recycle Bin behaviour unchanged (network locations were already refused).

## 11. Performance

Over SMB loopback: listing 20,000 files 105 ms (local 14 ms); 256 MiB PC → share 0.7 s (staged,
verified, flushed), share → PC 0.2 s; unknown server reported as not reachable after ~30–55 ms;
restart with a network tab lists it ~430 ms after start; overview open without any server
contact. Loopback has no network latency: real NAS speed will be lower and depends on the link.

## 12. Not supported

FTP/FTPS/SFTP, WebDAV, NFS, cloud drives, general device or printer discovery, IP range or port
scans, assigning drive letters, permanent deletion on shares, moves across a share boundary,
automatic reconnection of favourites.

## 13. Known limitations

- Windows' network list is often empty on current Windows; discovery then shows only known
  servers. Manual connection by path always works.
- Verification of a copy on a share reads back through the client; the SMB client may serve it
  from its cache. The flush asks the server to commit, but Nexdir cannot prove the server's disk
  state. (This is why delete-after-copy moves stay disabled.)
- Storage analysis, duplicate search and search on a share are not routed through the gate; a
  server that stops answering delays that task until Windows returns.
- Servers that report unstable file IDs make *Replace* refuse ("same file") rather than risk data.
- Pre-existing GUI-suite steps for *Report a Bug* fail in this environment on the branch and on the
  unchanged release build alike.

## 14. Real NAS tests still required

Synology, QNAP, TrueNAS (SMB), Samba on Linux, a second Windows PC; sign-in with other
credentials and "remember"; credential conflict; guest-blocked and signing-required servers;
connection loss during a copy (cable, NAS off); slow links and large files; mapped drive letters;
Windows 10; NSIS/MSI/portable packages of the release candidate.

## 15. Release readiness

**Ready for review and for testing with real NAS devices; not ready for a release yet.** The
feature is complete for the planned scope, data-safe by design (no permanent deletion, no
copy-and-delete across shares, existing connections untouched) and fully covered by unit, real-SMB
loopback and GUI tests. Before a release (e.g. 0.6.0), the tests in section 14 should run on at
least one real NAS and one Samba server, and the version, release notes and README would need
updating then. Nothing has been pushed; a pull request is prepared on request.
