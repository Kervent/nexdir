"""Console interface of the Nexdir Discord setup.

Read-only actions (check, analyze, preview, verify, export) run directly. Every action that
changes the server shows what will happen and asks for explicit confirmation; applying the
configuration requires typing the server's name. Changing actions refuse to run without an
interactive console.
"""

from __future__ import annotations

import argparse
import asyncio
import os
import sys
import time
from pathlib import Path

from .config import ConfigError, ServerSpec, load_config
from .executor import Executor, Result
from .export import export_snapshot
from .icon import IconError, circular_preview, find_icon, inspect_icon, upload_bytes
from .model import ApiError
from .permissions import invite_url, required_bot_permissions
from .planner import plan_changes
from .report import MANUAL_STEPS, plan_text, state_summary, write_report
from .settings import SettingsError, load_credentials, mask, read_env_file
from .texts import load_templates, plan_texts, publish
from .verify import verify

TOOL_DIR = Path(__file__).resolve().parent.parent
REPO_ROOT = TOOL_DIR.parent.parent
CONFIG = TOOL_DIR / "config" / "server.toml"
TEMPLATES = TOOL_DIR / "templates"
EXPORTS = TOOL_DIR / "exports"
REPORTS = TOOL_DIR / "reports"
ENV_FILE = TOOL_DIR / ".env"

MENU = """
NEXDIR DISCORD SETUP
────────────────────────────────
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
"""


def say(text: str = "") -> None:
    print(text, flush=True)


def interactive() -> bool:
    return sys.stdin.isatty()


def confirm(question: str) -> bool:
    if not interactive():
        say("Refusing to change the server without an interactive console.")
        return False
    return input(f"{question} [y/N] ").strip().lower() in ("y", "yes")


def load_spec() -> ServerSpec:
    return load_config(CONFIG)


async def connect():
    from .discord_api import DiscordGuildApi  # imported lazily: offline commands work without discord.py

    creds = load_credentials(ENV_FILE)
    try:
        return creds, await DiscordGuildApi.connect(creds.token, creds.guild_id)
    except ApiError as e:
        raise ApiError(mask(str(e), creds.token)) from None


# --- actions ------------------------------------------------------------------------


async def action_check(spec: ServerSpec) -> None:
    creds, api = await connect()
    try:
        state = await api.snapshot()
        say(f"Connected as {api.bot_name}.")
        say(state_summary(state))
        required = required_bot_permissions(spec)
        missing = required - state.bot_permissions if "administrator" not in state.bot_permissions else frozenset()
        if missing:
            say(f"\nThe bot is missing {len(missing)} permission(s): {', '.join(sorted(missing))}")
        else:
            say("\nThe bot has every permission the setup needs.")
        if "administrator" in state.bot_permissions:
            say("Note: the bot has Administrator. It does not need it; prefer the exact permissions below.")
        say("\nInvite / re-invite URL with exactly the needed permissions:")
        say("  " + invite_url(creds.client_id or str(state.bot_user_id), spec))
        say("Roles the bot must manage have to be BELOW the bot's own role (Server Settings → Roles).")
    finally:
        await api.close()


async def action_analyze(spec: ServerSpec) -> None:
    _, api = await connect()
    try:
        state = await api.snapshot()
    finally:
        await api.close()
    say(state_summary(state))
    configured_roles = {r.name.lower() for r in spec.roles}
    configured_channels = {ch.name for _, ch in spec.channels()} | {c.name.lower() for c in spec.categories}
    found_roles = [r.name for r in state.roles if r.name.lower() in configured_roles]
    other_roles = [r.name for r in state.roles if r.name.lower() not in configured_roles and not r.is_default]
    found_channels = [c.name for c in state.channels if c.name.lower() in configured_channels]
    other_channels = [c.name for c in state.channels if c.name.lower() not in configured_channels]
    say(f"\nConfigured roles already present ({len(found_roles)}): {', '.join(found_roles) or '—'}")
    say(f"Other roles (left untouched) ({len(other_roles)}): {', '.join(other_roles) or '—'}")
    say(f"Configured categories/channels present ({len(found_channels)}): {', '.join(found_channels) or '—'}")
    say(f"Other categories/channels (left untouched) ({len(other_channels)}): {', '.join(other_channels) or '—'}")


