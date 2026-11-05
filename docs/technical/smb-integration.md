# SMB integration: architecture and security

> Status: in development (`feature/network-storage-smb`), not released. User guide:
> [features/network-storage.md](../features/network-storage.md). Tests:
> [testing/network-storage-tests.md](../testing/network-storage-tests.md).

## Principles

1. **Windows does the networking.** Nexdir calls the Windows network provider API (`mpr.dll`,
   "WNet") and opens paths through the Windows SMB client. It contains no SMB implementation, sends
   no packets of its own and does not change SMB signing, SMB1, guest access, firewall rules, the
   registry or network discovery settings.
2. **No hidden network access.** The overview reads Windows' local connection state. Servers are
   contacted only after a user action (open, check, list shares, search, connect). There is no
   background monitoring, no address-range scan and no port scan.
3. **Bounded blocking.** Every call that can wait for a server runs on a separate thread behind a
   gate with a fixed number of slots and a timeout (see below).
4. **Nexdir never handles credentials.** Sign-in uses Windows' own dialog; Nexdir passes no user
   name or password and never receives one.
5. **Data safety before features.** Operations whose safety on a remote file system is not
   established are refused with an explanation (permanent deletion, copy-and-delete moves).

## Components

| Path | Purpose |
|---|---|
| `src-tauri/src/network/unc.rs` | Parses and validates network paths: `\\server`, `\\server\share`, `\\server\share\folder…`, also `//…`, `\\?\UNC\…`, `smb://…`. Server/share/folder rules, kind detection, canonical form, case-insensitive key. |
| `src-tauri/src/network/wnet.rs` | `NetApi` trait (testable) and `WindowsNet` (real WNet calls). |
| `src-tauri/src/network/pool.rs` | `Gate`: bounded execution of blocking calls with timeouts. |
| `src-tauri/src/network/status.rs` | Win32 error codes → `NetworkStatus` + message (codes checked against the SDK bindings). |
| `src-tauri/src/network/favorites.rs` | Saved locations (`network.json`: path + label only). |
| `src-tauri/src/network/mod.rs` | Overview (merge + dedupe), probe, share listing, discovery, connect, disconnect, release at exit, `is_remote`. |
| `src-tauri/src/commands/network.rs` | IPC commands (`network_*`), each with its own Tauri permission. |
| `src-tauri/src/security/mod.rs` | `validate_path` now also validates the server and share of UNC paths. |
| `src-tauri/src/operations/transfer.rs` | Refuses copy-and-delete moves that involve a network share; flushes copies on shares. |
| `src-tauri/src/error.rs` | Network error codes get clear English messages and error codes. |
| `src/features/network/*` | Network view, connect dialog, path helpers, tests. |

## Windows APIs

| API | Use |
|---|---|
| `WNetOpenEnumW` / `WNetEnumResourceW` / `WNetCloseEnum` | `RESOURCE_CONNECTED` (current connections incl. drive letters), `RESOURCE_REMEMBERED` (remembered drives), `RESOURCE_GLOBALNET` (network list; for a `\\server` container: its shares, which Windows obtains from the server). Buffer grows on `ERROR_MORE_DATA`; at most 4,096 items per enumeration. |
| `WNetAddConnection2W` | Connect as the signed-in user: `RESOURCETYPE_DISK`, no local name, no credentials, `CONNECT_TEMPORARY` (not remembered, no drive letter). |
| `WNetAddConnection3W` | Only after the user clicks *Sign in*: `CONNECT_INTERACTIVE \| CONNECT_PROMPT \| CONNECT_TEMPORARY` with Nexdir's window as owner. Windows shows its own dialog (including "Remember my credentials", stored by Windows in the Credential Manager). |
| `WNetCancelConnection2W` | `fForce = FALSE`, only for connections Nexdir created (registry below). |
| `GetVolumePathNameW` + `GetDriveTypeW` | `is_remote`: drive letters mapped to shares are `DRIVE_REMOTE` (local query). |
| File APIs via the SMB client | Listing, metadata, staged copies (`CreateFileW`, `SetFileInformationByHandle` with `FileRenameInfo` / `FileDispositionInfo`, `FlushFileBuffers`), as for local drives. |

