# Release process

Releases are made by the project owner. Nothing is published automatically: the Release
workflow builds a **draft**, and the owner reviews and publishes it by hand.

## Versions

Nexdir uses [Semantic Versioning](https://semver.org/). While the version is `0.x`, minor
versions may contain breaking changes.

| Kind | Tag | GitHub | Discord |
|---|---|---|---|
| Stable | `v0.4.0` | normal release | announced in #releases (+ Release Notifications role, if configured) |
| Pre-release | `v0.4.0-beta.1`, `v0.4.0-rc.1` | tick *Set as a pre-release* | only if `DISCORD_ANNOUNCE_PRERELEASES` is `true`; marked "not a stable version"; Beta Updates role |
| Test build | — | don't create a release; use the CI artifact of a workflow run | never |

A tag with a `-suffix` is always treated as a pre-release, even if the GitHub checkbox was
forgotten. Write `[skip discord]` anywhere in the release notes to publish without an
announcement.

## Steps

1. **Prepare** on a branch, through a normal pull request:
   - set the version in `package.json` (`npm version 0.4.0 --no-git-tag-version`, which also
     updates `package-lock.json`), `src-tauri/Cargo.toml` and `src-tauri/tauri.conf.json`;
   - run `cargo check --manifest-path src-tauri/Cargo.toml` to update `Cargo.lock`;
   - move the `## [Unreleased]` entries in `CHANGELOG.md` to `## [0.4.0] — YYYY-MM-DD`;
   - make sure CI is green, then merge.
2. **Tag** the merge commit on `main` and push the tag (owner only):
   ```bash
   git tag -a v0.4.0 -m "Nexdir 0.4.0"
   git push origin v0.4.0
   ```
   Pushing a tag does not start a build.
3. **Build the release files**, either way:
   - **In GitHub Actions:** *Actions → Release → Run workflow*, enter the tag. The workflow
     checks that the tag matches the version in `tauri.conf.json`, runs the tests, builds and
     creates a **draft release** with `Nexdir-<version>-Setup.exe`, `Nexdir-<version>-Portable.zip`,
     `Nexdir-<version>.msi` and `SHA256SUMS.txt`, collected by
     [`tools/release/package.ps1`](../../tools/release/package.ps1). All 0.x versions and tags with
     `-` are marked as pre-releases. It refuses a tag that already has a release, so files that
     were uploaded by hand are never replaced.
   - **Locally** (as for 0.4.3, where the tested files were uploaded):
     ```bash
     npm ci
     npm run app:build
     powershell -ExecutionPolicy Bypass -File tools/release/package.ps1 -Clean
     gh release create v0.4.0 --draft --prerelease --notes-file <notes.md> release-out/*
     ```
     Output: `release-out/` (ignored by Git). The script refuses stale bundles from an older
     version and an exe that is newer than the installers.

   Before either: `python tools/release/third_party_notices.py` regenerates
   `THIRD-PARTY-NOTICES.txt` when dependencies changed (CI fails if it is out of date); the file
   ships in the installers and the portable ZIP.
4. **Review the draft:**
   - download the installer and the portable ZIP and check them against `SHA256SUMS.txt`
     (`Get-FileHash <file> -Algorithm SHA256`);
   - install on a clean Windows test machine or VM and smoke-test with the
     [release checklist](checklist.md);
   - paste the changelog section as release notes. Top-level bullet points become the
     "What's new" list on Discord (up to 8), so put the most important ones first.
5. **Publish** the draft. The *Discord announcements* workflow posts to #releases within a
   minute. Check the run log if nothing appears; a missing webhook secret is logged, not an error.
6. **After the release:** close the milestone (announced in #roadmap), update
   [ROADMAP.md](../ROADMAP.md) if plans changed.

Releases are not code-signed yet, so Windows SmartScreen may warn. See
[code-signing.md](code-signing.md).

### If something goes wrong

- **Broken build in the draft:** delete the draft and the tag, fix, tag again. Drafts are never
  announced.
- **Published by mistake:** edit the release and mark it as pre-release or draft. The Discord
  message, if already posted, must be removed by hand in Discord.
- **Announcement missing:** run *Actions → Discord announcements → Run workflow*, kind
  "release", the tag, untick *dry_run*. Already announced releases are not posted twice.

## Repository settings

Recommended settings for the owner when the repository is published (none of them are applied
automatically):

- **Branches → `main`:** require a pull request with 1 approval, require all five CI jobs as
  status checks (Frontend, Backend, Discord setup tool, Discord announcements, Windows build),
  dismiss stale approvals, block force pushes and deletion.
- **Tags:** a ruleset that allows only the owner to create `v*` tags.
- **Actions → General:** workflow permissions *Read repository contents* (default token
  read-only); *Require approval for all outside collaborators* for fork pull requests.
- **Code security:** enable private vulnerability reporting, Dependabot alerts and secret scanning
  with push protection.
- **Environments → `discord`:** holds the webhook secrets; optionally add required reviewers
  (see [tools/discord-notify](../../tools/discord-notify/README.md)).
- **Labels:** run *Actions → Sync labels* once.

## Before the first public release

- ~~Replace the placeholder `OWNER`~~ — done: all links point to
  `https://github.com/Kervent/nexdir`.
- Add the Discord invite link to `README.md` and `SUPPORT.md` (still open), and set the
  repository variable `NEXDIR_DISCORD_INVITE` (*Settings → Secrets and variables → Actions →
  Variables*) so release builds show it in *Help & Support*. Local builds read the same value
  from the environment variable `VITE_NEXDIR_DISCORD_INVITE`. Without it, the Discord entry is
  shown as unavailable; only `https://discord.gg/…` and `https://discord.com/invite/…` links are
  accepted.
- Upload `assets/branding/png/nexdir-social-preview.png` under *Settings → Social preview*.
