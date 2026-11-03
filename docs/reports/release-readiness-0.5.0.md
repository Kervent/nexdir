# Release readiness — Nexdir 0.5.0-alpha.1

Date: 2026-11-03 · Branch: `feature/0.5.0-updater` (local, not pushed) · Base: `main` @ `ee01c35`
(0.4.3). Tested on Windows 11 Pro 26H2 (build 26300), x64, 1 PC. Nothing was published; the
`v0.4.3` release and tag were not touched.

## Verdict: ALPHA READY

Updated after the owner approved the production key (same day). The code, the update system, the
license and credits views and the release pipeline are implemented and tested, including a real
update from one installed signed build to the next (test key) and a smoke test of the installed
release build signed with the **production key** (section 7). Signed release files exist in
`release-out/0.5.0-alpha.1/` (section 12).

Before publishing, still open (owner):

1. **GitHub secret `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`**: when it was set, 29 characters were
   entered, for the successful local signed build 30. If the Release workflow is to sign in CI,
   set the secret again by pasting the password from the password manager; otherwise the CI build
   stops at signing (it cannot publish anything wrong). The local signed files do not depend on
   the secret.
2. Back up `C:\Users\User\NexdirKeys\nexdir-update.key` and the password offline
   ([signing.md](../updates/signing.md)).
3. Recommended: test the MSI update path with the owner confirming the administrator prompts.
4. Push the branch, merge after CI, tag, publish the release; check that the *Update feed* workflow
   lists it (it has never run on GitHub yet).

## 1. Implemented

| Area | What |
|---|---|
| Updater | Official `tauri-plugin-updater` 2.14.1, used only from Rust (no updater permissions for the WebView). Feed fetched by Nexdir (HTTPS only, no redirects, 256 KB limit, 10 s connect / 20 s total timeout), strictly validated (`updater/feed.rs`), then handed to `Updater::restore_update`, which always requires the signature to name the announced version. Download with real progress events, cancellation, read timeout 30 s, disk-space check; installer type must match the installation; installation only after a second confirmation and when no copy/move or index build runs. |
| Channels | Stable / Beta (Preview); feeds `stable.json` / `preview.json` on branch `update-feed`; SemVer incl. pre-release order; no downgrades; empty Stable channel shown as "no release yet"; pre-release builds default to Preview. |
| Automatic checks | Off by default; stored in `update-settings.json` (backend decides); 8 s after start, at most once per 24 h (or week); no parallel checks; quiet on failure; notice with View Changes / Download Update / Remind Me Later. |
| Update window | 10 states (idle, checking, up to date, available, downloading, ready to install = download complete + verified, installing, restart via installer, error), versions, date, channel, size, signature status, release notes (rendered without HTML), progress and speed, clear error messages. |
| Install types | NSIS (passive, per user, restarts Nexdir), MSI (`msiexec /passive`), portable (announce + release page only). Result of the last install shown at the next start. |
| Settings | New Settings dialog (General, Updates) with privacy and security explanation. |
| About | Version, channel, installation type, build (commit + date), Check for Updates, Open Source Licenses (search, license and type filter, details, full text, project page, copy), Technologies & Credits. |
| Licenses | Generator extended: native SQLite and WebView2 loader with their texts, copyleft review (MPL-2.0), dev tools listed separately, in-app index from the same data, CI check for missing/unknown/outdated information. |
| Release | Release workflow requires the key, signs installers, verifies `.sig` files (`-RequireSignatures`), attaches them, uses `docs/release/notes-<v>.md`, still draft-only and never overwrites a release. New *Update feed* workflow. |
| Other | Close guard during copy/move; Escape closes dialogs also after focus loss (bug found in testing); MSI upgrade code pinned. |

## 2. Changed files

