"""Library-independent snapshot of a Discord server and the interface used to change it.

The planner and executor only use these types, so they can be tested against an in-memory
fake server; `discord_api.py` implements the interface with discord.py.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Protocol

from .permissions import Overwrite


@dataclass
class RoleState:
    id: int
    name: str
    position: int
    permissions: frozenset[str]
    color: int = 0
    hoist: bool = False
    mentionable: bool = False
    managed: bool = False  # integration/bot roles cannot be edited
    is_default: bool = False  # @everyone


@dataclass
class ChannelState:
    id: int
    name: str
    type: str  # "category", "text", "forum", "voice", "other"
    category_id: int | None = None
    position: int = 0
    topic: str = ""
    overwrites: dict[str, Overwrite] = field(default_factory=dict)  # keys: "@everyone", "@bot", lowercase role names
    tags: list[str] = field(default_factory=list)


@dataclass
class ServerState:
    id: int
    name: str
    description: str
    has_icon: bool
    owner_id: int
    community: bool
    roles: list[RoleState]
    channels: list[ChannelState]
    bot_user_id: int
    bot_top_role_position: int
    bot_permissions: frozenset[str]

    def role(self, name: str) -> RoleState | None:
        return next((r for r in self.roles if r.name.lower() == name.lower() and not r.is_default), None)

    def category(self, name: str) -> ChannelState | None:
        return next((c for c in self.channels if c.type == "category" and c.name.lower() == name.lower()), None)

    def channel(self, name: str) -> ChannelState | None:
        """Channels are matched by name server-wide (Discord channel names are lowercase)."""
        return next((c for c in self.channels if c.type != "category" and c.name.lower() == name.lower()), None)


@dataclass
class PostedMessage:
    id: int
    content: str


class ApiError(Exception):
    """A Discord request failed. `retryable` marks temporary failures (5xx, network)."""

    def __init__(self, message: str, *, retryable: bool = False, missing_permissions: bool = False):
        super().__init__(message)
        self.retryable = retryable
        self.missing_permissions = missing_permissions


class GuildApi(Protocol):
    """Operations the setup needs. Every method that changes the server is listed here, so
    a dry run can be proven to make no changes by checking that none of them is called."""

    async def snapshot(self) -> ServerState: ...

    # Mutations
    async def create_role(self, name: str, permissions: frozenset[str], color: int | None, hoist: bool, mentionable: bool) -> int: ...
    async def edit_role(self, role_id: int, *, permissions: frozenset[str] | None = None, color: int | None = None, hoist: bool | None = None, mentionable: bool | None = None) -> None: ...
    async def move_roles(self, positions: dict[int, int]) -> None: ...
    async def create_category(self, name: str, overwrites: dict[str, Overwrite]) -> int: ...
    async def create_channel(self, kind: str, name: str, category_id: int | None, topic: str, overwrites: dict[str, Overwrite]) -> int: ...
    async def edit_channel(self, channel_id: int, *, topic: str | None = None, overwrites: dict[str, Overwrite] | None = None, category_id: int | None = None) -> None: ...
    async def create_forum_tag(self, channel_id: int, name: str) -> None: ...
    async def edit_server(self, *, name: str | None = None, description: str | None = None, icon: bytes | None = None) -> None: ...
    async def send_message(self, channel_id: int, content: str) -> int: ...
    async def edit_message(self, channel_id: int, message_id: int, content: str) -> None: ...
    async def pin_message(self, channel_id: int, message_id: int) -> None: ...
    async def create_forum_post(self, channel_id: int, title: str, content: str) -> int: ...
    async def add_role_to_member(self, user_id: int, role_id: int) -> None: ...

    # Reads
    async def bot_messages(self, channel_id: int, limit: int = 100) -> list[PostedMessage]: ...
    async def bot_forum_posts(self, channel_id: int) -> list[PostedMessage]: ...


MUTATING_METHODS = (
    "create_role",
    "edit_role",
    "move_roles",
    "create_category",
    "create_channel",
    "edit_channel",
    "create_forum_tag",
    "edit_server",
    "send_message",
    "edit_message",
    "pin_message",
    "create_forum_post",
    "add_role_to_member",
)
