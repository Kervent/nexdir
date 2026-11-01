"""Checks that the live server matches the configuration and its safety rules."""

from __future__ import annotations

from .config import ServerSpec
from .model import ServerState
from .permissions import EVERYONE, POSTING
from .planner import plan_changes


def verify(spec: ServerSpec, state: ServerState) -> tuple[list[str], list[str]]:
    """Returns (passed checks, problems)."""
    ok: list[str] = []
    problems: list[str] = []
    plan = plan_changes(spec, state)
    pending = [c for c in plan.changes if c.kind not in ("server_name", "server_description")]
    if pending:
        problems += [f"Not as configured: {c.description}" for c in pending]
    else:
        ok.append("All configured roles, categories, channels and tags exist with the configured settings.")

    admin_roles = [r.name for r in spec.roles if (live := state.role(r.name)) and "administrator" in live.permissions]
    if admin_roles:
        problems += [f'Role "{name}" has the Administrator permission (not set by this tool).' for name in admin_roles]
    else:
        ok.append("No configured role carries the Administrator permission.")

    for cat, ch in spec.channels():
        live = state.channel(ch.name)
        if live is None:
            continue
        everyone = live.overwrites.get(EVERYONE)
        if cat.private and not (everyone and "view_channel" in everyone.deny):
            problems.append(f'"#{ch.name}" should be hidden from @everyone but is not.')
        if ch.read_only and not (everyone and POSTING & everyone.deny):
            problems.append(f'"#{ch.name}" should be read-only for @everyone but is not.')
    if not any("hidden" in p or "read-only" in p for p in problems):
        ok.append("Private channels are hidden from @everyone; read-only channels block posting.")

    if state.name != spec.name:
        problems.append(f'Server name is "{state.name}", configured "{spec.name}".')
    if not state.has_icon:
        problems.append("The server has no icon yet (menu: Update server icon).")
    return ok, problems