Not used: `WNetGetConnectionW` / `WNetGetUniversalNameW` (the `RESOURCE_CONNECTED` enumeration
already returns drive letter and UNC target), `NetShareEnum` (used by Windows internally when a
server container is enumerated), direct Credential Manager access (left to Windows' dialog).

## Path validation

`unc::parse` (dialog input, favourites, commands) and `security::validate_path` (every path from
the WebView) apply the same rules to network paths:

- Accepted: `\\server`, `\\server\share[\…]`, `//…`, `\\?\UNC\server\share…`, `smb://server/share…`.
- Refused: device namespaces (`\\.\…`, `\\?\` other than `\\?\UNC\`, `\??\…`, `\\.\UNC\…`),
  `..` segments, three leading backslashes, the `IPC$` named-pipe share, servers that are empty,
  only dots, contain whitespace or `@ % , ; = + [ ] { } ^ ` ~ ! # & ' ( )` or a colon (IPv6 must use
  the Windows form `fe80--1.ipv6-literal.net`), share names longer than 80 characters or with
  `< > : " / \ | ? * [ ] + = ; ,`, folder names longer than 255 UTF-16 units or with invalid
  characters, control characters anywhere.
- A bare server is never treated as a folder: commands that need a share refuse it; the UI shows
  the server's share list instead.

## Blocking calls and timeouts

Windows network calls cannot be cancelled from outside, and a "timeout" in the UI does not stop a
blocked Win32 call. Nexdir therefore uses `pool::Gate`:

- Each call runs on its own thread; the caller waits with `recv_timeout`.
- A slot is taken before the thread starts and released only when the call **returns**, not when
  the caller gives up. With all slots held by hung calls, new calls fail at once with "Busy"
  instead of starting more threads. At most 6 threads can ever be blocked in network calls.
- Timeouts: 5 s for local Windows queries, 20 s for a server (check, list shares, connect without
  dialog), 10 min while Windows' sign-in dialog is open, 30 s for listings/previews/address
  resolution of network paths.
- Listings, previews and address resolution of network paths (`commands::fs_work`) use the same
  gate, so an unreachable NAS cannot freeze Nexdir or exhaust the async runtime's blocking pool.
- Discovery is a cancellable background task. Cancellation is checked between calls; a call that
  is already waiting for Windows finishes (or times out) first.

A separate helper process was considered for hard cancellation. It is not needed for the current
calls: the gate bounds both waiting time and the number of blocked threads. If a future API
requires real cancellation, a helper process would be the next step.

Storage analysis, duplicate search and search on a share run as the existing read-only tasks and
are not routed through the gate. A server that stops answering during such a task delays that
task until Windows returns; Stop takes effect afterwards.

## Connections

- `Registry` remembers the shares Nexdir connected itself in this session (case-insensitive).
- *Connect* first checks `RESOURCE_CONNECTED`: an existing Windows connection is reused and marked
  as not Nexdir's; it is never changed or replaced.
- *Disconnect* is offered and accepted only for registry entries, never forced, and refused while
  copy/move operations run.
- At exit Nexdir releases its registry connections (not forced, 3 s per connection).
- `ERROR_SESSION_CREDENTIAL_CONFLICT` (1219) is reported as *Other account in use*; Nexdir does
  not disconnect the other session to make room.
- No automatic reconnection: favourites never trigger connection attempts at start. A restored
  tab with a network folder lists it once (through the gate).

## File operations on shares

| Operation | Behaviour | Why |
|---|---|---|
| Copy (any direction) | Existing staged copy: `CREATE_NEW` temp file, BLAKE3 while reading, read-back verification through the same handle, rename by handle, *Replace* via backup rename. Copies to a share are always flushed (`FlushFileBuffers` → SMB2 FLUSH) before they count as finished. | Handle-based rename and delete are supported over SMB2/3, so the local guarantees carry over: no partial file under the final name, an existing file survives every failure. Limitation: the read-back may be served from the client cache; the flush asks the server to commit, but Nexdir cannot prove the server's disk state. |
| Move inside one share | Rename on the server. | Nothing is deleted. |
| Move across a share boundary (or merge into an existing folder on a share) | Refused in the UI before it starts and again in the engine (`NETWORK_MOVE_REFUSED`). | It would delete the original permanently after a copy whose durability on the server cannot be proven. |
| Delete | Refused (`check_recyclable`: drive type `DRIVE_REMOTE` has no Recycle Bin). | Windows would delete permanently. Also applies to duplicate removal. |
| Organize rules | Moves go through the same engine, so they are refused across shares; copies work. | |
| File identity | Some servers report unstable or zero file IDs. Identity comparisons only ever lead to a refusal ("same file"), never to deleting or overwriting. | Fail-safe direction. |
| Alternate data streams | Copied best effort; servers without stream support simply do not get them. | Existing behaviour. |
| Links/reparse points | Never followed while recursing (existing engine rules). | |

## Error handling

`status::from_code` maps (among others) 5, 53, 59, 64, 67, 85, 86, 121, 1203, 1219, 1222, 1223,
1225, 1231, 1232, 1265, 1272, 1326, 1327, 1330, 1331, 2250, 2401, 2404 to a status and an English
message. A unit test checks every constant against `windows::Win32::Foundation`, and another checks
that Windows knows the codes (`FormatMessage`). Network errors from file operations use the same
messages (`error.rs`). The Win32 code is shown as "Windows error N" for diagnostics. Messages never
contain credentials (Windows does not return them, and Nexdir never has them).

## Permissions (Tauri)

New commands, each with its own `allow-*` permission in `capabilities/main.json`:
`network_parse`, `network_overview`, `network_probe`, `network_list_shares`, `network_discover`,
`network_connect`, `network_disconnect`, `network_favorites_get`, `network_favorites_save`.
None accepts credentials or arbitrary file-system access; all paths are validated.