40 changed, 27 new paths (see `git status`). New: `src-tauri/src/updater/{mod,feed,settings,tests}.rs`,
`src-tauri/tests/updater_signatures.rs` + fixtures, `src-tauri/tauri.release.conf.json`,
`src/features/updates/*`, `src/features/about/*`, `src/features/settings/SettingsDialog.tsx`,
`src/generated/licenses.json`, `src/generated/license-texts.json`,
`tools/release/update_feed.py` + tests, `tools/release/licenses/`, `.github/workflows/update-feed.yml`,
`docs/updates/*`, `docs/licenses.md`, `docs/release/notes-0.5.0-alpha.1.md`, this report.
Changed: backend (`lib.rs`, `commands/mod.rs`, `security/mod.rs`, `operations/manager.rs`,
`tasks.rs`, `store.rs`, `platform/win32.rs`, `build.rs`, capabilities, `Cargo.toml`,
`tauri.conf.json`), frontend (`App.tsx`, `Dialog.tsx`, `dialogs.tsx`, `AboutDialog.tsx`, `api.ts`,
`links.ts`, `global.css`, `vite.config.ts`, `env.d.ts`), release tools, workflows, docs,
`THIRD-PARTY-NOTICES.txt`, versions.

## 3. Technologies

Tauri 2.12.5, Rust 1.99.1 (minimum now 1.90), React 19.3.1, TypeScript 7.0.2, Vite 8.3.5,
Vitest 5.0.5, SQLite 3.54.1, WebView2 loader 1.0.3800.47, tauri-plugin-updater 2.14.1
(minisign/Ed25519 via `minisign-verify`), reqwest 0.13.5 + rustls 0.23.46 (ring) with the
Windows certificate store (`rustls-platform-verifier`), Python 3 (release tools, stdlib only).

## 4. New dependencies

Direct: `tauri-plugin-updater` 2.14.1 (default features off: `rustls-tls`, `system-proxy`),
`reqwest` 0.13.5 and `rustls` 0.23.46 (same versions the plugin uses), `semver` 1.0.28; dev:
`tauri` feature `test`. Linked Rust crates: 186 → 225 (+39: the HTTP/TLS stack — hyper, tower,
rustls, ring, webpki, tokio-rustls … — plus `minisign-verify`, `tempfile`, `windows-registry`).
No new npm packages. `cargo audit`: 0 vulnerabilities (2 allowed warnings for Linux-only crates
that are not in the Windows binary, as in 0.4.3). `npm audit`: 0 vulnerabilities.

Size: Setup 2.5 → 3.3 MB, MSI 3.4 → 4.4 MB, portable ZIP 3.2 → 4.1 MB (mostly the TLS stack).

## 5. Licenses

234 bundled components (225 Rust crates, 7 JavaScript packages, 2 native: SQLite — public
domain; WebView2 loader — BSD-3-Clause-style Microsoft license, text taken from the NuGet package
of exactly that SDK version). Copyleft: MPL-2.0 in `cssparser`, `dtoa-short`, `option-ext`,
`selectors` (used unmodified, source on crates.io, documented). No GPL/LGPL/AGPL. New in the
notices: the WebView2 loader's license text (0.4.3 only linked to it) — the 0.4.3 release is
not changed; 0.5.0 ships the complete text. Details: [docs/licenses.md](../licenses.md).

## 6. Test results (all run on 2026-11-03)

| Suite | Result |
|---|---|
| `cargo fmt --check`, `cargo clippy -D warnings` (also with `--features updater-test`) | clean |
| Rust unit tests | 151 passed, 6 ignored (as before: real Recycle Bin / performance) |
| Rust `updater_signatures` (official updater, real signatures from a test key) | 10/10 |
| Rust performance tests | 2 ignored (run on demand) |
| Frontend (Vitest) | 150/150 (111 existing + 39 new: updates, settings, licenses, credits, release notes, Escape) |
| `third_party_notices.py --check` | up to date |
| Feed generator (`tools/release/tests`) | 11/11 (incl. RFC 8032 test vector) |
| Discord notifier incl. workflow safety checks (also for the new workflows) | 70/70 |
| Typecheck, `npm audit`, `cargo audit` | clean / 0 / 0 vulnerabilities |
| GUI regression (0.4.3 suite, 42 checks, on the new build) | **42/42** |
| End-to-end updater test on Windows (below) | **74 passed in the final runs; 2 failures caused by the test script, both re-run and passed** |

