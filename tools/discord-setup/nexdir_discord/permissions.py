"""Permission names and the overwrite rules derived from the configuration.

Permissions are handled as sets of discord.py flag names so the core logic does not depend
on discord.py and can be tested without it.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from .config import CategorySpec, ChannelSpec, ServerSpec

# Mirrors discord.Permissions.VALID_FLAGS (discord.py 2.7) so the core needs no import.
ALL_FLAGS = frozenset(
    """add_reactions administrator attach_files ban_members bypass_slowmode change_nickname connect
    create_events create_expressions create_instant_invite create_polls create_private_threads
    create_public_threads deafen_members embed_links external_emojis external_stickers kick_members
    manage_channels manage_emojis manage_emojis_and_stickers manage_events manage_expressions
    manage_guild manage_messages manage_nicknames manage_permissions manage_roles manage_threads
    manage_webhooks mention_everyone moderate_members move_members mute_members pin_messages
    priority_speaker read_message_history read_messages request_to_speak send_messages
    send_messages_in_threads send_polls send_tts_messages send_voice_messages
    set_voice_channel_status speak stream use_application_commands use_embedded_activities
    use_external_apps use_external_emojis use_external_sounds use_external_stickers use_soundboard
    use_voice_activation view_audit_log view_channel view_creator_monetization_analytics
    view_guild_insights""".split()
)

# discord.py exposes some bits under several names. Everything is compared by canonical name.
ALIASES = {
    "read_messages": "view_channel",
    "manage_permissions": "manage_roles",
    "manage_emojis": "manage_expressions",
    "manage_emojis_and_stickers": "manage_expressions",
    "use_external_emojis": "external_emojis",
    "use_external_stickers": "external_stickers",
}


def canonical(flags) -> frozenset[str]:
    return frozenset(ALIASES.get(f, f) for f in flags)


EVERYONE = "@everyone"
BOT = "@bot"

# What posting means in a channel. Denied for @everyone in read-only channels.
POSTING = frozenset({"send_messages", "send_messages_in_threads", "create_public_threads", "create_private_threads"})
# What the members of a private channel need.
PRIVATE_ACCESS = frozenset({"view_channel", "read_message_history", "send_messages", "send_messages_in_threads", "add_reactions", "attach_files", "embed_links"})
# What the setup bot needs inside private/read-only channels to keep managing them.
# Never "manage_permissions" (Manage Roles): Discord refuses that in a channel overwrite unless
# the bot is an Administrator ("Missing Permissions", seen on the real server). The bot's
# server-wide Manage Roles already lets it edit these overwrites, since no overwrite denies it.
BOT_ACCESS = frozenset({"view_channel", "read_message_history", "send_messages", "manage_channels", "manage_messages", "pin_messages", "manage_threads", "embed_links"})
MODERATION = frozenset({"manage_messages", "manage_threads", "pin_messages"})

# Permissions the bot uses itself, independent of the roles it creates.
BOT_BASE = frozenset(
    {
        "view_channel",
        "manage_roles",
        "manage_channels",
        "manage_guild",
        "send_messages",
        "send_messages_in_threads",
        "create_public_threads",
        "manage_threads",
        "manage_messages",
        "pin_messages",
        "embed_links",
        "read_message_history",
    }
)


@dataclass(frozen=True)
class Overwrite:
    allow: frozenset[str] = frozenset()
    deny: frozenset[str] = frozenset()

    def merge(self, other: "Overwrite") -> "Overwrite":
        allow = self.allow | other.allow
        return Overwrite(allow=allow, deny=(self.deny | other.deny) - allow)


@dataclass
class Overwrites:
    """Overwrites keyed by EVERYONE, BOT or a role name."""

    entries: dict[str, Overwrite] = field(default_factory=dict)

    def add(self, target: str, allow: set[str] | frozenset[str] = frozenset(), deny: set[str] | frozenset[str] = frozenset()) -> None:
        new = Overwrite(canonical(allow), canonical(deny))
        self.entries[target] = self.entries[target].merge(new) if target in self.entries else new

    def normalized(self) -> dict[str, Overwrite]:
        return {k.lower(): v for k, v in self.entries.items() if v.allow or v.deny}


def category_overwrites(cat: "CategorySpec") -> Overwrites:
    ow = Overwrites()
    if cat.private:
        ow.add(EVERYONE, deny={"view_channel"})
        ow.add(BOT, allow=BOT_ACCESS)
    return ow


def channel_overwrites(cat: "CategorySpec", ch: "ChannelSpec") -> Overwrites:
    """Overwrites for a channel. They are complete (not relying on category sync) so the
    intended visibility holds even if someone moves the channel later."""
    ow = category_overwrites(cat)
    if cat.private:
        # In read-only private channels (e.g. mod-logs) only the writers may post.
        access = PRIVATE_ACCESS - POSTING if ch.read_only else PRIVATE_ACCESS
        for role in ch.visible_to:
            ow.add(role, allow=access)
    if ch.read_only:
        ow.add(EVERYONE, allow=set() if cat.private else {"view_channel", "read_message_history", "add_reactions"}, deny=POSTING)
        ow.add(BOT, allow=BOT_ACCESS)
        for role in ch.writers:
            ow.add(role, allow=POSTING | {"view_channel", "read_message_history"})
    if ch.type == "forum" and not cat.private:
        ow.add(EVERYONE, allow={"view_channel", "read_message_history", "send_messages_in_threads", "create_public_threads"})
    for role in ch.moderators:
        ow.add(role, allow=MODERATION)
    return ow


def required_bot_permissions(spec: "ServerSpec") -> frozenset[str]:
    """Discord only lets a bot grant permissions it holds itself, so the bot needs the
    union of everything it assigns to roles and overwrites, plus its own base set."""
    needed = set(BOT_BASE)
    for role in spec.roles:
        needed |= role.permissions
    for cat, ch in spec.channels():
        for o in channel_overwrites(cat, ch).entries.values():
            needed |= o.allow | o.deny
    return canonical(needed)


# Bit values of the permission flags (Discord API), used to build the invite URL.
BITS: dict[str, int] = {
    "create_instant_invite": 0,
    "kick_members": 1,
    "ban_members": 2,
    "administrator": 3,
    "manage_channels": 4,
    "manage_guild": 5,
    "add_reactions": 6,
    "view_audit_log": 7,
    "priority_speaker": 8,
    "stream": 9,
    "view_channel": 10,
    "send_messages": 11,
    "send_tts_messages": 12,
    "manage_messages": 13,
    "embed_links": 14,
    "attach_files": 15,
    "read_message_history": 16,
    "mention_everyone": 17,
    "use_external_emojis": 18,
    "view_guild_insights": 19,
    "connect": 20,
    "speak": 21,
    "mute_members": 22,
    "deafen_members": 23,
    "move_members": 24,
    "use_voice_activation": 25,
    "change_nickname": 26,
    "manage_nicknames": 27,
    "manage_roles": 28,
    "manage_webhooks": 29,
    "manage_expressions": 30,
    "use_application_commands": 31,
    "request_to_speak": 32,
    "manage_events": 33,
    "manage_threads": 34,
    "create_public_threads": 35,
    "create_private_threads": 36,
    "use_external_stickers": 37,
    "send_messages_in_threads": 38,
    "use_embedded_activities": 39,
    "moderate_members": 40,
    "pin_messages": 51,
}


def permission_bits(flags: frozenset[str]) -> int:
    return sum(1 << BITS[f] for f in flags if f in BITS)


def invite_url(client_id: str, spec: "ServerSpec") -> str:
    bits = permission_bits(required_bot_permissions(spec))
    return f"https://discord.com/oauth2/authorize?client_id={client_id}&scope=bot&permissions={bits}"
