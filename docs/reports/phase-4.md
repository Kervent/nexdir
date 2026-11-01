# Phase 4 report — Discord community setup

Date: 2026-10-30 · Tool version 0.1.0 · Status: **tool complete and tested offline; not yet run against the real server**

## 1. Implemented (`tools/discord-setup/`)

| # | Function | Where |
|---|---|---|
| 1–3 | Check bot connection, server ID and bot permissions; invite URL with exactly the needed permissions | `cli.action_check`, `permissions.required_bot_permissions` |
| 4–6 | Detect existing roles, categories, channels (also unrelated ones, left untouched) | `cli.action_analyze`, `discord_api.snapshot` |
| 7 | Load and validate the desired configuration | `config.py`, `config/server.toml` |
| 8–9 | Compute changes; dry run with report | `planner.py`, `report.py` |
| 10 | Export the readable configuration (with "not a full backup" note), automatically before every apply | `export.py` |
| 11–15 | Create roles (with hierarchy), categories, channels, permission overwrites, forum tags | `executor.py` |
| 16 | Info texts from 8 versioned templates with preview, idempotent publishing/updating | `texts.py`, `templates/` |
| 17–18 | Find the official logo, validate, circular-crop preview, upload after confirmation | `icon.py` |
| 19 | Verification (structure + safety checks) | `verify.py` |
| 20 | Change reports (Markdown) and an apply journal (JSONL) | `report.py`, `executor.py` |
| — | Founder role → server owner only, after confirmation | `cli.action_founder` |

Console menu as specified (plus texts, Founder and manual steps); every action is also a command.

## 2. Files

`nexdir_discord/{config,permissions,model,planner,executor,discord_api,texts,icon,export,verify,report,settings,cli}.py`,
`config/server.toml`, `templates/*.md`, `tests/*`, `README.md`, `.env.example`,
`requirements*.txt`, `pyproject.toml`; CI job `discord-setup`; `docs/community/discord.md`.

## 3. Architecture decisions

- The core (config → plan → apply) is independent of discord.py and talks to Discord through a
  small `GuildApi` interface. Tests use an in-memory server; `discord_api.py` is the only
  discord.py code.
- REST only (no gateway): no privileged intents; messages always sent with mentions disabled.
- Never deletes. Missing items are created; differences on existing items are "updates" that
  need a separate confirmation. Overwrite updates merge (other members'/roles' overwrites stay).
- No role ever gets the Administrator flag, and the bot does not need it: a bot can only grant
  permissions it holds, so the required set is computed from the config.
- Forum channels require a Community server; without it they become text channels (or are
  skipped), with a warning. Onboarding is documented as a manual step so an existing Onboarding
  setup is never overwritten.
- Permission names are normalised (discord.py aliases) so re-runs don't report false drift.

## 4. Test results

40 automated tests, all passing (`pytest`, Python 3.14.7, discord.py 2.7.1):
config validation (incl. no Administrator, permission-free interest roles), access matrix of
private channels, read-only and forum permissions, bot permission union and permission bits
checked against discord.py, full setup on an empty simulated server and **idempotent second run**,
**dry run makes no changes**, existing items reused/never deleted/updates gated, overwrite
merge, Community fallback, missing bot permissions → blocker, roles above the bot untouched, API
errors recorded with retry and resume, failed category → no private channel outside it, export
content, verification catches weakened private channels and admin roles, text chunking/markers/
idempotent publishing/in-place update, logo detection/validation/circular preview/upload,
credential loading and token masking, `.env` ignored by Git, CLI safety (no console → no change,
wrong server name → no change, updates declined → existing items untouched, Founder only to the
owner).

**Manually tested (offline):** `invite` URL, `manual`, missing-credentials error, simulated dry
run (46 creations, 1 update, 0 changes made).

**Live run on the real server (2026-10-31, with the owner's approval):**
- Check connection: bot connected, all permissions present, Community already enabled.
- Analyze/preview: 3 existing roles (Dyno, Ticket Tool, Nexdir Setup) and #moderator-only left
  untouched; #rules recognised as existing; 65 creations + 1 update planned.
- First apply: 52 done, **14 failed** with "Missing Permissions" — every read-only and private
  channel. Cause: the bot's own channel overwrite allowed *Manage Permissions*, which Discord
  only accepts from Administrators. The simulated server did not model this rule. Fixed in
  `permissions.BOT_ACCESS`, regression test added (41 tests).
- Retry: the 14 items (incl. the #rules update) done, 0 failed; completed items were not repeated.
- Verify: all roles, categories, channels and tags match; no Administrator role; private
  channels hidden, read-only channels block posting. Only open point: no server icon yet.
- A second dry run afterwards: 0 changes (idempotent on the real server).
- Server icon: circular preview checked, official logo uploaded after confirmation; Verify then
  reported no problems.
- Founder role assigned to the server owner (after confirmation).
- Info texts: **not posted yet** — postponed by the owner until the GitHub repository exists,
  because 7 of 8 templates link to `github.com/OWNER/nexdir` (placeholder).

## 5. Build

Pure Python tool, isolated virtual environment in `tools/discord-setup/.venv` (ignored by Git).
Pinned dependencies: discord.py 2.7.1, Pillow 13.0.0 (tests: pytest 9.2.0, pytest-asyncio 1.4.1).

## 6. Security findings

- Creating the Moderator role requires the bot to hold kick/ban/timeout itself → documented;
  recommend removing the bot after the setup.
- Private channels must keep an overwrite for the bot, otherwise it locks itself out → added.
- In read-only private channels (mod-logs), viewers must not inherit posting → fixed in the
  overwrite rules and tested.

## 7. Known limitations

- Not yet executed against the real server.
- Channels are matched by name; a configured channel name used by an unrelated channel is treated
  as the same channel (reported as an update, never changed without permission).
- Forums created as text channels before Community is enabled are not converted automatically.
- Onboarding, safety settings and Community settings are manual steps (listed in the tool).

## 8. Next steps (need your action)

1. Create the bot application, put token/IDs in `tools/discord-setup/.env`, invite the bot with
   the printed URL, move its role to the top.
2. Run **Check connection** and **Preview changes**, and share the dry-run report if you want me
   to review it.
3. On your explicit approval: **Apply configuration**, **Verify**, icon, texts, Founder role.

Then Phase 5: GitHub ↔ Discord release notifications and community documentation.
