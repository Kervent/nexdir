"""Human-readable reports of plans, applied changes and verification."""

from __future__ import annotations

import time
from pathlib import Path

from .executor import Result
from .model import ServerState
from .planner import Plan

MANUAL_STEPS = [
    "Server Settings → Enable Community (needed for forums, server description, Onboarding and rules screening). Then run the setup again.",
    "Server Settings → Onboarding: add a question 'What are you interested in?' with the 'Interested: …' roles and one for notifications (Release Notifications, Beta Updates, Development News). The setup does not change Onboarding so it never overwrites your existing configuration.",
    "Server Settings → Safety Setup: verification level Medium or higher, explicit media filter for all members, enable AutoMod spam filters.",
    "Server Settings → Overview: default notifications 'Only @mentions'.",
    "Server Settings → Community → set #rules as the rules channel and #announcements as the updates channel.",
    "Give the Founder role to the project owner only (menu: 'Assign Founder to the server owner'). Team roles are assigned manually after an application was accepted; Administrator is never assigned by the tool.",
    "After the setup, remove the setup bot or reduce its permissions. Permanent GitHub announcements use separate channel webhooks for #releases, #github-updates and #roadmap — see tools/discord-notify/README.md.",
]


def plan_text(plan: Plan) -> str:
    lines: list[str] = []
    if plan.blockers:
        lines.append("BLOCKERS (nothing can be applied until these are fixed):")
        lines += [f"  ✖ {b}" for b in plan.blockers]
        lines.append("")
    creations, updates = plan.creations, plan.updates
    lines.append(f"New items to create: {len(creations)}")
    lines += [f"  + {c.description}" for c in creations]
    lines.append("")
    lines.append(f"Changes to EXISTING items (applied only if you allow it): {len(updates)}")
    lines += [f"  ~ {c.description}" for c in updates]
    if plan.warnings:
        lines.append("")
        lines.append("Warnings:")
        lines += [f"  ! {w}" for w in plan.warnings]
    lines.append("")
    lines.append("Nothing is ever deleted by this tool.")
    return "\n".join(lines)


def state_summary(state: ServerState) -> str:
    cats = [c for c in state.channels if c.type == "category"]
    chans = [c for c in state.channels if c.type != "category"]
    return (
        f"Server: {state.name} (id {state.id})\n"
        f"Community enabled: {'yes' if state.community else 'no'} · icon set: {'yes' if state.has_icon else 'no'}\n"
        f"Roles: {len([r for r in state.roles if not r.is_default])} · categories: {len(cats)} · channels: {len(chans)}"
    )


def write_report(directory: Path, title: str, state: ServerState | None, plan: Plan | None, results: list[Result] | None, extra: list[str] | None = None) -> Path:
    directory.mkdir(parents=True, exist_ok=True)
    path = directory / f"report-{time.strftime('%Y%m%d-%H%M%S')}.md"
    out = [f"# {title}", "", f"Created: {time.strftime('%Y-%m-%d %H:%M:%S')}", ""]
    if state:
        out += ["## Server", "", "```", state_summary(state), "```", ""]
    if plan:
        out += ["## Plan", "", "```", plan_text(plan), "```", ""]
    if results is not None:
        done = [r for r in results if r.status == "done"]
        failed = [r for r in results if r.status == "failed"]
        skipped = [r for r in results if r.status == "skipped"]
        out += [f"## Result: {len(done)} done, {len(failed)} failed, {len(skipped)} skipped", ""]
        out += [f"- ✔ {r.change.description}" for r in done]
        out += [f"- ✖ {r.change.description} — {r.message}" for r in failed]
        out += [f"- ○ {r.change.description} — {r.message}" for r in skipped]
        out.append("")
    if extra:
        out += extra + [""]
    out += ["## Manual steps", ""] + [f"- [ ] {s}" for s in MANUAL_STEPS]
    path.write_text("\n".join(out) + "\n", encoding="utf-8")
    return path
