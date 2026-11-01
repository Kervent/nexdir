# Phase 2 report — Complete file management

Date: 2026-10-30 · Version 0.2.0 · Status: **complete with carry-overs** (see §8)

## 1. Implemented

- **Tabs**: new/close/switch/reorder, middle-click to open a folder in a background tab,
  session restore of all tabs and split panes, window size/position remembered.
- **Split view**: two independent panes per tab, active-pane indicator, F6 to switch,
  "Open / Copy / Move to other pane" context actions.
- **Drag and drop** inside Nexdir: onto folders, empty space of another pane, sidebar
  entries and breadcrumbs; Explorer semantics for move vs. copy, Ctrl/Shift overrides;
  invalid targets (self, own subtree, own folder) rejected with no drop highlight.
- **Windows clipboard**: copy/cut/paste interoperates with Explorer (`CF_HDROP` +
  `Preferred DropEffect`); cut items dimmed.
- **Preview pane**: images (incl. SVG via `<img>`), text (UTF-8/UTF-16, 128 KB), folder item
  counts, metadata and attributes; debounced, read-only.
- **Undo** of the last rename (Ctrl+Z).
- **Operations panel**: transfer speed and time remaining.
- **Shortcuts**: Ctrl+T/W/Tab/1…9, Ctrl+\, F6, Alt+P, Ctrl+D, Ctrl+Z, F1 overview dialog.
- Pinned folders (existing) now accept drops and Ctrl+D.

## 2. Files

New: `src/features/workspace/{workspace.ts,useWorkspace.ts,TabBar.tsx}`,
`src/features/explorer/ExplorerPane.tsx`, `src/features/dnd/dnd.ts`,
`src/features/preview/PreviewPanel.tsx`, `src/features/settings/ShortcutsDialog.tsx`,
`src-tauri/src/platform/clipboard.rs`, `src-tauri/src/filesystem/preview.rs`, tests.
Reworked: `App.tsx`, `FileView.tsx`, `Sidebar.tsx`, `AddressBar.tsx`, operations panel,
styles. Removed: `useExplorer.ts` (replaced by the workspace reducer).

## 3. Architecture decisions

- Window state is a **pure reducer** (`workspace.ts`) — tabs, panes, history and selection
  are unit-tested without React or the backend.
- Each pane loads its own listing; inactive tabs keep their state but do not poll.
- Drag and drop uses **HTML5 events inside the WebView** (Tauri's native file-drop stays
  disabled so internal drag and drop works). Consequence: no drag and drop with Explorer yet.
- The Windows clipboard is written and read in Rust; clipboard paths are untrusted input
  and validated like UI input.
- Columns use **CSS container queries** so each pane adapts to its own width.
- New IPC commands: `startup_location`, `clipboard_set_files`, `clipboard_get_files`,
  `preview_path` — each with its own permission.

## 4. Test results

| Suite | Result |
|---|---|
| Rust unit tests | 43 passed, 0 failed (1 ignored: real Recycle Bin) |
| Frontend (Vitest) | 45 passed — incl. 8 workspace reducer, 7 dnd/rate, 10 Phase-2 integration tests |
| `tsc`, `cargo fmt --check`, `cargo clippy -D warnings` | clean |

New automated coverage: tab open/switch/close, background tab via middle click, session
restore, split + copy to other pane, Windows clipboard copy (cut flag) and paste from
external paths, drag-and-drop move onto a folder, rejection of drop into itself, preview
renders text as text (no HTML), undo rename, shortcuts dialog, CF_HDROP encode/decode incl.
malformed buffers and ANSI lists, text/image/binary/folder previews, UTF-16 decoding.

**Manually tested:** real app (release build) starts with tab bar and per-pane navigation on a
test folder. Browser preview with fake backend: split view with two panes, active-pane
highlight, responsive columns, image preview with metadata.

**Not tested manually:** clipboard exchange with Explorer on the real system clipboard
(not exercised to avoid overwriting your clipboard), drag and drop in the real WebView2
window, window-state persistence across restarts, high-DPI, Windows 10.

## 5. Build results

| Artifact | Size | SHA-256 |
|---|---|---|
| `nexdir.exe` | 6.3 MB | `b461e2513e2ef7a2aea8f95b9585c61fc30b8bcfa4b1ed0010bb0a91ca1731b2` |
| `Nexdir_0.2.0_x64-setup.exe` | 1.7 MB | `eb282110aefeb4b081b831cdb149e22221c10d17c428a236ab0c4ee8c0090231` |
| `Nexdir_0.2.0_x64_en-US.msi` | 2.4 MB | `411ca4940fa2e8f25cddd46d10a40d358c8e7a8d5c4bed649f06f7155649972c` |

Unsigned; installers built but not installed.

## 6. Security findings

- Clipboard data from other applications is untrusted → bounds-checked parsing and full path
  validation.
- SVG previews could contain scripts → rendered only through `<img>`, where scripts never run.
- Found during review: split-pane columns could overflow into the neighbouring pane; fixed
  with container queries.

## 7. Known limitations

- No drag and drop to/from Windows Explorer; no thumbnails; no PDF/video preview.
- After a cut-and-paste, the Windows clipboard still lists the moved files; pasting them again
  reports "not found" errors instead of doing anything harmful.
- Undo covers renames only.
- Tab session and preferences live in WebView storage.

## 8. Carry-overs to later releases

Explorer drag and drop (native OLE), thumbnails, PDF/media preview, inline rename, more view
modes, reordering pinned folders, parallel operations, undo for same-volume moves.

## 9. Next steps

Phase 3: storage analysis, duplicate finder (BLAKE3), recursive search with filters,
optional SQLite FTS5 index, deterministic natural-language queries, organisation rules
with dry run.
