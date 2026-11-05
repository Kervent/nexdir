# Roadmap

Priorities: data safety → correctness → reliability → stability → performance → usability.

## Phase 1 — Foundation ✅ (v0.1)
Project structure, Tauri/Rust/React stack, logo and icons, design system with themes,
navigation, drives, file lists, address bar, sorting, safe basic operations, error
handling, automated tests, CI.

## Phase 2 — Complete file management ✅ (v0.2)
Done: tabs (with session restore), split view, drag and drop inside Nexdir, Windows
clipboard integration (CF_HDROP), pinned folders (Ctrl+D), saved window size and position,
preview pane (images, text), undo for rename, transfer speed and remaining time, extended
keyboard shortcuts.

Carried over (planned for a later 0.x release):
- Drag and drop to and from Windows Explorer (needs native OLE drag and drop)
- Thumbnails in icon view (Windows shell thumbnail cache)
- PDF and media preview through safe renderers
- Inline rename, more view modes (list, small/large icons, tiles, compact)
- Reordering pinned folders
- Parallel operations; undo for same-volume moves

## Phase 3 — Intelligent features ✅ (v0.3)
Done: recursive search with filters, optional SQLite FTS5 index (user-chosen folders, content
indexing, update on start, delete), deterministic natural-language search, cancellable storage
analysis with treemap, BLAKE3 duplicate finder (never auto-deleting), organisation rules with
dry run, per-file approval and conflict policy.

Carried over:
- Similar-image detection (perceptual hashing)
- Watched folders that apply rules automatically (opt-in, with log)
- Incremental index updates (file system change journal) instead of full rebuilds
- ~~Hard-link awareness in the duplicate finder~~ — done in 0.4.3

## Phase 4 — Discord community setup ✅ (tool ready; live run pending owner approval)
Python/discord.py tool in `tools/discord-setup/` that configures the existing Nexdir
Discord server idempotently, with dry run, export and explicit confirmations. Tested against a
simulated server; it has not been run against the real server yet.

## Phase 5 — GitHub & community integration ✅ (announcements go live once webhooks are set up)
Done: GitHub → Discord announcements via channel webhooks (releases, selected issues and
merged PRs, milestones, roadmap) with duplicate protection; label set and sync workflow; issue
forms and PR template; contributor walkthrough, community guidelines, team roles, release
process, support page.

Carried over:
- Translation infrastructure (UI string catalogue) so translators can contribute
- Updating a Discord announcement when release notes are edited

## Phase 6 — Optimisation & release ✅ (v0.4, ready for a first public pre-release)
Done: benchmarks on 50,000-file synthetic trees and 100,000-entry lists, faster sorting and
filtering, security review with fixes, Windows compatibility tests (long paths, Unicode, case-only
rename), application manifest, About dialog, reproducible packaging script with checksums,
release checklist, code-signing preparation.

## 0.4.3 — UX, onboarding and support ✅
Done: first-start tour over the real interface, Help menu and Help & Support center with offline
help, in-app bug reports and feature requests (reviewed, cleaned, opened as prefilled GitHub
issue forms), backend enforcement of the duplicate finder's "keep one copy" rule, hard-link
awareness, installer install/update/uninstall test.

Carried over:
- A settings page (it would also offer *Restart Tutorial*; today the tour restarts from Help)
- Discord invite link in Help once the owner sets `NEXDIR_DISCORD_INVITE`

## 0.5.0 — Updates and licenses ✅
Done: signed in-app updates (Stable and Beta (Preview) channels), Open Source Licenses and
credits, settings dialog.

## 0.6.0 — Network shares and NAS (SMB) ✅ (alpha)
Done: Network area (network drives, connections, saved shares, a server's shares, search on
request), connection by path with the Windows account or Windows' own sign-in dialog, shares in
tabs and split view, verified copies to and from shares, bounded network calls, tested against a
Samba server.

Carried over:
- Tests with NAS devices (Synology, QNAP, TrueNAS) and on Windows 10
- Moving across shares and deleting on shares, once their safety can be proven
- Finding and removing `~nexdir-….partial` leftovers after an outage (with confirmation)
- Faster copies on high-latency links (several writes in flight)
- Assigning drive letters

## Next (after the first public release)
- Everything listed under "Carried over" above
- Code signing once a signing option is chosen
- Translation infrastructure
- UI frame-time measurements and tests on Windows 10 and high-DPI setups
