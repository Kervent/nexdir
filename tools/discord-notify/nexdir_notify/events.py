"""Turns a GitHub event payload into one Discord announcement, or a reason to skip it.

Pure functions: no network, no environment. The CLI supplies the event JSON and settings.
"""

from __future__ import annotations

from datetime import datetime, timezone

from .model import (
    ANNOUNCE_LABEL,
    GITHUB_UPDATES,
    RELEASES,
    ROADMAP,
    SKIP_MARKER,
    Announcement,
    Settings,
    Skip,
)
from .text import first_paragraph, highlights, neutralise_mentions, parse_version, plain, truncate

BRAND_COLOR = 0x4F4FE6
PRERELEASE_COLOR = 0xE8A317
MILESTONE_COLOR = 0x0E9FBD

FOOTER = "Nexdir · GitHub"


def build(event_name: str, event: dict, settings: Settings) -> Announcement | Skip:
    action = event.get("action")
    if event_name == "release":
        if action != "published":
            return Skip(f"release action '{action}' is not announced")
        return release(event.get("release") or {}, settings)
    if event_name == "issues":
        return issue(event, settings)
    if event_name in ("pull_request", "pull_request_target"):
        return pull_request(event, settings)
    if event_name == "milestone":
        if action != "closed":
            return Skip(f"milestone action '{action}' is not announced")
        return milestone(event.get("milestone") or {}, settings)
    return Skip(f"event '{event_name}' is not announced")


# --- releases -------------------------------------------------------------------------


def release(rel: dict, settings: Settings) -> Announcement | Skip:
    tag = str(rel.get("tag_name") or "")
    if rel.get("draft"):
        return Skip(f"{tag or 'release'} is a draft")
    parsed = parse_version(tag)
    if parsed is None:
        return Skip(f"tag '{tag}' is not a version tag (expected v1.2.3 or v1.2.3-beta.1)")
    version, semver_pre = parsed
    body = rel.get("body") or ""
    if SKIP_MARKER in body.lower():
        return Skip(f"{tag}: release notes contain {SKIP_MARKER}")

    # A release is stable only if GitHub *and* the version number both say so. Anything
    # else is a pre-release and is never presented as a stable version.
    prerelease = bool(rel.get("prerelease")) or semver_pre
    if prerelease and not settings.announce_prereleases:
        return Skip(f"{tag} is a pre-release and pre-release announcements are turned off")

    url = rel.get("html_url") or f"{settings.repo_url}/releases/tag/{tag}"
    published = _date(rel.get("published_at"))
    items, more = highlights(body)

    if prerelease:
        title = f"Nexdir {version} pre-release"
        intro = "**Pre-release for testing — not a stable version.** Expect rough edges and keep backups of important files."
    else:
        title = f"Nexdir {version} is out"
        intro = ""
    lines = [intro] if intro else []
    if items:
        lines.append("**What's new**")
        lines += [f"• {item}" for item in items]
        if more:
            lines.append(f"…and {more} more in the [full release notes]({url}).")
    else:
        summary = first_paragraph(body)
        lines.append(summary or f"See the [release notes]({url}).")

    fields = [{"name": "Version", "value": version, "inline": True}]
    if published:
        fields.append({"name": "Released", "value": published.strftime("%d %B %Y").lstrip("0"), "inline": True})
    download = _download_link(rel, url)
    fields.append({"name": "Download", "value": download, "inline": False})
    fields.append({"name": "Links", "value": f"[Release notes]({url}) · [Roadmap]({_roadmap_url(settings)})", "inline": False})

    embed = _embed(
        title=title,
        url=url,
        description="\n".join(lines),
        color=PRERELEASE_COLOR if prerelease else BRAND_COLOR,
        fields=fields,
        timestamp=published,
    )
    role = settings.beta_role_id if prerelease else settings.release_role_id
    kind = "pre-release" if prerelease else "release"
    return Announcement(
        key=f"release-{tag}",
        channel=RELEASES,
        payload=_payload(embed, role),
        summary=f"{kind} {version} → #{RELEASES}" + (" (with role mention)" if role else ""),
    )


def _download_link(rel: dict, release_url: str) -> str:
    assets = {a.get("name", ""): a.get("browser_download_url") for a in rel.get("assets") or []}
    parts = []
    for suffix, label in (("-Setup.exe", "Installer"), ("-Portable.zip", "Portable"), (".msi", "MSI")):
        for name, link in assets.items():
            if name.endswith(suffix) and link:
                parts.append(f"[{label}]({link})")
                break
    if not parts:
        return f"[Download from GitHub]({release_url})"
    if any(name == "SHA256SUMS.txt" for name in assets):
        parts.append("checksums in SHA256SUMS.txt")
    return " · ".join(parts)


# --- issues and pull requests -----------------------------------------------------------


