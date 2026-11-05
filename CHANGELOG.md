# Changelog

All notable changes to Nexdir are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
- **Network shares and NAS (SMB)**, in development: a Network area with the network drives and
  connections Windows already has, saved shares (visible offline), a server's share list, an
  on-demand network search with progress and Stop, manual connection by path with the signed-in
  Windows user or Windows' own sign-in dialog (Nexdir never handles passwords), status checks,
  and a context menu (open in new tab / other pane, check, sign in, save, copy path, properties,
  disconnect for Nexdir's own connections). Shares work in tabs and split view; copies to and
  from shares are verified and flushed. See `docs/features/network-storage.md`.

### Changed
- Moves that would copy and then delete across a network share (or merge into an existing folder
  on a share) are refused with an explanation; renames inside a share still work.
- Network paths are validated more strictly (server and share names, no `IPC$`).
- Listings, previews and address resolution of network paths are bounded in time; a NAS that does
  not answer no longer ties up background threads.
- Network errors are explained in plain English (sign-in required, not reachable, other account in
  use, blocked by Windows, …).
- Nexdir releases network connections it created itself when it closes.

## [0.5.0-alpha.1] — 2026-11-03 — updates, licenses and credits (alpha pre-release)

### Added
- **Update check and in-app updates** with the official Tauri updater: *Help → Check for
  Updates*, *About Nexdir* and *Settings → Updates*. The update window shows installed and
  available version, date, channel, size, signature status, release notes, real download
  progress and speed. Nothing is downloaded without *Download Update* and nothing installed
  without confirming *Close Nexdir and Install*.
- **Signed updates:** installers are verified against Nexdir's public update key; the signature
  must name the announced version (`requireSignedVersion`). Tampered files, foreign keys, older
  versions and the wrong installer type (MSI vs. per-user setup) are refused.
- **Release channels** *Stable* and *Beta (Preview)*, fed from `stable.json` / `preview.json`
  on the `update-feed` branch, generated from the published GitHub releases after verifying their
  signatures. Switching channels never installs an older version; the empty Stable channel says so.
- **Automatic checks** (off by default): after start, at most once per day or week, never in
  parallel, quiet when offline; a small notice with *View Changes*, *Download Update* and
  *Remind Me Later*.
- **Settings** dialog (gear button): appearance and updates.
- **About Nexdir** with *Open Source Licenses* (all bundled components, search, license and type
  filter, full texts offline, project link, copy) and *Technologies & Credits*; build
  information (commit and date).
- **Close guard:** closing Nexdir while copies or moves run asks first.
- Portable copies announce updates and open the download page; they never change themselves.

### Changed
- `THIRD-PARTY-NOTICES.txt` now also lists the native SQLite and WebView2 loader code with their
  license texts, documents copyleft (MPL-2.0) components and lists development tools as not
  distributed. The same generator writes the in-app license index; CI checks both.
- Privacy statements updated: the update check is the only connection Nexdir makes itself.
- Release workflow: requires the update signing key, signs both installers, verifies the
  signatures, attaches the `.sig` files and uses `docs/release/notes-<version>.md`.
- Minimum Rust version 1.90 (required by the updater plugin). MSI upgrade code pinned to the
  value 0.4.3 used.

## [0.4.3] — 2026-11-01 — UX, onboarding and support (alpha pre-release, first public release)

### Added
- **First-start tour:** on the first start Nexdir offers a short guided tour (*Start Tutorial* /
  *Skip for Now*). Five steps — navigation, tabs and split view, file operations, smart tools,
  personalization — highlight the real interface, followed by a closing step. Back/next, skip at
  any time, ←/→ and Esc, focus stays in the tour card, screen-reader announcements, no
  animation with reduced motion, highlights stay inside the window when it is resized and steps
  whose elements are hidden fall back to a centred card. The choice is stored locally with a
  tour version, so a future redesign can offer a new tour without touching settings. Restart it
  any time with *Help → Take a Tour*.
- **Help menu and Help & Support center** (question-mark button in the toolbar): take the tour,
  keyboard shortcuts, documentation, report a bug, request a feature, GitHub repository, Discord
  community and About. Offline help for navigation, file operations, search, duplicates, storage
  analysis, organize rules, keyboard shortcuts and common problems is bundled with the app.
- **Report a Bug / Request a Feature:** in-app forms that open the matching GitHub issue form
  prefilled. Nexdir version, Windows version and architecture are read locally and shown for
  review; each can be left out. Paths, the Windows user name, e-mail addresses and
  password/token-like values are replaced with placeholders, and the exact text is shown before
  anything leaves the PC. Long reports go through the clipboard. Nothing is sent by Nexdir
  itself — the issue only exists if the user submits it on GitHub.
- Backend command `open_url` that opens only the project's GitHub pages and Discord invite
  links, after a click; `system_info` for the report (no user names, paths or hardware IDs).

### Security / data safety
- **The duplicate finder's "keep at least one copy" rule is now enforced in the backend**
  (`recycle_duplicates`), not only in the interface. A group without a kept file is refused;
  one kept copy is opened with read-only sharing (nobody can change, rename or delete it during
  the operation) and its full content is hashed again; every file to remove must still be a
  regular file with the same size and BLAKE3 hash and must not be the kept file under another
  name (hard link, 8.3 name, case). Changed, missing or locked files are skipped and reported;
  removal requests run one at a time. The duplicates view no longer offers the generic *Move to
  Recycle Bin* context-menu entry. Details: `docs/security/duplicate-removal.md`.
- Hard links are no longer reported as duplicates (they take no extra space).

### Changed
- The toolbar's separate *Keyboard shortcuts* and *About* buttons moved into the Help menu
  (F1 still opens the shortcuts).
- Issue forms: the feature request form asks for problem, proposed solution, expected benefit
  and additional context; both forms carry ids that match the in-app forms.
- Release builds can include a Discord invite through the repository variable
  `NEXDIR_DISCORD_INVITE`.

### Release and repository
- `THIRD-PARTY-NOTICES.txt` lists every third-party component in the release (186 Rust crates,
  7 npm packages, the WebView2 loader) with its license text. It ships in the installers and the
  portable ZIP; `tools/release/third_party_notices.py` regenerates it and CI checks it is current.
- The Release workflow is started by hand for an existing tag and refuses tags that already have a
  release; 0.x versions are always marked as pre-releases.
- CI: new security job (`npm audit`, `cargo audit`, notice check) and dependency review for pull
  requests.
- README with screenshots of the application, installation and verification steps.

## [0.4.2] — race-condition fix (not yet released)

Replaces 0.4.1, which was built but never published.

### Fixed (data safety)
- **Race condition in the staged copy:** if another program took the planned temporary file
  name at the wrong moment, cleanup after a failed copy could delete that program's file. Temp
  files are now created exclusively (`CREATE_NEW`, no sharing) and every rename and delete —
  temp file, backup of a replaced file, source of a move — goes through the handle of the file
  Nexdir itself opened, never through a path. Settings files use the same scheme.
- Copies keep the "downloaded from the internet" mark (`Zone.Identifier`), timestamps and the
  read-only/hidden attributes of the source.

### Changed
- All project links point to https://github.com/Kervent/nexdir (issue templates, Discord setup
  configuration, security policy, About dialog, package metadata).
