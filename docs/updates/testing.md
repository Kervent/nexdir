# Testing the updater

## Automated tests

| Suite | Covers |
|---|---|
| `cargo test` → `updater::feed`, `updater::settings`, `updater::tests` | Feed validation (newer/same/older versions, SemVer pre-release order, stable vs. preview, malformed JSON, wrong channel, foreign or HTTP download addresses, path tricks, missing/malformed signatures, missing installer type), settings (off by default, interval, clock changes, snooze, channel switch), HTTP behaviour against a local server (404, 403/429, 5xx, redirects refused, oversized answers, dropped connection, timeout, offline, HTTPS required), one check at a time, installer type check |
| `cargo test --test updater_signatures` | The official updater with real signatures: valid, modified file, truncated file, other key, signature for another version, signature without version, garbage signature, older/equal release not offered, download with progress, interrupted download, corrupted download never returned |
| `python -m unittest discover -s tools/release/tests` | Feed generator: RFC 8032 test vector, Tauri signatures, SemVer order, drafts/unsigned/tampered/mismatching releases left out, stable vs. preview selection |
| `npm test` → `updates.test.tsx`, `licenses.test.tsx` | Automatic check only after the delay and only via the backend, no automatic download/install, quiet failures, last-install message, update window states, two-step install confirmation, portable mode, empty stable channel, notice, release notes never rendered as HTML, settings persistence through the backend, preview warning, privacy text |

The signature fixtures in `src-tauri/tests/fixtures/updater` were signed with a throwaway test
key. Only its public half is in the repository.

## Test builds with a test key

Release builds always use the official feed and the production public key. For an end-to-end
test on Windows:

1. Generate a test key outside the repository:
   `npx tauri signer generate --ci -p <password> -w <tmp>\test.key`.
2. Create a config overlay outside the repository, e.g. `<tmp>\tauri.test.conf.json`:
   `{"plugins":{"updater":{"pubkey":"<contents of test.key.pub>"}},"bundle":{"createUpdaterArtifacts":true}}`.
3. Build with the test feature and overlay, signing with the test key (environment variables
   `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`, in a separate
   `CARGO_TARGET_DIR`):
   `npx tauri build --features updater-test --config <tmp>\tauri.test.conf.json`.
4. Build a second, higher version the same way (a copy of the source with the version raised).
5. Serve a feed and the installers from a local folder (`python -m http.server --bind 127.0.0.1`)
   and start the first build with
   `NEXDIR_TEST_FEED_BASE=http://127.0.0.1:<port>/` and
   `NEXDIR_TEST_DOWNLOAD_BASE=http://127.0.0.1:<port>/download/`.

The `updater-test` feature is the only way to change the feed or download address, and only
through those variables; without the feature they are ignored. Never publish a test build.

Use a Windows user profile without important Nexdir settings, or back up
`%APPDATA%\org.nexdir.desktop` and `%LOCALAPPDATA%\org.nexdir.desktop` first; never use personal
files as test data.