### Update cases covered

No update, newer preview, stable vs. preview (incl. empty stable), channel switch with
confirmation, no downgrade (unit, generator and E2E), SemVer pre-release order, malformed JSON,
wrong channel, invalid version, foreign/HTTP/path-trick URLs, missing/garbage signature, wrong
key, signature for another version, signature without version, tampered and truncated files,
HTTP 404/403/429/500/503, redirect refused, oversized answer, timeout, offline, dropped
connection during feed and download, parallel checks, no automatic install, automatic checks off
(0 requests), automatic check once per interval, settings persistence (backend file, survives
updates), portable mode, NSIS update, installer-type check. **Not covered:** MSI in-app update on
a real machine (unit-level only; see limitations), Windows 10, real GitHub feed (workflow not run
yet — needs the push).

## 7. Installation and upgrade tests (this PC, user data moved aside and restored)

Builds A = 0.5.0-alpha.1 and B = 0.5.0-alpha.2, both with the `updater-test` feature, signed with
a throwaway **test key** (private key only in the session's temp folder, never in the repository),
served by a local feed server on 127.0.0.1.

1. Installed the published **0.4.3** setup (SHA-256 checked against the release) silently, set
   settings (dark theme, icon view, sort by size, pinned folder) and a rules file.
2. **Manual upgrade 0.4.3 → A** with A's setup: settings, pinned folder and `rules.json`
   (same SHA-256) kept; detected as per-user installation; Preview channel; automatic checks off;
   **0 update requests in 12 s**.
3. In A: up to date (same version), 404/429/500/malformed explained, parallel checks (one refused
   as busy, the other timed out after 20 s; 1 request), Stable without release, Beta needs
   confirmation, **tampered installer rejected** ("signature is invalid … nothing was
   installed"), install refused during a 3 GiB copy, closing during the copy asks first,
   cancelled copy left no partial file.
4. **In-app update A → B**: throttled download with 12 distinct progress values and speed
   (≈1 MB/s), signature verified, second confirmation, Nexdir closed after 0.4 s, installer ran,
   **Nexdir restarted as 0.5.0-alpha.2 after 1 s**; settings, pinned folder, channel and
   `rules.json` kept; pending-install marker consumed; feed announcing the older alpha.1 →
   "up to date" (no downgrade).
5. Startup messages: "Nexdir was updated to …" and "The update to … was not installed … your
   current version keeps working" (simulated marker; the real path was step 4).
6. About in the installed app: version/channel/build; 234 licenses offline; search, MPL-2.0 filter
   (4), detail with full text and copyleft note; project link via `open_project_url` (intercepted);
   backend refuses links not in the index (fresh start, no interception).
7. Automatic checks: exactly 1 request ~8 s after start, notice shown, nothing downloaded;
   Remind Me Later hides it; next start within the interval: 0 requests.
8. Portable (unpacked exe): detected as portable, announces the update, no download button,
   backend refuses to download, opens the release page (intercepted).
9. Uninstalled silently; no files or uninstall entry left; the PC's previous Nexdir data restored.
10. **Release build with the production key** (`release-out/0.5.0-alpha.1`): installed the signed
    `Setup.exe` silently → version 0.5.0-alpha.1, per-user installation, *can install updates*
    (production public key built in), Preview channel, automatic checks off; a manual check
    reached GitHub over HTTPS in 364 ms and reported "HTTP 404" (correct: the `update-feed` branch
    does not exist before the first publication); an automatic check was skipped (off). The
    portable ZIP's exe carries no installer mark (detected as portable). Uninstalled, data
    restored. The GUI regression (42/42) ran on the build of the same code before the public key
    and one release-notes rendering fix (covered by unit tests) were added.

Test-script failures (not app defects, both re-run): a cancelled copy is shown as cancelled a
moment before its job has finished, so the test clicked *Install* too early and the backend
(correctly) still refused; and an interception wrapper from an earlier run caught one call that
was meant to reach the backend. Links were only clicked after the interception had been proven
on a harmless command; no browser window was opened.

## 8. Signature verification

Tauri CLI 2.12.4 writes `version:<v>` into the signed trusted comment (checked on both test
builds). Nexdir runs the updater with `requireSignedVersion`. Verified in three independent
places: the official updater (Rust integration tests, E2E), `package.ps1 -RequireSignatures`
(refuses unsigned builds — tested), and the feed generator's own Ed25519/minisign check (tested
against the Tauri CLI's signatures and the RFC 8032 vector). Update signatures, Authenticode and
SHA-256 are documented as separate mechanisms.

## 9. Security assessment

- Nexdir accepts only feeds from its fixed HTTPS address (test override only in builds with the
  `updater-test` feature, never in release builds — unit-tested), never follows redirects, limits
  sizes, and trusts nothing in the feed it can check itself.
- The installer must be signed by the built-in key for exactly the announced version, be of the
  installation's type, and be strictly newer. A compromised feed can at most withhold updates or
  offer one that then fails verification.
- The private key is the critical asset (theft = ability to push updates once a release is
  published). Mitigations: GitHub secret + offline backup, draft-only releases, manual publishing,
  feed only for published releases, rotation procedure. Recommended: a GitHub environment with
  required reviewer for the release workflow, and 2FA on the owner account.
- No update request without user action or opt-in; no personal data in requests (user agent
  `Nexdir/<version>`; GitHub sees the IP address — documented, no misleading promises).
- The WebView gets no updater plugin permissions; external links: repository/Discord
  (`open_url`) and license-index entries only (`open_project_url`).

## 10. Known problems

- MSI in-app update not tested on a real machine (needs administrator confirmation by the owner).
- A cancelled copy appears "cancelled" a moment before it has finished; *Install* says "still
  running" during that moment (safe, slightly confusing).
- The update feed is served by raw.githubusercontent.com, which may cache for a few minutes after
  a release is published.
- Installers grew by about 1 MB (TLS stack).

## 11. Limitations

- No Authenticode signature (SmartScreen warnings remain).
- 0.4.3 users must install 0.5.0 manually once.
- Portable copies are never updated automatically (by design).
- Windows 10 not tested; screen readers tested through ARIA roles/labels and unit tests, not with
  NVDA/Narrator; DPI/small-window layouts tested for the tour (GUI suite) and by CSS review for the
  new dialogs.

## 12./13. Artifacts and SHA-256

Built locally with `tools/release/build-signed.ps1` (production key `B64E9EF8C4E690D2`), both
update signatures verified by `package.ps1 -RequireSignatures`; in `release-out/0.5.0-alpha.1/`
(not uploaded):

```text
c7d593504a1a0afc2b2a4b378444711390c116fa4e11c7fcf5f3c1e4dabe3f65  Nexdir-0.5.0-alpha.1-Setup.exe
5a2d98029d069d9ec996b9f3f8c9dfce078d210eae02c107d2a794bfc2c32492  Nexdir-0.5.0-alpha.1-Setup.exe.sig
f386dffe9cc8c3d99c3d41623f8df034c1e13caaa5dc5e3bd25619881a1b5ad1  Nexdir-0.5.0-alpha.1.msi
3340cddf02ac4d6f577265a4443f53841b58191f4ed14f555206378b2f824606  Nexdir-0.5.0-alpha.1.msi.sig
2512e41fca7e2e7a466439efede353c83b9e8ab5e67c8d213a27e54094886a45  Nexdir-0.5.0-alpha.1-Portable.zip
```

Sizes: Setup 3.3 MB, MSI 4.4 MB, ZIP 4.1 MB. Built from the working tree of the commit that adds
the public key. If the release is built by the workflow instead, its files and checksums differ
(new timestamps and signatures).

## Next alpha

Version `0.5.0-alpha.1` (MSI version `0.5.0.1`) is set; release notes are in
`docs/release/notes-0.5.0-alpha.1.md`; the CHANGELOG entry is marked "unreleased". Remaining steps
are listed under the verdict.