def issue(event: dict, settings: Settings) -> Announcement | Skip:
    issue_ = event.get("issue") or {}
    number = issue_.get("number")
    if event.get("action") != "labeled" or _label(event) != ANNOUNCE_LABEL:
        return Skip(f"issue #{number}: only the '{ANNOUNCE_LABEL}' label triggers an announcement")
    if "pull_request" in issue_:
        return Skip(f"#{number} is a pull request")
    if issue_.get("state") != "open":
        return Skip(f"issue #{number} is closed")
    labels = [l.get("name", "") for l in issue_.get("labels") or [] if l.get("name") != ANNOUNCE_LABEL]
    help_wanted = any(name in ("help wanted", "good first issue") for name in labels)
    description = f"Issue #{number} · opened by {plain(_login(issue_), 64)}"
    if help_wanted:
        description += "\n**Help wanted** — comment on the issue if you'd like to work on it."
    fields = [{"name": "Labels", "value": plain(", ".join(labels), 512), "inline": False}] if labels else []
    embed = _embed(
        title=plain(f"#{number} {issue_.get('title', '')}", 256),
        url=issue_.get("html_url") or f"{settings.repo_url}/issues/{number}",
        description=description,
        color=BRAND_COLOR,
        fields=fields,
    )
    return Announcement(
        key=f"issue-{number}",
        channel=GITHUB_UPDATES,
        payload=_payload(embed, None),
        summary=f"issue #{number} → #{GITHUB_UPDATES}",
    )


def pull_request(event: dict, settings: Settings) -> Announcement | Skip:
    pr = event.get("pull_request") or {}
    number = pr.get("number")
    action = event.get("action")
    labels = [l.get("name", "") for l in pr.get("labels") or []]
    if action == "labeled" and _label(event) != ANNOUNCE_LABEL:
        return Skip(f"PR #{number}: label '{_label(event)}' does not trigger an announcement")
    if action not in ("closed", "labeled"):
        return Skip(f"PR #{number}: action '{action}' is not announced")
    if not pr.get("merged"):
        return Skip(f"PR #{number} is not merged")
    if ANNOUNCE_LABEL not in labels:
        return Skip(f"PR #{number} does not have the '{ANNOUNCE_LABEL}' label")
    embed = _embed(
        title=plain(f"Merged: #{number} {pr.get('title', '')}", 256),
        url=pr.get("html_url") or f"{settings.repo_url}/pull/{number}",
        description=f"Pull request #{number} by {plain(_login(pr), 64)} was merged into "
        f"`{plain((pr.get('base') or {}).get('ref', 'main'), 64)}`. It will be part of the next release.",
        color=BRAND_COLOR,
    )
    return Announcement(
        key=f"pr-{number}",
        channel=GITHUB_UPDATES,
        payload=_payload(embed, None),
        summary=f"merged PR #{number} → #{GITHUB_UPDATES}",
    )


# --- milestones and roadmap -------------------------------------------------------------


def milestone(ms: dict, settings: Settings) -> Announcement | Skip:
    number = ms.get("number")
    title = plain(ms.get("title", ""), 200)
    if not title:
        return Skip("milestone has no title")
    closed = ms.get("closed_issues", 0)
    lines = [f"The milestone **{title}** is complete ({closed} issue{'s' if closed != 1 else ''} closed)."]
    description = plain(ms.get("description") or "", 600)
    if description:
        lines.append(description)
    lines.append(f"What comes next: [roadmap]({_roadmap_url(settings)}).")
    embed = _embed(
        title=f"Milestone reached: {title}",
        url=ms.get("html_url") or f"{settings.repo_url}/milestones",
        description="\n\n".join(lines),
        color=MILESTONE_COLOR,
    )
    return Announcement(
        key=f"milestone-{number}",
        channel=ROADMAP,
        payload=_payload(embed, settings.news_role_id),
        summary=f"milestone '{ms.get('title', '')}' → #{ROADMAP}",
    )


def roadmap(settings: Settings, *, note: str = "", key: str) -> Announcement:
    lines = ["The Nexdir roadmap shows what is done, what we are working on and what comes next."]
    if note:
        lines.append(truncate(neutralise_mentions(note.strip()), 1000))
    lines.append(f"[Read the roadmap]({_roadmap_url(settings)}) · [Open milestones]({settings.repo_url}/milestones)")
    embed = _embed(title="Nexdir roadmap", url=_roadmap_url(settings), description="\n\n".join(lines), color=MILESTONE_COLOR)
    return Announcement(key=key, channel=ROADMAP, payload=_payload(embed, None), summary=f"roadmap link → #{ROADMAP}")


# --- helpers ----------------------------------------------------------------------------


def _roadmap_url(settings: Settings) -> str:
    return f"{settings.repo_url}/blob/main/docs/ROADMAP.md"


def _label(event: dict) -> str:
    return str((event.get("label") or {}).get("name", ""))


def _login(item: dict) -> str:
    return str((item.get("user") or {}).get("login", "someone"))


def _date(value: str | None) -> datetime | None:
    if not value:
        return None
    try:
        return datetime.fromisoformat(value.replace("Z", "+00:00")).astimezone(timezone.utc)
    except ValueError:
        return None


def _embed(*, title: str, url: str, description: str, color: int, fields: list | None = None, timestamp=None) -> dict:
    embed = {
        "title": truncate(title, 256),
        "url": url,
        "description": truncate(description, 4000),
        "color": color,
        "footer": {"text": FOOTER},
    }
    if fields:
        embed["fields"] = [{**f, "name": truncate(f["name"], 256), "value": truncate(f["value"], 1024)} for f in fields[:25]]
    if timestamp:
        embed["timestamp"] = timestamp.isoformat()
    return embed


def _payload(embed: dict, role_id: str | None) -> dict:
    # Mentions are opt-in per role. @everyone/@here and user mentions are never allowed.
    payload: dict = {"embeds": [embed], "allowed_mentions": {"parse": []}}
    if role_id:
        payload["content"] = f"<@&{role_id}>"
        payload["allowed_mentions"]["roles"] = [role_id]
    return payload
