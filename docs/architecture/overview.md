# Architecture overview

Nexdir is a [Tauri 2](https://tauri.app) desktop application: a Rust backend owns all
filesystem access, and a React/TypeScript UI runs in the system WebView2.

```text
┌──────────────────────────── WebView2 (untrusted UI) ───────────────────────────┐
│ React app (src/)                                                               │
│  app/App.tsx ─ features/{explorer,navigation,transfers,settings} ─ components/ │
│                    │ typed calls only                                          │
│               services/api.ts ──── invoke("list_dir", …) / listen(events)      │
└────────────────────┼───────────────────────────────────────────────────────────┘
                     │ Tauri IPC — only the commands allowed in capabilities/main.json
┌────────────────────┼──────────────── Rust backend (src-tauri/src) ─────────────┐
│ commands/   thin layer: validate input, spawn_blocking, map errors             │
│ security/   path + name validation, executable detection, containment checks  │
│ filesystem/ directory listings, drives (with timeouts), known folders          │
│ operations/ create/rename/recycle, transfer engine, job queue (manager)        │
│ platform/   Win32 wrappers: MoveFileExW, CopyFileExW, IFileOperation,          │
│             ShellExecuteExW, GetVolumeInformationW, reparse tags               │
│ error.rs    AppError { code, message, path } shared by all modules             │
└────────────────────────────────────────────────────────────────────────────────┘
```

## Key decisions

| Decision | Reason |
|---|---|
| **Tauri 2 instead of Electron** | Small binaries, native Rust backend, WebView2 is part of Windows. |
| **No Tauri fs/shell plugins** | The plugins expose generic filesystem/shell APIs to the WebView. Nexdir instead exposes ~18 narrow commands, each enabled via an app-manifest permission (`build.rs`). |
| **Win32 APIs for mutations** | `std::fs::rename`/`copy` replace existing targets silently. `MoveFileExW` without `MOVEFILE_REPLACE_EXISTING` and `CopyFileExW` with `COPY_FILE_FAIL_IF_EXISTS` fail atomically instead (no TOCTOU window). `CopyFileExW` also gives progress, cancellation and cleanup of partial files. |
| **Recycle Bin via `IFileOperation`** | With `FOFX_RECYCLEONDELETE` and `FOF_WANTNUKEWARNING`, Windows asks before anything would be destroyed instead of recycled. Nexdir additionally refuses non-fixed drives up front. |
| **Single worker job queue** | Copy/move jobs run sequentially on one background thread so the UI never blocks and jobs on the same disk do not thrash. Parallel jobs are a later optimisation. |
| **Links are never followed in recursion** | Prevents copying data from outside the selection (junction attacks) and infinite loops. Same-volume moves rename the link itself. |
| **Long paths** | `std::fs` handles long paths automatically; raw Win32 calls in `platform/` add the `\\?\` prefix. |
| **Drive queries with timeouts** | Disconnected network drives can block for a long time; each drive is queried on its own thread with a 1.5 s limit. |
| **Virtualised lists** | `@tanstack/react-virtual` renders only visible rows, so very large folders stay responsive. |
| **UI preferences in WebView storage** | Theme, view mode, sorting and pinned folders are conveniences; a backend settings store will follow with the settings feature. |

## Workspace model (tabs and split view)

`src/features/workspace/workspace.ts` holds the window state as a pure reducer:
`Workspace { tabs, activeTab, panes }`, where each tab references one or two panes and each
pane owns its history, listing, selection and filter. Each mounted `ExplorerPane` loads its
own listing (`usePaneListing`), so inactive tabs cost nothing until shown. Open tabs are
saved to WebView storage and restored on start; invalid saved data is ignored. Window size
and position are persisted by `tauri-plugin-window-state` (Rust side only, no WebView
permission).

## Clipboard, drag and drop, preview

- **Clipboard**: copy/cut writes `CF_HDROP` + `Preferred DropEffect` to the Windows clipboard
  (`platform/clipboard.rs`), so Explorer and Nexdir interoperate. Paths read from the
  clipboard come from other applications and are validated like any other input.
- **Drag and drop** uses HTML5 drag events inside the WebView (`features/dnd`). Drops reuse
  the paste flow (conflict check → job queue). Dropping onto the item itself, into its own
  subtree or into its current folder is rejected in the UI; the backend independently
  refuses copying a folder into itself.
- **Preview** (`filesystem/preview.rs`) only reads: images ≤ 25 MB are returned base64 for an
  `<img>` element (scripts inside SVG do not run there), text is capped at 128 KB and shown
  as plain text by React, other types show metadata only.

## Tools (Phase 3)

Read-only work runs as **background tasks** (`src-tauri/src/tasks.rs`): each on its own
thread, cancellable, reporting throttled `task-progress` events; the result (also a partial
one after cancellation) is fetched with `task_result`. Copy/move jobs keep using the separate
single-worker queue because they modify files.

| Tool | Backend | Notes |
|---|---|---|
| Storage analysis | `analytics/storage.rs` | Iterative walk, folder sizes aggregated bottom-up, top-N heaps; links skipped |
| Duplicates | `analytics/duplicates.rs` | Size → 64 KiB BLAKE3 → full BLAKE3, streamed in 1 MiB chunks; hard links collapsed. Removal: `operations/dedupe.rs` (`recycle_duplicates`) re-verifies and keeps one locked copy per group |
| Search | `search/mod.rs` | Live walk with `Matcher` (terms or wildcards, type, size, date, kind) |
| Index | `search/index.rs` | SQLite + FTS5 (`unicode61`, diacritics removed); rebuild into a new file, then swap |
| Organize | `operations/organize.rs` | `plan` = dry run; `prepare` re-checks confirmed items and creates a normal transfer job |

Tools are **locations** (`{ type: "tool", tool, path? }`), so they open in tabs and split panes
like folders and are restored with the session. The natural-language parser
(`src/features/search/queryParser.ts`) is deterministic and local; it only produces filter
values, and the UI shows how it understood the request.

Configuration files (`rules.json`, `index-settings.json`) live in the app's config folder and
are written atomically (`store.rs`); the index lives in the app's local data folder.

## Help, tour and issue reports (0.4.3)

- **Tour** (`src/features/onboarding/`): steps are data (`TOUR_STEPS`) that name `data-tour`
  attributes on real elements. The overlay measures those elements on every resize/scroll,
  clips highlights to the window and places the card beside them; a step without visible
  elements shows its card centred. State (`nexdir.onboarding.v1` in WebView storage: version,
  offered/skipped/completed, times) is separate from preferences; raising `TOUR_VERSION`
  offers the tour again without touching settings.
- **Help & Support** (`src/features/help/HelpCenter.tsx`) bundles the offline help. External
  pages open only through `open_url`, which accepts nothing but the repository URL tree and
  Discord invites (`security::validate_external_url`). Links live in `src/config/links.ts`; the
  Discord invite comes from the build-time variable `VITE_NEXDIR_DISCORD_INVITE`.
- **Issue reports** (`src/features/help/report.ts`, `IssueDialog.tsx`): form → local cleaning
  (paths, user name, e-mail, secrets) → review → prefilled GitHub issue-form link (field ids
  match `.github/ISSUE_TEMPLATE/*.yml`); above 6,000 characters the report goes through the
  clipboard. `system_info` reads version facts locally. Nexdir makes no network requests.

## Data flow example: paste

1. UI calls `find_conflicts(sources, destination)`.
2. If names collide, the user chooses *keep both*, *skip* or *replace* in a dialog.
3. UI calls `start_operation(kind, sources, destination, policy)` → returns a job id.
4. The worker thread scans sizes, then transfers item by item, emitting
   `operation-progress` events (throttled to ~8/s).
5. When finished, the UI refreshes the folder; problems are listed in an operation report
   (`operation_report(id)`) with every processed, skipped and failed path.

## Planned modules

`indexing/` (SQLite FTS5 search index), `analytics/` (storage analysis, duplicates),
`organize/` (rules engine) — see [ROADMAP](../ROADMAP.md). The Discord setup tool lives in
`tools/discord-setup/` and is not a runtime dependency of the app.