- Files are copied by Nexdir's own read/write loop instead of `CopyFileExW`, and existing files
  are swapped in with handle-based renames instead of `ReplaceFileW`. Replacing many small files
  got faster (1,000 files: 5.1 s → 2.8 s); copying many small files to new names is about 2×
  a plain copy.

## [0.4.1] — data-safety fix (not yet released)

Replaces 0.4.0, which was built but never published.

### Fixed (data safety)
- **Replacing a file could lose the existing file** when the copy was cancelled or failed
  (disk full, read error): Windows deleted the half-written destination. Copies now go to a
  temporary file, are verified, and are swapped in with `ReplaceFileW` and a backup that is
  restored if the swap fails. A move deletes its source only after the verified copy is in
  place. Replacing a file with itself under another name (8.3 name, hard link) and writing
  through a destination link or junction are refused. Details: `docs/security/safe-copy.md`.

### Changed
- Copies to new names are about 1.8× slower for many small files (temporary file and rename);
  replacing existing files additionally flushes and compares content (BLAKE3).

## [0.4.0] — Phase 6 (not yet released)

### Added
- *About Nexdir* dialog with the app version (toolbar info button).
- Windows application manifest: runs without administrator rights (`asInvoker`), declares
  Windows 10/11 support and long-path awareness.

### Changed
- Typing in the filter box no longer re-sorts the whole folder (~330 ms → ~4 ms per keystroke
  in a 100,000-item folder); sorting by type and date is faster.
- The file operations button uses an activity icon instead of a gear, which suggested settings.

### Security
- Path validation rejects every spelling of Windows device paths (e.g. `//./PhysicalDrive0`,
  lower-case `\\?\globalroot`), NTFS alternate data streams (`file.txt:stream`), wildcards and
  control characters in paths. See `docs/security/review-2026-10.md`.

### Release
- `tools/release/package.ps1` builds the release file set and `SHA256SUMS.txt`, locally and in
  the Release workflow; the workflow refuses tags that do not match the app version.
- Release smoke-test checklist, code-signing preparation, accurate data locations in the
  portable README.

### Fixed (tooling)
- `tools/discord-setup`: read-only and private channels failed with "Missing Permissions" on the
  real server because the bot gave itself *Manage Permissions* in their overwrites (only allowed
  for Administrators). The overwrite no longer includes it.

