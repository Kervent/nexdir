# Threat model

Nexdir runs with the user's normal rights and never requests elevation. The main assets are
the user's files; the main risks are data loss and unintended execution.

| Threat | Mitigation | Where |
|---|---|---|
| Compromised or buggy UI sends malicious paths | WebView has no fs/shell plugin; commands validate every path and name (reserved names, invalid characters, length). Paths must be absolute drive or UNC paths (also in `\\?\` form); device namespace paths are rejected by prefix *type*, so no spelling (`//./`, `\\?\globalroot`, volume GUIDs, pipes) gets through; `..`, NUL, control characters, wildcards and `:` (NTFS alternate data streams) in path segments are rejected. Batch size limited. | `security/`, `commands/` |
| Path traversal via names (`..\x`) | Names may not contain separators; `..` rejected. | `security::validate_name` |
| Overwriting files by accident | Create/rename/move use atomic fail-if-exists Win32 calls; replace only with explicit *Replace* policy. | `platform::move_no_replace`, `platform::copy_file_new` |
| Losing the existing file when a replace is cancelled or fails | Staged copy: new temp file → verify → swap with backup → confirm → remove backup; half-failed swaps are rolled back. Destination links/junctions and "same file under another name" are refused. See [safe-copy.md](safe-copy.md). | `operations/safe_copy.rs` |
| Deleting or adopting someone else's file through a reused path (TOCTOU on temp/backup/source names) | Temp files are created with `CREATE_NEW` and held without sharing; renames and deletions go through handles (`SetFileInformationByHandle`), never through paths; a move deletes the source through the handle it was copied from. | `operations/safe_copy.rs`, `platform/win32.rs`, `store.rs` |
| TOCTOU between conflict check and write | Conflict check is advisory; the write itself fails if the target appeared meanwhile. | `operations/transfer.rs` |
| Junction/symlink attacks (copying or deleting outside the selection) | Recursion never follows links; links are skipped and reported. Same-volume moves move the link itself. | `transfer.rs` |
| Copy/move a folder into itself | Canonical containment check before starting. | `transfer.rs` |
| Data loss on cross-volume move | Source file removed only after its verified copy (flushed, size and BLAKE3) is in place under its final name; source folders removed only when empty (`remove_dir`, never `remove_dir_all`). | `transfer.rs`, `safe_copy.rs` |
| Permanent deletion | Only Recycle Bin; non-fixed drives refused; shell asks before nuking (`FOF_WANTNUKEWARNING`). | `operations::recycle_entries` |
| Unintended code execution | Executables, scripts, shortcuts need a confirmation dialog, enforced in the backend (`allow_executable`). Previews (later) will never execute content. | `commands::open_path` |
| Resource exhaustion / hangs | Drive queries time out; listing errors per entry are counted, not fatal; report lists capped. | `drives.rs`, `transfer.rs` |
| Malicious clipboard content from other apps | `CF_HDROP` parsing is bounds-checked; every path is re-validated (absolute, no `..`, no device paths) before use; batch size limited. | `platform/clipboard.rs`, `commands::clipboard_get_files` |
| Malicious file content in preview | Preview never executes or interprets content: images via `<img>` data URL (no script execution, even for SVG), text rendered as plain text, size caps (25 MB images, 128 KB text). | `filesystem/preview.rs`, `PreviewPanel.tsx` |
| Accidental drag and drop | Drops onto the dragged item, into its own subtree or its own folder are rejected; drops go through the same conflict dialog as paste. | `features/dnd` |
| Search index exposing file names/contents | Off until the user chooses folders; stored only in the local app data folder; content indexing is a separate opt-in limited to small text files; one-click deletion; never uploaded. | `search/index.rs` |
| SQL/FTS injection through search text | All values are bound parameters; user words are quoted for FTS5 (`"word"*` with doubled quotes); LIKE wildcards in scope paths are escaped. | `search/index.rs` |
| Duplicate cleanup deleting the last copy | Nothing is deleted by the scan; removal is a separate confirmed Recycle Bin action, and the UI refuses when every copy of a group is selected. | `DuplicatesView.tsx` |
| Organisation rules moving unexpected files | Dry run first, per-file approval, re-check at apply time, destination may not be the source or its parent, hidden/system files and links ignored, normal conflict policy and report. | `operations/organize.rs` |
| Long scans exhausting resources | Scans are cancellable, iterative (no recursion limits), bounded result lists (top-N, 5,000 groups, 50,000 search results). | `analytics/`, `search/` |
| Malicious web content in WebView | Strict CSP, no remote content, `freezePrototype`. | `tauri.conf.json` |
| Privacy | No telemetry. Network access only for update checks to GitHub, manual or opt-in automatic (off by default), sending no file names, paths, device IDs or account data. | `updater/`, `SettingsDialog.tsx` |
| Malicious or tampered update | Feed fetched over HTTPS without redirects, size-limited and fully validated (channel, SemVer, download URLs inside this repository's release for exactly that version); installer verified by the official Tauri updater against the built-in public key with `requireSignedVersion` (signature must name the announced version); never older than the running version; installer type must match the installation (NSIS/MSI); installation only after explicit confirmation and when no file operation runs; portable copies are never modified. | `updater/feed.rs`, `updater/mod.rs`, `tools/release/update_feed.py`, [signing.md](../updates/signing.md) |
| Stolen update signing key | Key only in GitHub Actions secrets and an offline backup; releases are drafts until a maintainer publishes them; the feed lists only published releases; rotation procedure documented. | [signing.md](../updates/signing.md) |
| Unneeded privileges | Application manifest requests `asInvoker`; the installer installs per user. Nexdir never asks for administrator rights. | `windows-app-manifest.xml`, `tauri.conf.json` |
| Vulnerable or tampered dependencies | Locked versions (`package-lock.json`, `Cargo.lock`, pinned Python requirements); actions pinned to commit SHAs; Dependabot; `npm audit` and `cargo audit` are part of the release review. | `.github/`, [security review](review-2026-10.md) |
| Tampered downloads | `SHA256SUMS.txt` with every release; code signing prepared ([code-signing.md](../release/code-signing.md)). | `tools/release/package.ps1` |

A dated review with all findings: [review-2026-10.md](review-2026-10.md).

## Known limitations

- Recycle Bin outcome is determined per item by checking existence afterwards; the shell's
  own per-item error reasons are not yet surfaced.
- Paths longer than 260 characters cannot be recycled through the shell API.
- Symbolic links can only be listed, moved on the same volume and recycled — not copied.
