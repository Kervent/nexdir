"""Exports the readable server configuration before changes are made.

This is NOT a complete backup of a Discord server: messages, members, emojis, webhooks,
integrations, onboarding, AutoMod rules and many settings are not included and cannot be
restored from this file.
"""

from __future__ import annotations

import json
import time
from pathlib import Path

from .model import ServerState

DISCLAIMER = (
    "Readable configuration snapshot created by the Nexdir Discord setup. "
    "This is not a complete backup: messages, members, emojis, webhooks, integrations, onboarding "
    "and many server settings are not included, and nothing can be restored automatically from it."
)


def snapshot_to_dict(state: ServerState) -> dict:
    by_id = {c.id: c.name for c in state.channels if c.type == "category"}
    return {
        "_note": DISCLAIMER,
        "exported_at": time.strftime("%Y-%m-%dT%H:%M:%S"),
        "server": {"id": str(state.id), "name": state.name, "description": state.description, "has_icon": state.has_icon, "community": state.community},
        "roles": [
            {
                "name": r.name,
                "id": str(r.id),
                "position": r.position,
                "permissions": sorted(r.permissions),
                "color": f"#{r.color:06X}",
                "hoist": r.hoist,
                "mentionable": r.mentionable,
                "managed": r.managed,
            }
            for r in sorted(state.roles, key=lambda r: -r.position)
        ],
        "channels": [
            {
                "name": c.name,
                "id": str(c.id),
                "type": c.type,
                "category": by_id.get(c.category_id) if c.category_id else None,
                "position": c.position,
                "topic": c.topic,
                "overwrites": {k: {"allow": sorted(v.allow), "deny": sorted(v.deny)} for k, v in sorted(c.overwrites.items())},
                **({"tags": c.tags} if c.type == "forum" else {}),
            }
            for c in sorted(state.channels, key=lambda c: (c.category_id or 0, c.position))
        ],
    }


def export_snapshot(state: ServerState, directory: Path) -> Path:
    directory.mkdir(parents=True, exist_ok=True)
    path = directory / f"server-config-{time.strftime('%Y%m%d-%H%M%S')}.json"
    path.write_text(json.dumps(snapshot_to_dict(state), indent=2, ensure_ascii=False), encoding="utf-8")
    return path
