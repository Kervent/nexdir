# Phase 1 report — Project foundation

Date: 2026-10-30 · Version 0.1.0 · Status: **complete** (logo concept A approved 2026-10-30)

## 1. Implemented

- Tauri 2.12 / Rust 1.99 / React 19 / TypeScript 7 / Vite 8 project with the documented structure.
- Branding: SVG master logo ("Next Directory"), mark + mono variants, 2 alternative concepts,
  PNG 16–1024, ICO, Tauri icon set, Discord icon (full-bleed 512/1024), GitHub social preview,
  generator script (`npm run branding`).
- Design system (`src/styles/tokens.css`): colours, states, typography, spacing, radii, icon
  sizes, elevation; light, dark and system themes.
- Home page (quick access, pinned, drives with usage/file system/availability), sidebar,
  back/forward/up/refresh, breadcrumb + editable address bar (`%ENV%`, `~`, file → select),
  `nexdir.exe <path>` start argument.
- File list: details + icon views, virtualised, natural sort with folders first, filter,
  hidden toggle, multi-select (click/Ctrl/Shift/keyboard), type-ahead, link/junction badges.
- Operations: new folder, rename, cut/copy/paste via a background job queue with progress,
  cancellation, conflict choice (keep both / skip / replace-merge) and per-item reports;
  Recycle Bin; open with default app (confirmation for executables); Windows properties;
  copy path; context menus; keyboard shortcuts; toasts; root error boundary.
- GitHub: README, LICENSE (MIT), CONTRIBUTING, CODE_OF_CONDUCT, SECURITY, CHANGELOG,
  issue/PR templates, CI + draft-release workflows (SHA-pinned actions, least privilege),
  Dependabot, architecture/development/security/release docs, roadmap.

## 2. Files

See `git status` / the repository tree. Main code: `src-tauri/src/**` (backend),
`src/**` (UI), `assets/branding/**`, `.github/**`, `docs/**`.

## 3. Architecture decisions

See [architecture overview](../architecture/overview.md). Key points: no generic fs/shell
plugins; 15 narrow IPC commands with app-manifest permissions; Win32 fail-if-exists APIs
for all writes; Recycle Bin through `IFileOperation` with nuke warning; links never
followed in recursion; single-worker job queue.

## 4. Test results

| Suite | Result |
|---|---|
| Rust unit tests (`cargo test`) | 37 passed, 0 failed (1 ignored by default) |
| Rust Recycle Bin test (`--ignored`, real Recycle Bin, temp file) | passed |
| Rust performance tests (`--ignored`) | passed — see [performance](../development/performance.md) |
| Frontend (Vitest, 21 tests: lib + App integration with fake backend) | 21 passed |
| `tsc --noEmit`, `cargo fmt --check`, `cargo clippy -D warnings` | clean |

Automated coverage includes: listing, Unicode names, hidden files, typed errors, junction
detection and non-following, long paths (>260 chars), create/rename without overwrite,
case-only rename, copy/move trees, skip/keep-both/replace-merge, copy-into-self refusal,
cancellation with processed list, queue ordering, name/path validation, executable
confirmation, Recycle Bin confirmation, navigation, selection, filter, theme persistence.

**Manually tested (real app, this machine):** startup, Home with real drives and known
folders, opening a synthetic test folder (Unicode, junction, hidden file hidden, file-type
icons), a 5,000-file folder, layout at 1240×780. **In the browser preview with a fake
backend:** light theme, icon view, Ctrl multi-select, context menu, status bar.

**Not tested manually yet:** copy/move/rename/recycle through the UI of the real app,
properties dialog, network shares, removable drives, high-DPI scaling, Windows 10.

## 5. Build results

`npx tauri build` succeeded (release, LTO):

| Artifact | Size | SHA-256 |
|---|---|---|
| `nexdir.exe` | 6.4 MB | `30bdf90d972941458141dc871fade10344fd35295df2f58a9416628bf3f1dc1f` |
| `Nexdir_0.1.0_x64-setup.exe` (NSIS, per-user) | 1.7 MB | `0e6643a35af4ba0a2a93566010e60f44b4d7446018f184727e3e8fb052326aaf` |
| `Nexdir_0.1.0_x64_en-US.msi` | 2.5 MB | `59d470ed28d10e00ef7dfd7b60fa21529de3c63df6eba9da242a81289769720b` |

Installers were built but **not installed or run**. Builds are unsigned.

## 6. Security findings

- `std::fs::rename`/`copy` overwrite silently → replaced by Win32 fail-if-exists calls.
- Recycle Bin is unavailable on network/removable drives (the shell would delete
  permanently) → Nexdir refuses on non-fixed drives.
- Bug found and fixed during manual testing: a deep path widened the layout beyond the
  window and hid columns and the filter box.

## 7. Known limitations

- Internal clipboard only (no Windows clipboard / drag-and-drop yet).
- No permanent delete, no undo, no tabs/split view/preview/thumbnails (Phase 2).
- Filter applies to the current folder only (recursive search: Phase 3).
- Recycle results are determined by existence checks; paths >260 chars cannot be recycled.
- UI preferences stored in WebView storage.
- Address-bar edit closes when the window loses focus.

## 8. Open tasks

- Replace `OWNER` in `.github/ISSUE_TEMPLATE/config.yml` with the GitHub account.
- Manual end-to-end pass of file operations in the real app.

## 9. Next steps

Phase 2: tabs, split view, drag-and-drop, Windows clipboard, preview pane, thumbnails,
inline rename, more view modes, saved layout, undo for reversible operations.