### Added (tooling, not part of the app)
- `tools/discord-setup`: setup program for the existing Nexdir Community Discord server —
  roles with hierarchy, categories, channels, forum tags, permission overwrites (private team
  area with per-channel access), info texts from versioned templates, official logo as server
  icon with circular preview, read-only analysis, dry run, configuration export, verification and
  Markdown reports. Never deletes, idempotent, confirmation before every change, least-privilege
  invite URL, token only from environment/.env. 40 tests against a simulated server; CI job.
- `tools/discord-notify` and the *Discord announcements* workflow: published releases go to
  #releases (version, date, main changes, download and release links), merged PRs and issues
  labelled `discord: announce` to #github-updates, closed milestones and roadmap posts to
  #roadmap. Channel webhooks only, no `@everyone`, optional opt-in role mentions, duplicate
  protection, pre-releases never presented as stable, manual runs are dry runs by default.
- *Sync labels* workflow and `.github/labels.json` (creates/updates labels, never deletes).
- Issue forms for documentation and pre-release feedback; "get help" link; extended pull
  request template.
- Docs: first-contribution walkthrough, community guidelines, team roles and application
  process, release process with recommended repository settings, `SUPPORT.md`, docs index,
  enforcement steps in the Code of Conduct.

### Changed (tooling)
- Release workflow passes the tag to its script through an environment variable instead of
  an inline expression.
- Dependabot also watches the Python tools.

## [0.3.0] — Phase 3 (not yet released)

### Added
- **Search** tool: plain-English requests ("videos larger than 2 GB in Downloads", "PDF files from
  last month", "*.log older than 30 days") parsed deterministically on the device, with every
  interpretation shown as a chip; filters for name/wildcards, extensions, categories, size,
  modification date, files/folders, subfolders and hidden files; results with folder column,
  context menu, open and reveal.
- Optional **search index** (SQLite FTS5) with user-chosen folders, optional content indexing of
  small text files, manual or on-start updates and one-click deletion; stored only locally.
- **Storage analysis** of drives and folders: totals, drive usage, interactive treemap with
  drill-down, largest folders and files, distribution by type and extension; cancellable,
  links never followed, unreadable folders counted instead of failing.
- **Duplicate finder**: size grouping → BLAKE3 of the first 64 KiB → full BLAKE3; reclaimable
  space; "select all but oldest/newest"; moving selected copies to the Recycle Bin only after
  confirmation and never all copies of a group.
- **Organize** rules (source, extensions, name, age, size → move/copy to a destination with a
  conflict policy) with dry-run preview, per-file approval and confirmation; runs through the
  regular operation queue with progress, cancellation and report.
- Tools section on Home and in the sidebar; "Search in this folder", "Analyze storage" and "Find
  duplicates" in context menus; Ctrl+Shift+F opens Search.
- Background task framework for read-only work (progress events, cancellation with partial
  results).

### Changed
- Inactive tabs stay loaded, so running scans and results survive tab switches.

## [0.2.0] — Phase 2 (not yet released)

### Added
- Tabs: new (Ctrl+T), close (Ctrl+W, middle click), switch (Ctrl+Tab, Ctrl+1…9), reorder by
  dragging; open folders in a background tab with the middle mouse button.
- Split view (Ctrl+\\) with two independent panes per tab, F6 to switch, "Open / Copy / Move
  to other pane" in the context menu.
- Drag and drop between folders, panes, sidebar entries and breadcrumbs (move on the same
  drive, copy across drives; Ctrl forces copy, Shift forces move).
- Windows clipboard integration: files copied or cut in Nexdir can be pasted in Explorer and
  vice versa; cut items are dimmed.
- Preview pane (Alt+P): images (incl. SVG, rendered safely as images), text files (UTF-8/UTF-16,
  first 128 KB), folder item counts and file metadata.
- Undo of the last rename (Ctrl+Z).
- Transfer speed and remaining time in the operations panel.
- Keyboard shortcut overview (F1), Ctrl+D to pin the current folder.
- Session restore (open tabs and split panes) and remembered window size/position.

### Changed
- File list columns adapt to the width of each pane instead of the window.
- Folders shown in any pane refresh after operations that affect them.

## [0.1.0] — Phase 1 (not yet released)

### Added
- Tauri 2 + Rust + React/TypeScript project foundation.
- Official Nexdir logo (SVG master) with generated PNG, ICO, Tauri, Discord and social-preview assets.
- Design system with light, dark and system themes.
- Home page with quick access folders, pinned folders and drives (usage, file system, availability).
- Folder browsing with details and icon views, natural sorting, filter, hidden-item toggle,
  multi-selection, type-ahead and keyboard navigation.
- Breadcrumb address bar with editable path, `%ENV%` expansion and file selection.
- New folder, rename, cut/copy/paste with background job queue, progress, cancellation,
  conflict handling (keep both / skip / replace-merge) and per-item reports.
- Move to Recycle Bin (fixed drives only), open with default app, Windows properties dialog, copy path.
- Confirmation before opening programs, scripts and shortcuts.
- Rust and Vitest test suites, GitHub Actions CI and draft-release workflow.
