# Phase 3 report — Intelligent features

Date: 2026-10-30 · Version 0.3.0 · Status: **complete with carry-overs** (see §8)

## 1. Implemented

- **Search**: natural-language box with a deterministic local parser (types, extensions, sizes,
  relative/absolute dates, scopes, folders-only, wildcards, quoted names); interpretation chips;
  location, subfolder and hidden-file options; results in the regular file list with a Folder
  column, context menu, open, middle-click and reveal.
- **Search index** (optional): SQLite FTS5, user-chosen folders, optional content of small text
  files, build/update on demand or on start, delete; rebuilds swap in atomically.
- **Storage analysis**: totals, drive usage, squarified treemap with drill-down, largest
  folders/files, type and extension distribution; cancellable with partial results.
- **Duplicate finder**: size → 64 KiB BLAKE3 → full BLAKE3, streamed; reclaimable space;
  selection helpers; Recycle Bin only after confirmation, never all copies of a group.
- **Organize rules**: conditions (extensions, name, age, size, subfolders), move/copy, conflict
  policy; dry-run preview, per-file approval, confirmation, execution through the operation
  queue with report. Rules persist in `rules.json`.
- Tools as tab locations (sidebar, Home, context menus, Ctrl+Shift+F); inactive tabs stay
  mounted so running scans survive tab switches.
- Background task framework for read-only work.
- "Similar images" (optional in the plan) was not implemented — see §8.

## 2. Files

Backend: `tasks.rs`, `store.rs`, `analytics/{mod,storage,duplicates}.rs`,
`search/{mod,index}.rs`, `operations/organize.rs`, `commands/tools.rs` (16 new commands, each
with its own permission). Frontend: `features/tools/*` (Search, Storage, Duplicates, Organize
views, treemap, task hook), `features/search/queryParser.ts`, workspace/tool locations, styles.
New dependencies: `blake3`, `rusqlite` (bundled SQLite with FTS5).

## 3. Architecture decisions

- Read-only tasks run in parallel on their own threads; file-modifying jobs keep the
  single-worker queue. Organize reuses that queue instead of moving files itself.
- The NL parser lives in the frontend (pure, testable, instant feedback); the backend only
  receives explicit filter values.
- The index is a separate file in the app's local data folder; queries use bound parameters and
  quoted FTS5 prefix terms.
- Tools are workspace locations, so they get tabs, split view and session restore for free.

## 4. Test results

| Suite | Result |
|---|---|
| Rust unit tests | 59 passed, 0 failed (1 ignored) — run in the **release** profile, see §7 |
| Frontend (Vitest) | 62 passed |
| `tsc`, `cargo fmt --check`, `cargo clippy -D warnings` | clean |

New automated coverage: task completion/failure/cancellation with partial result; storage sizes,
junction not double-counted, largest items, categories; duplicates (identical vs same-size,
difference after the first 64 KiB, overlapping roots, junctions, min size); search filters,
wildcards, limit; index build/query (prefix, diacritics, content, wildcards, size, scope with `_`,
quote injection), rebuild, delete; rules (dry run changes nothing, subfolders, conditions,
conflicts, approved-only apply, dangerous destinations); settings store; NL parser (documented
examples, sizes, dates, extensions vs names, scopes, chips); treemap geometry; duplicate
selection safety; UI flows for search, storage via context menu, organize preview → confirm.

**Manually tested:** release app starts with the Tools section (real window). Browser preview
with fake data: storage analysis layout (stats, treemap, bars).

**Not tested manually:** the tools against real data in the real window (no input automation
possible), index build on large folders, organize applied to real files.

## 5. Build results

| Artifact | Size | SHA-256 |
|---|---|---|
| `nexdir.exe` | 7.9 MB | `90ce14f6f2e7f24c068eb7d7f1d2a90dcce4edaa6d3f237a0fa1c35119092527` |
| `Nexdir_0.3.0_x64-setup.exe` | 2.3 MB | `aa20e5d48f70966e561788b0be10b4bdec4bcb9005e5874b6e16586b284c235b` |
| `Nexdir_0.3.0_x64_en-US.msi` | 3.2 MB | `2d45ebea8a1a8aa333c7df65211078b6d5ce1748a5b91f89173b0a9b6cd94182` |

## 6. Security findings

- FTS5 match syntax could be broken by user quotes → terms are quoted and escaped (tested).
- `_` and `%` in folder names would act as LIKE wildcards in index scopes → escaped (tested).
- A rule whose destination is the source's parent could cascade → rejected (tested).

## 7. Known limitations and incidents

- **Windows blocked the debug test binary** (`os error 32`, then `os error 5` on execution) after
  this phase's changes; no process held it, consistent with a security scanner/reputation check.
  No security setting was changed. The same tests pass in the release profile.
- Running release-profile tests in the default target folder overwrote the release
  `nexdir.exe` with a development build; it was rebuilt, and the docs now use a separate
  `--target-dir` for that.
- Search results from the index can be stale until the next update.
- Duplicate finder does not detect hard links (two names for the same file).
- Rules flatten matches from subfolders into one destination folder.

## 8. Carry-overs

Similar-image detection, watched folders for rules, incremental index updates, hard-link
awareness.

## 9. Next steps

Phase 4: Discord setup tool (Python/discord.py) — local tests and dry run only until you
approve changes to the server.
