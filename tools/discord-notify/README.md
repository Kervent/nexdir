# Nexdir Discord announcements

Posts GitHub events to the Nexdir Discord server. Runs in GitHub Actions
([`.github/workflows/discord-notify.yml`](../../.github/workflows/discord-notify.yml)) and uses
one Discord **webhook per channel** — no bot, no bot token, no server permissions.

This is the *permanent* integration. It is independent of the one-time
[`tools/discord-setup`](../discord-setup/README.md): you can remove the setup tool and its bot
after the server is configured, and announcements keep working.

## What gets announced

| GitHub event | Discord channel | Mention (optional) |
|---|---|---|
| Release **published** (stable version) | #releases | Release Notifications role |
| Release published as **pre-release** — only if `DISCORD_ANNOUNCE_PRERELEASES` is `true`; always labelled "not a stable version" | #releases | Beta Updates role (never Release Notifications) |
| Issue gets the label `discord: announce` (open issues only) | #github-updates | — |
| Pull request **merged** with the label `discord: announce` | #github-updates | — |
| Milestone **closed** | #roadmap | Development News role |
| Manual run → "roadmap" | #roadmap | — |
| Manual run → "release" + tag (e.g. to announce an older release) | #releases | as above |

A release announcement contains the version, release date, the main changes (top-level bullet
points of the release notes, up to 8), links to the installer/portable/MSI downloads and the
checksums, the release notes and the roadmap.

**Never announced:** drafts, tags that are not version numbers (`v1.2.3` or `v1.2.3-beta.1`),
releases whose notes contain `[skip discord]`, issues/PRs without the label, closed issues,
unmerged PRs. Only people with triage access or higher can add labels, so outside contributors
cannot trigger announcements.

## Safety

- **No `@everyone` or `@here`, ever.** Every message sets `allowed_mentions` so that only the
  configured role (if any) can be pinged; titles from GitHub are escaped so they cannot form
  mentions, links or Markdown.
- **No duplicates.** After posting, the workflow stores an empty artifact named after the
  announcement (e.g. `discord-announced-release-v1.2.0`). Every run checks for it first and does
  not post again. If the check fails, nothing is posted. Runs about the same item are queued, not
  run in parallel.
- **Pre-releases are never presented as stable.** A release counts as stable only if GitHub's
  pre-release box is unticked *and* the version has no `-suffix`.
- **Pull requests from forks** are announced via `pull_request_target`, but the job checks out
  only the base branch and reads only metadata (number, title, author) — code from a pull
  request is never run.
- The webhook URLs are secrets; the tool refuses any URL that is not a Discord webhook and never
  prints it. Workflow permissions: `contents: read`, `actions: read`.
- Standard library only — the workflow installs nothing.

## Setup (project owner)

1. **Create three webhooks in Discord** (needs *Manage Webhooks*):
   *#releases → Edit channel → Integrations → Webhooks → New Webhook*. Name it "Nexdir",
   upload `assets/branding/png/nexdir-logo-256.png` as its avatar, *Copy Webhook URL*.
   Repeat for #github-updates and #roadmap.
   Treat a webhook URL like a password: anyone who has it can post in that channel. If one
   leaks, delete the webhook in Discord and create a new one.
2. **Add them to GitHub** — *Settings → Environments → New environment* `discord`, then
   *Environment secrets*:
   - `DISCORD_WEBHOOK_RELEASES`
   - `DISCORD_WEBHOOK_GITHUB_UPDATES`
   - `DISCORD_WEBHOOK_ROADMAP`

   Optional: under *Deployment protection rules* add yourself as **required reviewer** — every
   announcement then waits for your click in the Actions tab before it is posted.
3. **Optional role mentions** — *Settings → Secrets and variables → Actions → Variables*:
   - `DISCORD_RELEASE_ROLE_ID` — ID of "Release Notifications"
   - `DISCORD_BETA_ROLE_ID` — ID of "Beta Updates"
   - `DISCORD_NEWS_ROLE_ID` — ID of "Development News"
   - `DISCORD_ANNOUNCE_PRERELEASES` — `true` to announce pre-releases

   Get an ID in Discord with *User Settings → Advanced → Developer Mode*, then right-click the
   role in *Server Settings → Roles → Copy Role ID*. Leave a variable unset for no mention.
4. **Create the labels**: *Actions → Sync labels → Run workflow* (creates `discord: announce`
   and the other labels from [`.github/labels.json`](../../.github/labels.json); deletes nothing).
5. **Test without posting**: *Actions → Discord announcements → Run workflow*, kind "roadmap",
   leave **dry_run** ticked. The log shows the exact message. Untick it to post for real.

A channel without a webhook secret is skipped quietly, so you can enable channels one by one.

## Day-to-day use

- **Release:** publish the draft release created by the Release workflow → announcement in
  #releases within a minute. See [docs/release/process.md](../../docs/release/process.md).
- **Highlight an issue or merged PR:** add the label `discord: announce`.
- **Milestone:** close it on GitHub.
- **Announce again on purpose:** delete the artifact `discord-announced-…` of the earlier run
  (Actions → run → Artifacts), then run the workflow manually.

## Local preview (offline)

```bash
cd tools/discord-notify
python -m nexdir_notify preview release path/to/event.json
```

Prints the message for a saved GitHub event payload. Nothing is sent.

## Tests

```bash
cd tools/discord-notify
python -m pip install -r requirements-dev.txt
python -m pytest -q
```

The tests use fake HTTP responses; they never contact Discord or GitHub. They also check the
workflow files (no expressions inside scripts, actions pinned to commit SHAs, explicit
permissions, pull request code never checked out).

## Known limitations

- The duplicate ledger uses workflow artifacts, which GitHub keeps for up to 90 days in public
  repositories. If a release is unpublished and published again more than 90 days later, it can
  be announced a second time.
- A manual "release" run and the automatic run for the same tag at the same moment are not
  queued against each other; the second may post too. Manual runs are dry runs by default.
- Editing release notes after publishing does not update the Discord message.