async def action_preview(spec: ServerSpec) -> None:
    _, api = await connect()
    try:
        state = await api.snapshot()
    finally:
        await api.close()
    plan = plan_changes(spec, state)
    say(plan_text(plan))
    path = write_report(REPORTS, "Dry run (no changes made)", state, plan, None)
    say(f"\nDry-run report saved to {path}")


async def action_apply(spec: ServerSpec) -> None:
    _, api = await connect()
    try:
        state = await api.snapshot()
        plan = plan_changes(spec, state)
        say(plan_text(plan))
        if plan.blockers:
            say("\nFix the blockers first. Nothing was changed.")
            return
        if plan.is_empty:
            say("\nThe server already matches the configuration. Nothing to do.")
            return
        export = export_snapshot(state, EXPORTS)
        say(f"\nCurrent configuration exported to {export} (not a complete backup).")
        if not interactive():
            say("Refusing to change the server without an interactive console.")
            return
        typed = input(f'\nType the server name "{state.name}" to apply {len(plan.creations)} creation(s): ').strip()
        if typed != state.name:
            say("Name did not match. Nothing was changed.")
            return
        allow_updates = bool(plan.updates) and confirm(f"Also apply the {len(plan.updates)} change(s) to EXISTING items listed above?")
        journal = REPORTS / f"apply-{time.strftime('%Y%m%d-%H%M%S')}.jsonl"

        def show(r: Result) -> None:
            mark = {"done": "✔", "failed": "✖", "skipped": "○"}[r.status]
            say(f"  {mark} {r.change.description}" + (f" — {r.message}" if r.message else ""))

        results = await Executor(api, state, journal=journal).run(plan, allow_updates=allow_updates, on_result=show)
        after = await api.snapshot()
        ok, problems = verify(spec, after)
        failed = [r for r in results if r.status == "failed"]
        say(f"\n{len([r for r in results if r.status == 'done'])} done, {len(failed)} failed, {len([r for r in results if r.status == 'skipped'])} skipped.")
        if failed:
            say("Failed changes can be retried by running 'Apply configuration' again; completed ones are not repeated.")
        extra = ["## Verification", ""] + [f"- ✔ {o}" for o in ok] + [f"- ✖ {p}" for p in problems]
        path = write_report(REPORTS, "Apply configuration", after, plan, results, extra)
        say(f"Report: {path}\nJournal: {journal}")
    finally:
        await api.close()


async def action_verify(spec: ServerSpec) -> None:
    _, api = await connect()
    try:
        state = await api.snapshot()
    finally:
        await api.close()
    ok, problems = verify(spec, state)
    for o in ok:
        say(f"  ✔ {o}")
    for p in problems:
        say(f"  ✖ {p}")
    path = write_report(REPORTS, "Verification", state, None, None, ["## Checks", ""] + [f"- ✔ {o}" for o in ok] + [f"- ✖ {p}" for p in problems])
    say(f"\nReport: {path}")
    say("Verified: the server matches the configuration." if not problems else f"\n{len(problems)} problem(s) found.")


async def action_export(_: ServerSpec) -> None:
    _, api = await connect()
    try:
        state = await api.snapshot()
    finally:
        await api.close()
    path = export_snapshot(state, EXPORTS)
    say(f"Exported to {path}\nNote: this is a readable snapshot, not a complete backup of the server.")


async def action_icon(spec: ServerSpec) -> None:
    path = find_icon(spec.icon, REPO_ROOT)
    if path is None:
        say("The official logo was not found. Generate it with 'npm run branding' in the repository and try again. This step can be repeated any time.")
        return
    try:
        info = inspect_icon(path)
    except IconError as e:
        say(str(e))
        return
    preview = circular_preview(path, REPORTS / "icon-preview.png")
    say(f"Logo: {info.path}\nSize: {info.width}×{info.height}, {info.file_bytes / 1024:.0f} KB")
    say(f"Preview of the circular crop: {preview}")
    if interactive() and confirm("Open the preview image now?") and hasattr(os, "startfile"):
        os.startfile(preview)  # type: ignore[attr-defined]  # Windows: opens in the default viewer
    if not confirm("Upload this logo as the Discord server icon?"):
        say("Server icon unchanged.")
        return
    _, api = await connect()
    try:
        await api.edit_server(icon=upload_bytes(path))
        say("Server icon updated.")
    except ApiError as e:
        say(f"Could not update the icon: {e}")
    finally:
        await api.close()


