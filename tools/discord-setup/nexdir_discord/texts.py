"""Info texts (welcome, rules, guides) from versioned templates in templates/*.md.

Texts are only posted after a preview and explicit approval. Each posted text carries an
invisible-ish marker line ("-# nexdir-setup:<name>:<version>"), which lets a later run see
that a text is already there (skip) or outdated (offer to edit) instead of posting twice.
"""

from __future__ import annotations

import hashlib
import re
from dataclasses import dataclass
from pathlib import Path

from .config import ServerSpec
from .model import GuildApi, ServerState

MAX_MESSAGE = 2000
MARKER_RE = re.compile(r"-# nexdir-setup:([a-z0-9-]+):([0-9a-f]{8})")
TITLES = {
    "support-guide": "📌 How to get help",
    "bug-report-guide": "📌 How to report a bug",
    "feature-request-guide": "📌 How to suggest a feature",
}


@dataclass
class TextPlan:
    template: str
    channel: str
    channel_id: int | None
    forum: bool
    chunks: list[str]
    status: str  # "new", "up to date", "outdated", "channel missing", "template missing"
    existing_ids: list[int]


def render(template: str, spec: ServerSpec) -> str:
    """Replaces {links.github}-style placeholders. Unknown placeholders are left visible."""

    def sub(m: re.Match) -> str:
        key = m.group(1)
        if key.startswith("links.") and key[6:] in spec.links:
            return spec.links[key[6:]]
        if key == "server.name":
            return spec.name
        return m.group(0)

    return re.sub(r"\{([a-z_.]+)\}", sub, template)


def version_of(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8")).hexdigest()[:8]


def split_message(text: str, name: str, limit: int = MAX_MESSAGE) -> list[str]:
    """Splits at paragraph (then line) boundaries; the marker goes on the last chunk."""
    marker = f"\n-# nexdir-setup:{name}:{version_of(text)}"
    chunks: list[str] = []
    current = ""
    for para in text.strip().split("\n\n"):
        piece = para if not current else "\n\n" + para
        if len(current) + len(piece) <= limit - len(marker):
            current += piece
            continue
        if current:
            chunks.append(current)
        current = ""
        for line in para.split("\n"):
            piece = line if not current else "\n" + line
            if len(current) + len(piece) > limit - len(marker):
                chunks.append(current)
                current = line[: limit - len(marker)]
            else:
                current += piece
    if current or not chunks:
        chunks.append(current)
    chunks[-1] += marker
    return chunks


def load_templates(directory: Path) -> dict[str, str]:
    return {p.stem: p.read_text(encoding="utf-8") for p in sorted(directory.glob("*.md"))}


async def plan_texts(spec: ServerSpec, state: ServerState, api: GuildApi, templates: dict[str, str]) -> list[TextPlan]:
    plans: list[TextPlan] = []
    for _, ch in spec.channels():
        if not ch.template:
            continue
        channel = state.channel(ch.name)
        if ch.template not in templates:
            plans.append(TextPlan(ch.template, ch.name, None, False, [], "template missing", []))
            continue
        text = render(templates[ch.template], spec)
        chunks = split_message(text, ch.template)
        if channel is None:
            plans.append(TextPlan(ch.template, ch.name, None, False, chunks, "channel missing", []))
            continue
        forum = channel.type == "forum"
        posted = await (api.bot_forum_posts(channel.id) if forum else api.bot_messages(channel.id))
        ours = [(m, MARKER_RE.search(m.content)) for m in posted]
        ours = [(m, mk) for m, mk in ours if mk and mk.group(1) == ch.template]
        if not ours:
            status, ids = "new", []
        elif any(mk.group(2) == version_of(text) for _, mk in ours):
            status, ids = "up to date", [m.id for m, _ in ours]
        else:
            status, ids = "outdated", [m.id for m, _ in ours]
        plans.append(TextPlan(ch.template, ch.name, channel.id, forum, chunks, status, ids))
    return plans


async def publish(plan: TextPlan, api: GuildApi) -> str:
    """Posts a new text (and pins it), or replaces an outdated one in place."""
    if plan.channel_id is None:
        return "skipped: channel missing"
    if plan.status == "up to date":
        return "skipped: already up to date"
    if plan.forum:
        if len(plan.chunks) != 1:
            return "skipped: forum guides must fit into one message (2,000 characters)"
        if plan.status == "outdated" and plan.existing_ids:
            # The starter message of a forum post has the same id as the post (thread).
            await api.edit_message(plan.existing_ids[0], plan.existing_ids[0], plan.chunks[0])
            return "updated forum post"
        title = TITLES.get(plan.template, f"📌 {plan.template.replace('-', ' ').capitalize()}")
        await api.create_forum_post(plan.channel_id, title, plan.chunks[0])
        return "created pinned forum post"
    if plan.status == "outdated" and len(plan.existing_ids) == len(plan.chunks):
        # Bot messages are listed newest first; the text was posted oldest first.
        for message_id, chunk in zip(reversed(plan.existing_ids), plan.chunks):
            await api.edit_message(plan.channel_id, message_id, chunk)
        return "updated"
    first = None
    for chunk in plan.chunks:
        message_id = await api.send_message(plan.channel_id, chunk)
        first = first or message_id
    if first is not None:
        await api.pin_message(plan.channel_id, first)
    return "posted and pinned" + (" (older version left in place; remove it manually)" if plan.status == "outdated" else "")
