"""Text helpers. Issue and pull request titles come from anyone on GitHub, so they are
escaped; release notes are written by maintainers and keep their Markdown, but mentions are
always neutralised."""

from __future__ import annotations

import re

_CONTROL = re.compile(r"[\x00-\x08\x0b-\x1f\x7f‎‏‪-‮⁦-⁩]")
# "<" too, so untrusted text cannot form <@user>, <#channel> or <t:...> tokens.
_MARKDOWN = re.compile(r"([\\*_~`|>\[\]<])")
_IMAGE = re.compile(r"!\[[^\]]*\]\([^)]*\)")
_HTML_COMMENT = re.compile(r"<!--.*?-->", re.DOTALL)
_BULLET = re.compile(r"^[-*+]\s+(.*\S)\s*$")
_SEMVER = re.compile(r"^v?(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z.-]+))?(?:\+[0-9A-Za-z.-]+)?$")

ZWSP = "​"


def neutralise_mentions(text: str) -> str:
    """Breaks @everyone, @here and <@...> so they never render as mentions, even if
    allowed_mentions were misconfigured."""
    return text.replace("@", "@" + ZWSP)


def clean(text: str) -> str:
    return _CONTROL.sub("", text).replace("\r\n", "\n").replace("\r", "\n")


def plain(text: str, limit: int) -> str:
    """Untrusted single-line text (titles, names): no Markdown, no mentions, no newlines."""
    text = " ".join(clean(text).split())
    text = _MARKDOWN.sub(r"\\\1", text)
    return truncate(neutralise_mentions(text), limit)


def truncate(text: str, limit: int) -> str:
    if len(text) <= limit:
        return text
    cut = text[: limit - 1].rstrip()
    # Never end on a lone escape backslash.
    while cut.endswith("\\") and not cut.endswith("\\\\"):
        cut = cut[:-1]
    return cut + "…"


def parse_version(tag: str) -> tuple[str, bool] | None:
    """Returns (version without "v", is_prerelease) for semantic version tags."""
    match = _SEMVER.match(tag.strip())
    if not match:
        return None
    version = tag.strip().removeprefix("v")
    return version, match.group(4) is not None


def highlights(body: str | None, *, max_items: int = 8, item_limit: int = 180) -> tuple[list[str], int]:
    """Top-level bullet points of release notes, in order. Returns (items, number left out)."""
    if not body:
        return [], 0
    text = _HTML_COMMENT.sub("", clean(body))
    items: list[str] = []
    for line in text.split("\n"):
        if line[:1] in (" ", "\t"):
            continue  # nested bullets stay in the full release notes
        match = _BULLET.match(line)
        if not match:
            continue
        item = _IMAGE.sub("", match.group(1)).strip()
        if item:
            items.append(truncate(neutralise_mentions(item), item_limit))
    return items[:max_items], max(0, len(items) - max_items)


def first_paragraph(body: str | None, limit: int = 600) -> str:
    if not body:
        return ""
    text = _IMAGE.sub("", _HTML_COMMENT.sub("", clean(body))).strip()
    for block in re.split(r"\n\s*\n", text):
        block = block.strip()
        if block and not block.startswith("#"):
            return truncate(neutralise_mentions(block), limit)
    return ""