async def action_texts(spec: ServerSpec) -> None:
    templates = load_templates(TEMPLATES)
    _, api = await connect()
    try:
        state = await api.snapshot()
        plans = await plan_texts(spec, state, api, templates)
        for p in plans:
            say(f"\n══ #{p.channel} · {p.template} · {p.status} ══")
            if p.status in ("new", "outdated"):
                say("\n— next message —\n".join(p.chunks))
        todo = [p for p in plans if p.status in ("new", "outdated")]
        if not todo:
            say("\nAll info texts are published and up to date.")
            return
        say(f"\n{len(todo)} text(s) can be published: " + ", ".join(f"#{p.channel}" for p in todo))
        if not confirm("Publish these texts now? Mentions are disabled, nobody will be pinged."):
            say("Nothing was posted.")
            return
        for p in todo:
            try:
                say(f"  #{p.channel}: {await publish(p, api)}")
            except ApiError as e:
                say(f"  #{p.channel}: failed — {e}")
    finally:
        await api.close()


async def action_founder(spec: ServerSpec) -> None:
    _, api = await connect()
    try:
        state = await api.snapshot()
        founder = state.role("Founder")
        if founder is None:
            say("The Founder role does not exist yet. Apply the configuration first.")
            return
        say(f"The Founder role will be given to the server owner only (user id {state.owner_id}).")
        if not confirm("Assign the Founder role to the server owner?"):
            say("Nothing changed.")
            return
        await api.add_role_to_member(state.owner_id, founder.id)
        say("Founder role assigned to the server owner.")
    except ApiError as e:
        say(f"Could not assign the role: {e}")
    finally:
        await api.close()


def action_manual(_: ServerSpec) -> None:
    say("Manual steps (not possible or deliberately not automated via the Bot API):")
    for i, step in enumerate(MANUAL_STEPS, 1):
        say(f" {i}. {step}")


ACTIONS = {
    "check": action_check,
    "analyze": action_analyze,
    "preview": action_preview,
    "apply": action_apply,
    "verify": action_verify,
    "export": action_export,
    "icon": action_icon,
    "texts": action_texts,
    "founder": action_founder,
}
MENU_KEYS = {"1": "check", "2": "analyze", "3": "preview", "4": "apply", "5": "verify", "6": "export", "7": "icon", "8": "texts", "9": "founder", "m": "manual"}


def run_action(name: str, spec: ServerSpec) -> int:
    if name == "manual":
        action_manual(spec)
        return 0
    try:
        asyncio.run(ACTIONS[name](spec))
        return 0
    except (SettingsError, ApiError) as e:
        say(f"Error: {e}")
        return 1


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="nexdir-discord-setup", description="Configures the existing Nexdir Discord server. Never deletes anything.")
    parser.add_argument("command", nargs="?", choices=[*ACTIONS, "manual", "invite"], help="run one action instead of the menu")
    parser.add_argument("--client-id", help="application id, for 'invite'")
    args = parser.parse_args(argv)
    try:
        spec = load_spec()
    except ConfigError as e:
        say(str(e))
        return 2
    if args.command == "invite":
        client_id = args.client_id or os.environ.get("DISCORD_CLIENT_ID") or read_env_file(ENV_FILE).get("DISCORD_CLIENT_ID")
        if not client_id:
            say("Pass --client-id (the application id from the Discord Developer Portal).")
            return 2
        say(invite_url(client_id, spec))
        return 0
    if args.command:
        return run_action(args.command, spec)
    while True:
        say(MENU)
        choice = input("Choose: ").strip().lower() if interactive() else "0"
        if choice == "0":
            return 0
        if choice in MENU_KEYS:
            run_action(MENU_KEYS[choice], spec)
        else:
            say("Unknown choice.")
