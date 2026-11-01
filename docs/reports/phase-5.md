# Phase 5 report — GitHub and community integration

Date: 2026-10-31 · Status: **implemented and tested offline; not yet run on GitHub or against
Discord** (no repository has been published and no webhook exists yet)

## 1. Implemented

| Requirement | Result | Where |
|---|---|---|
| Release notifications for Discord | Published release → #releases with version, date, main changes, installer/portable/MSI links, checksum note, release and roadmap links; optional "Release Notifications" mention | `tools/discord-notify/`, `.github/workflows/discord-notify.yml` |
| Selected issues / important PRs | Label `discord: announce` (maintainers only) → #github-updates; PRs only once merged | same |
| Milestones and roadmap links | Closed milestone → #roadmap (optional "Development News" mention); manual roadmap post | same |
| No duplicate announcements | Ledger of workflow artifacts, checked before every post; fails closed; runs per item are queued | `net.GitHub.already_announced`, workflow `concurrency` |
| No test releases as stable | Stable only if GitHub's pre-release box is unticked **and** the version has no `-suffix`; pre-releases opt-in, labelled, Beta Updates role only; drafts, non-version tags and `[skip discord]` never announced | `events.release` |
| Never `@everyone` | `allowed_mentions` restricts pings to the one configured role; GitHub titles are escaped | `events._payload`, `text.plain` |
| One-time setup vs permanent integration | Separate tool with webhooks (no bot); the setup tool and its bot can be removed | `tools/discord-notify/README.md` |
| GitHub issue templates | + Documentation, + Pre-release feedback, "get help" contact link; existing bug report updated | `.github/ISSUE_TEMPLATE/` |
| Pull request template | Type of change, tool tests, secrets and changelog checks | `.github/PULL_REQUEST_TEMPLATE.md` |
| GitHub Actions | Discord announcements, Sync labels (manual, never deletes), CI job for the notifier; release workflow hardened | `.github/workflows/` |
| Labels | 23 labels incl. `data safety`, `good first issue`, area labels, `discord: announce` | `.github/labels.json` |
| Contributor documentation | First-contribution walkthrough, CONTRIBUTING extended (ways to help, workflow changes, triage, releases) | `docs/development/first-contribution.md`, `CONTRIBUTING.md` |
| Community participation guidelines | Where things happen, ways to help (6 areas), good practice, privacy, AI-assisted contributions | `docs/community/guidelines.md` |
| Team role documentation | Project role vs GitHub access vs Discord permissions, responsibilities, application and review process | `docs/community/team-roles.md` |
| Open-source documentation | `SUPPORT.md`, docs index, release process with recommended repository settings, Code of Conduct enforcement steps | `SUPPORT.md`, `docs/README.md`, `docs/release/process.md` |

## 2. Architecture decisions

- **Webhooks instead of a bot.** A webhook can only post to its own channel, needs no gateway
  connection and no server permissions. One per channel, stored as secrets of a `discord`
  environment so the owner can require approval for every post.
- **Standard library only.** The workflow installs nothing at runtime: no third-party Python
  packages and no extra actions besides checkout/upload-artifact (both pinned to SHAs).
- **Pure core.** `events.py` turns an event payload into a message or a skip reason without
  I/O; HTTP is injected, so all behaviour is tested with fake responses.
- **Ledger in workflow artifacts.** Needs only `actions: read` and no write access to the
  repository; an unreadable ledger blocks posting.
- **`pull_request_target`** so merged fork PRs can be announced; the job checks out only the
  base branch and never runs PR code. A test enforces this.
- **Webhook URL allow-list**: anything that is not a `discord.com/api/webhooks/…` URL is
  refused, and the URL never appears in logs or error messages.

## 3. Test results

**Automated (70 tests, all passing, Python 3.14.7, pytest 9.2.0)** in `tools/discord-notify`:
text escaping (Markdown, mentions, bidi/control characters), truncation, version parsing,
release highlights; stable release content, role mention opt-in, release without assets,
pre-release rules (skipped by default, never stable, beta role only), drafts/non-version
tags/skip marker, only `published` announces, Discord length limits; issues (label, closed, PR,
hostile title), PRs (merged+label, label after merge, unmerged, other label), milestones,
roadmap; CLI: posts once and records the ledger, second run does not post, ledger error fails
closed, no token → no post, missing webhook → quiet skip, manual runs dry by default, manual
release/roadmap, rate-limit retry, Discord error without URL, URL allow-list, role ID
validation, pre-release opt-in, offline preview. **Workflow guards:** no `${{ }}` in any `run:`
script across all workflows (this found and fixed one case in `release.yml`), every workflow
declares permissions, all actions pinned to commit SHAs, the notifier never checks out PR code
and has no write permission, every webhook secret is passed, labels used by templates exist.

Also run: `tools/discord-setup` tests (40 passed, after updating its manual-steps text) and the
frontend tests (62 passed).

**Manually tested (offline):** `python -m nexdir_notify run --dry-run` with a sample release
event (correct message, `@everyone` in the notes neutralised, no mentions) and without
`--dry-run` and no webhook (quiet skip, exit code 0); all YAML files under `.github/` parsed
with PyYAML 6.0.3 (in a throwaway environment).

**Not tested:**
- The workflows on GitHub (the repository is not published) — including the artifact ledger
  against the real API, environment secrets and the Sync labels job.
- Posting to Discord (no webhook exists yet).
- Python 3.11 (the CI job uses 3.11 as the oldest supported version; locally only 3.14 was
  available).

## 4. Security notes

- Only maintainers can add labels, so outside contributors cannot trigger announcements; issue
  and PR titles are escaped so they cannot ping, link or format.
- A leaked webhook URL lets anyone post in that one channel; documented: delete and recreate it.
- Recommended repository settings (branch protection, read-only default token, fork-PR approval,
  tag ruleset, secret scanning) are documented but cannot be applied before the repository exists.

## 5. Known limitations

- The ledger relies on artifacts (kept up to 90 days in public repositories); re-publishing a
  release after that can announce it again.
- A manual "release" run and the automatic run for the same tag at the same moment are not
  queued against each other (manual runs are dry runs by default).
- Edited release notes do not update the Discord message.
- Translation: the app has no string catalogue yet; the guidelines say so honestly.
- The `OWNER` placeholder remains in `.github/ISSUE_TEMPLATE/config.yml` and
  `tools/discord-setup/config/server.toml`; the Discord invite link is not in README/SUPPORT yet.

## 6. Next steps (need your action)

1. Decide the GitHub account/organisation → replace `OWNER`; give me the Discord invite link
   if it should appear in the README.
2. When you approve publishing: create the repository, push, apply the repository settings from
   `docs/release/process.md`, run *Sync labels*.
3. Create the three channel webhooks and the `discord` environment secrets, then test with a
   manual dry run.

Then Phase 6: optimisation, security review, installer/portable builds, checksums, code-signing
preparation.
