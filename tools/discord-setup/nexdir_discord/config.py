"""Loads and validates the desired server structure (config/server.toml)."""

from __future__ import annotations

import re
import tomllib
from dataclasses import dataclass, field
from pathlib import Path

from .permissions import ALL_FLAGS, canonical

ROLE_GROUPS = ("team", "community", "notification", "interest")
CHANNEL_TYPES = ("text", "forum")
MAX_FORUM_TAGS = 20
MAX_TOPIC_TEXT = 1024
MAX_TOPIC_FORUM = 4096


class ConfigError(ValueError):
    """The configuration file is invalid. The message lists every problem."""


@dataclass(frozen=True)
class RoleSpec:
    name: str
    group: str
    permissions: frozenset[str] = frozenset()
    color: int | None = None
    hoist: bool = False
    mentionable: bool = False


@dataclass(frozen=True)
class ChannelSpec:
    name: str
    type: str
    topic: str = ""
    read_only: bool = False
    writers: tuple[str, ...] = ()
    visible_to: tuple[str, ...] = ()
    moderators: tuple[str, ...] = ()
    tags: tuple[str, ...] = ()
    template: str | None = None


@dataclass(frozen=True)
class CategorySpec:
    name: str
    private: bool = False
    channels: tuple[ChannelSpec, ...] = ()


@dataclass(frozen=True)
class ServerSpec:
    name: str
    description: str
    icon: Path | None
    forum_fallback: str
    links: dict[str, str] = field(default_factory=dict)
    roles: tuple[RoleSpec, ...] = ()
    categories: tuple[CategorySpec, ...] = ()

    def role(self, name: str) -> RoleSpec | None:
        return next((r for r in self.roles if r.name.lower() == name.lower()), None)

    def channels(self) -> list[tuple[CategorySpec, ChannelSpec]]:
        return [(c, ch) for c in self.categories for ch in c.channels]


def _color(value: object, where: str, errors: list[str]) -> int | None:
    if value is None:
        return None
    if isinstance(value, str) and re.fullmatch(r"#[0-9A-Fa-f]{6}", value):
        return int(value[1:], 16)
    errors.append(f"{where}: color must look like '#5B5BF7'")
    return None


def load_config(path: Path) -> ServerSpec:
    with path.open("rb") as f:
        raw = tomllib.load(f)
    return parse_config(raw, base_dir=path.parent)


def parse_config(raw: dict, base_dir: Path) -> ServerSpec:
    errors: list[str] = []
    server = raw.get("server", {})
    name = str(server.get("name", "")).strip()
    if not 2 <= len(name) <= 100:
        errors.append("server.name must be 2–100 characters")
    fallback = server.get("forum_fallback", "text")
    if fallback not in ("text", "skip"):
        errors.append("server.forum_fallback must be 'text' or 'skip'")
    icon = (base_dir / server["icon"]).resolve() if server.get("icon") else None

    roles: list[RoleSpec] = []
    seen_roles: set[str] = set()
    for i, r in enumerate(raw.get("roles", [])):
        where = f"roles[{i}] ({r.get('name', '?')})"
        rname = str(r.get("name", "")).strip()
        if not 1 <= len(rname) <= 100:
            errors.append(f"{where}: name must be 1–100 characters")
        if rname.lower() in seen_roles:
            errors.append(f"{where}: duplicate role name")
        seen_roles.add(rname.lower())
        group = r.get("group")
        if group not in ROLE_GROUPS:
            errors.append(f"{where}: group must be one of {', '.join(ROLE_GROUPS)}")
        perms = frozenset(r.get("permissions", []))
        unknown = perms - ALL_FLAGS
        if unknown:
            errors.append(f"{where}: unknown permissions {sorted(unknown)}")
        # Safety rules from the project brief.
        if "administrator" in perms:
            errors.append(f"{where}: the 'administrator' flag is never granted by this tool")
        if group in ("interest", "notification") and perms:
            errors.append(f"{where}: {group} roles must not carry permissions")
        roles.append(
            RoleSpec(
                name=rname,
                group=group or "community",
                permissions=canonical(perms),
                color=_color(r.get("color"), where, errors),
                hoist=bool(r.get("hoist", False)),
                mentionable=bool(r.get("mentionable", False)),
            )
        )

    role_names = {r.name.lower() for r in roles}

    def check_roles(names: list[str], where: str) -> tuple[str, ...]:
        for n in names:
            if n.lower() not in role_names:
                errors.append(f"{where}: unknown role '{n}'")
        return tuple(names)

    categories: list[CategorySpec] = []
    seen_channels: set[str] = set()
    seen_categories: set[str] = set()
    for c in raw.get("categories", []):
        cname = str(c.get("name", "")).strip()
        if cname.lower() in seen_categories:
            errors.append(f"category '{cname}': duplicate name")
        seen_categories.add(cname.lower())
        private = bool(c.get("private", False))
        channels: list[ChannelSpec] = []
        for ch in c.get("channels", []):
            where = f"channel '{ch.get('name', '?')}'"
            chname = str(ch.get("name", ""))
            if not re.fullmatch(r"[a-z0-9][a-z0-9_-]{0,99}", chname):
                errors.append(f"{where}: use lowercase letters, digits, '-' or '_'")
            if chname in seen_channels:
                errors.append(f"{where}: duplicate channel name")
            seen_channels.add(chname)
            ctype = ch.get("type", "text")
            if ctype not in CHANNEL_TYPES:
                errors.append(f"{where}: type must be 'text' or 'forum'")
            topic = str(ch.get("topic", ""))
            if len(topic) > (MAX_TOPIC_FORUM if ctype == "forum" else MAX_TOPIC_TEXT):
                errors.append(f"{where}: topic is too long")
            tags = tuple(ch.get("tags", []))
            if tags and ctype != "forum":
                errors.append(f"{where}: only forum channels have tags")
            if len(tags) > MAX_FORUM_TAGS or len({t.lower() for t in tags}) != len(tags):
                errors.append(f"{where}: at most {MAX_FORUM_TAGS} unique tags")
            visible_to = check_roles(list(ch.get("visible_to", [])), where)
            if private and not visible_to:
                errors.append(f"{where}: channels in a private category need 'visible_to'")
            if not private and visible_to:
                errors.append(f"{where}: 'visible_to' is only for private categories")
            writers = check_roles(list(ch.get("writers", [])), where)
            read_only = bool(ch.get("read_only", False))
            if writers and not read_only:
                errors.append(f"{where}: 'writers' requires read_only = true")
            channels.append(
                ChannelSpec(
                    name=chname,
                    type=ctype,
                    topic=topic,
                    read_only=read_only,
                    writers=writers,
                    visible_to=visible_to,
                    moderators=check_roles(list(ch.get("moderators", [])), where),
                    tags=tags,
                    template=ch.get("template"),
                )
            )
        categories.append(CategorySpec(name=cname, private=private, channels=tuple(channels)))

    if errors:
        raise ConfigError("Invalid configuration:\n  - " + "\n  - ".join(errors))
    return ServerSpec(
        name=name,
        description=str(server.get("description", "")),
        icon=icon,
        forum_fallback=fallback,
        links={k: str(v) for k, v in raw.get("links", {}).items()},
        roles=tuple(roles),
        categories=tuple(categories),
    )
