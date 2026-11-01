# Nexdir Discord setup

Configures the **existing** Nexdir Community Discord server: roles, categories, channels,
permissions, forum tags, info texts and the server icon (the official Nexdir logo).

It is a one-time setup tool. It is not part of the Nexdir app, and you can delete it after the
setup without losing anything on the server.

## Safety guarantees

- **Never deletes anything.** Extra roles, channels and messages on the server are left alone.
- **Dry run first.** "Preview changes" shows exactly what would happen and changes nothing.
- **Explicit confirmation.** Applying the configuration requires typing the server's name.
  Changes to items that already exist (e.g. a channel with the same name but other
  permissions) are listed separately and need a second, separate "yes".
  Icon upload, publishing texts and assigning the Founder role each ask again.
- **Idempotent.** Running it twice creates no duplicates; an interrupted run can simply be
  repeated, and only the missing parts are created.
- **Least privilege.** The bot never needs or grants *Administrator*. "Check connection" prints
  an invite URL with exactly the permissions required.
- **No pings.** Messages are sent with all mentions disabled; `@everyone` is never used.
- **Token safety.** The token is read from an environment variable or the untracked `.env`
  file and never printed. Before every apply, a readable export of the current configuration is
  written to `exports/` (this is *not* a complete backup).
- **Official API only.** Bot token + discord.py, REST only, no self-bots, no user tokens, no
  undocumented endpoints, no privileged intents.

## Setup by the project owner

1. **Open the Discord Developer Portal:** <https://discord.com/developers/applications>
2. **Create an application:** *New Application* → name it e.g. "Nexdir Setup".
3. **Configure the bot:** *Bot* → *Reset Token* → copy the token (shown once). Leave all
   *Privileged Gateway Intents* **off** — they are not needed. Turn *Public Bot* off.
4. **Install the tool** (Windows, Python 3.11+):
   ```powershell
   cd tools\discord-setup
   python -m venv .venv
   .venv\Scripts\python -m pip install -r requirements.txt
   copy .env.example .env
   ```
5. **Configure credentials:** edit `.env` and set `DISCORD_BOT_TOKEN`, `DISCORD_GUILD_ID` and
   `DISCORD_CLIENT_ID` (Application ID from *General Information*).
   Server ID: Discord → *User Settings → Advanced → Developer Mode* on, then right-click the
   server icon → *Copy Server ID*.
6. **Choose the permissions / get the invite URL:**
   ```powershell
   .venv\Scripts\python -m nexdir_discord invite
   ```
   The URL contains exactly the permissions the setup needs (the bot can only create roles
   with permissions it has itself, which is why it needs e.g. *Kick Members* to create the
   Moderator role).
7. **Add the bot to the existing server:** open the URL, choose the Nexdir server, *Authorize*.
   Then in *Server Settings → Roles*, drag the bot's role to the **top** (it can only manage
   roles below its own).
8. **Test the connection:** run the tool and choose **[1] Check connection**.
   ```powershell
   .venv\Scripts\python -m nexdir_discord
   ```
9. **Dry run:** **[2] Analyze** and **[3] Preview changes**. A report is written to `reports/`.
10. **Apply:** **[4] Apply configuration** → type the server name. Then **[5] Verify**, **[7]
    Update server icon** (preview first), **[8] Info texts** (preview first), **[9] Founder role**.
    Finally work through **[m] Manual steps**.

After the setup, remove the bot from the server (or remove its role's permissions).

## Menu

```
NEXDIR DISCORD SETUP
 [1] Check connection and bot permissions
 [2] Analyze existing server
 [3] Preview changes (dry run)
 [4] Apply configuration
 [5] Verify setup
 [6] Export server configuration
 [7] Update server icon
 [8] Preview and publish info texts
 [9] Assign Founder role to the server owner
 [m] Manual steps checklist
 [0] Exit
```

Every action is also available as a command, e.g. `python -m nexdir_discord preview`.

## What the API cannot (or should not) do — manual steps

- **Enable Community** (needed for forum channels, server description, Onboarding). Without it,
  forums are created as text channels; after enabling Community, rename or remove those text
  channels yourself and run the setup again.
- **Onboarding / Channels & Roles:** the interest roles (`Interested: …`) and notification
  roles are meant to be self-selected there. The Bot API can edit Onboarding, but the tool
  deliberately does not, so it never overwrites an existing Onboarding configuration.
- Safety settings (verification level, content filter, AutoMod), default notifications, rules
  and updates channels in Community settings.
- Assigning team roles to people (after an accepted application) — never automatic.

## Configuration

- `config/server.toml` — roles (order = hierarchy), categories, channels, permissions, tags.
- `templates/*.md` — versioned info texts. Placeholders like `{links.github}` are filled from
  `[links]` in the config. Published texts carry a marker so later runs update them instead of
  posting duplicates.

## Development

```powershell
.venv\Scripts\python -m pip install -r requirements-dev.txt
.venv\Scripts\python -m pytest
```

Tests run against an in-memory simulated server (`tests/fake.py`) and never contact Discord.
