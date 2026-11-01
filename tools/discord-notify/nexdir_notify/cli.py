"""Command line.

    python -m nexdir_notify run [--dry-run]           inside GitHub Actions
    python -m nexdir_notify preview EVENT_NAME FILE   offline preview of a saved event payload

``run`` reads the event from GITHUB_EVENT_NAME/GITHUB_EVENT_PATH and the configuration from
environment variables (see README). It posts at most one message per run.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
from datetime import datetime, timezone
from pathlib import Path

from . import events
from .model import CHANNELS, Announcement, Settings, Skip
from .net import GitHub, NotifyError, Webhook, ledger_name

WEBHOOK_ENV = {channel: "DISCORD_WEBHOOK_" + channel.upper().replace("-", "_") for channel in CHANNELS}
ROLE_ENV = {
    "release_role_id": "DISCORD_RELEASE_ROLE_ID",
    "beta_role_id": "DISCORD_BETA_ROLE_ID",
    "news_role_id": "DISCORD_NEWS_ROLE_ID",
}
_SNOWFLAKE = re.compile(r"^\d{15,25}$")


def settings_from_env(env=os.environ) -> Settings:
    roles = {}
    for field, name in ROLE_ENV.items():
        value = (env.get(name) or "").strip()
        if value and not _SNOWFLAKE.match(value):
            raise NotifyError(f"{name} must be a Discord role ID (a long number), not a name")
        roles[field] = value or None
    return Settings(
        repository=env.get("GITHUB_REPOSITORY", "Kervent/nexdir"),
        server_url=env.get("GITHUB_SERVER_URL", "https://github.com"),
        webhooks={ch: env[var].strip() for ch, var in WEBHOOK_ENV.items() if (env.get(var) or "").strip()},
        announce_prereleases=(env.get("DISCORD_ANNOUNCE_PRERELEASES") or "").strip().lower() == "true",
        **roles,
    )


def plan(event_name: str, event: dict, settings: Settings, github: GitHub | None, today: str) -> Announcement | Skip:
    if event_name != "workflow_dispatch":
        return events.build(event_name, event, settings)
    inputs = event.get("inputs") or {}
    kind = (inputs.get("kind") or "").strip()
    if kind == "release":
        tag = (inputs.get("tag") or "").strip()
        if not tag:
            return Skip("enter the release tag, e.g. v1.2.0")
        if github is None:
            return Skip("a release can only be looked up with GITHUB_TOKEN")
        rel = github.release_by_tag(tag)
        if rel is None:
            return Skip(f"no published release with tag '{tag}' (drafts are never announced)")
        return events.release(rel, settings)
    if kind == "roadmap":
        return events.roadmap(settings, note=inputs.get("note") or "", key=f"roadmap-{today}")
    return Skip(f"unknown kind '{kind}' (use 'release' or 'roadmap')")


def run(env, out=print, *, dry_run: bool, transport=None, sleep=None) -> int:
    settings = settings_from_env(env)
    event_name = env.get("GITHUB_EVENT_NAME", "")
    event = json.loads(Path(env["GITHUB_EVENT_PATH"]).read_text(encoding="utf-8"))
    if event_name == "workflow_dispatch":
        # Manual runs are a dry run unless the maintainer turned it off.
        dry_run = dry_run or str((event.get("inputs") or {}).get("dry_run", "true")).lower() != "false"

    net = {"transport": transport} if transport else {}
    token = env.get("GITHUB_TOKEN") or None
    github = GitHub(token, settings.repository, env.get("GITHUB_API_URL", "https://api.github.com"), **net) if token else None
    result = plan(event_name, event, settings, github, datetime.now(timezone.utc).date().isoformat())

    if isinstance(result, Skip):
        out(f"Nothing to announce: {result.reason}.")
        _outputs(env, posted="false")
        return 0

    out(f"Announcement: {result.summary} (key {result.key})")
    url = settings.webhooks.get(result.channel)
    if not url and not dry_run:
        out(f"Skipped: no webhook for #{result.channel}. Add the secret {WEBHOOK_ENV[result.channel]} to enable it.")
        _outputs(env, posted="false")
        return 0

    if github is None:
        if not dry_run:
            # Without the ledger we cannot rule out a duplicate, so we do not post.
            raise NotifyError("GITHUB_TOKEN is required to check for earlier announcements")
        out("Ledger not checked (no GITHUB_TOKEN).")
    elif github.already_announced(result.key):
        out(f"Already announced earlier (ledger entry {ledger_name(result.key)}); not posting again.")
        _outputs(env, posted="false")
        return 0

    if dry_run:
        out("Dry run — this message would be posted:")
        out(json.dumps(result.payload, indent=2, ensure_ascii=False))
        _outputs(env, posted="false")
        return 0

    webhook = Webhook(url, **net, **({"sleep": sleep} if sleep else {}))
    message_id = webhook.post(result.payload)
    out(f"Posted to #{result.channel}" + (f" (message {message_id})." if message_id else "."))
    _outputs(env, posted="true", ledger=ledger_name(result.key))
    return 0


def preview(event_name: str, path: Path, out=print) -> int:
    event = json.loads(path.read_text(encoding="utf-8"))
    settings = settings_from_env({k: v for k, v in os.environ.items() if not k.startswith("DISCORD_WEBHOOK_")})
    result = plan(event_name, event, settings, None, datetime.now(timezone.utc).date().isoformat())
    if isinstance(result, Skip):
        out(f"Nothing to announce: {result.reason}.")
        return 0
    out(f"{result.summary} (key {result.key})")
    out(json.dumps(result.payload, indent=2, ensure_ascii=False))
    return 0


def _outputs(env, **values: str) -> None:
    path = env.get("GITHUB_OUTPUT")
    if path:
        with open(path, "a", encoding="utf-8") as fh:
            for key, value in values.items():
                fh.write(f"{key}={value}\n")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="nexdir_notify", description="Nexdir GitHub → Discord announcements")
    sub = parser.add_subparsers(dest="command", required=True)
    run_p = sub.add_parser("run", help="announce the current GitHub Actions event")
    run_p.add_argument("--dry-run", action="store_true", help="print the message instead of posting it")
    prev = sub.add_parser("preview", help="show the message for a saved event payload (offline)")
    prev.add_argument("event_name", help="release, issues, pull_request_target, milestone or workflow_dispatch")
    prev.add_argument("file", type=Path)
    args = parser.parse_args(argv)

    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8")
    try:
        if args.command == "run":
            return run(os.environ, dry_run=args.dry_run)
        return preview(args.event_name, args.file)
    except NotifyError as err:
        print(f"::error::{err}")
        return 1
