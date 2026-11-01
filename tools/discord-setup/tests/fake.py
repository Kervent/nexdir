"""In-memory Discord server implementing GuildApi, used instead of the real API in tests."""

from __future__ import annotations

import itertools
from dataclasses import replace

from nexdir_discord.model import ApiError, ChannelState, PostedMessage, RoleState, ServerState
from nexdir_discord.permissions import ALL_FLAGS, Overwrite

BOT_ID = 999
OWNER_ID = 42


class FakeGuild:
    def __init__(self, *, community: bool = True, bot_permissions: frozenset[str] | None = None, name: str = "Nexdir Community"):
        self.ids = itertools.count(1000)
        self.name = name
        self.description = ""
        self.icon: bytes | None = None
        self.community = community
        self.roles: list[RoleState] = [
            RoleState(id=1, name="@everyone", position=0, permissions=frozenset({"view_channel", "send_messages", "read_message_history"}), is_default=True),
            RoleState(id=2, name="Nexdir Setup", position=50, permissions=bot_permissions if bot_permissions is not None else ALL_FLAGS - {"administrator"}, managed=True),
        ]
        self.channels: list[ChannelState] = []
        self.messages: dict[int, list[PostedMessage]] = {}
        self.forum_posts: dict[int, list[PostedMessage]] = {}
        self.member_roles: dict[int, set[int]] = {OWNER_ID: set()}
        self.calls: list[str] = []
        self.fail: dict[str, ApiError] = {}  # method name or "method:target" → error to raise

    # helpers --------------------------------------------------------------------------
    def _check(self, method: str, target: str = "") -> None:
        self.calls.append(method)
        err = self.fail.get(f"{method}:{target}") or self.fail.get(method)
        if err:
            if err.retryable:
                # Retryable errors fail once, then succeed.
                self.fail.pop(f"{method}:{target}", None)
                self.fail.pop(method, None)
            raise err

    def _role(self, role_id: int) -> RoleState:
        return next(r for r in self.roles if r.id == role_id)

    def _channel(self, channel_id: int) -> ChannelState:
        for c in self.channels:
            if c.id == channel_id:
                return c
        raise ApiError("Unknown Channel")

    def _keys_ok(self, overwrites: dict[str, Overwrite]) -> None:
        names = {r.name.lower() for r in self.roles}
        for k in overwrites:
            if k not in ("@everyone", "@bot") and k not in names:
                raise ApiError(f"unknown role {k}")

    @property
    def mutations(self) -> list[str]:
        from nexdir_discord.model import MUTATING_METHODS

        return [c for c in self.calls if c in MUTATING_METHODS]

    bot_name = "Nexdir Setup#0001"

    async def close(self) -> None:
        self.calls.append("close")

    # GuildApi -------------------------------------------------------------------------
    async def snapshot(self) -> ServerState:
        bot_role = self.roles[1]
        return ServerState(
            id=1,
            name=self.name,
            description=self.description,
            has_icon=self.icon is not None,
            owner_id=OWNER_ID,
            community=self.community,
            roles=[replace(r) for r in self.roles],
            channels=[replace(c, overwrites=dict(c.overwrites), tags=list(c.tags)) for c in self.channels],
            bot_user_id=BOT_ID,
            bot_top_role_position=bot_role.position,
            bot_permissions=bot_role.permissions,
        )

    async def create_role(self, name, permissions, color, hoist, mentionable) -> int:
        self._check("create_role", name)
        rid = next(self.ids)
        self.roles.append(RoleState(id=rid, name=name, position=1, permissions=permissions, color=color or 0, hoist=hoist, mentionable=mentionable))
        return rid

    async def edit_role(self, role_id, *, permissions=None, color=None, hoist=None, mentionable=None) -> None:
        self._check("edit_role")
        r = self._role(role_id)
        if permissions is not None:
            r.permissions = permissions
        if color is not None:
            r.color = color
        if hoist is not None:
            r.hoist = hoist
        if mentionable is not None:
            r.mentionable = mentionable

    async def move_roles(self, positions) -> None:
        self._check("move_roles")
        for rid, pos in positions.items():
            self._role(rid).position = pos

    async def create_category(self, name, overwrites) -> int:
        self._check("create_category", name)
        self._keys_ok(overwrites)
        cid = next(self.ids)
        self.channels.append(ChannelState(id=cid, name=name, type="category", overwrites=dict(overwrites)))
        return cid

    async def create_channel(self, kind, name, category_id, topic, overwrites) -> int:
        self._check("create_channel", name)
        self._keys_ok(overwrites)
        if kind == "forum" and not self.community:
            raise ApiError("Forum channels require a Community server")
        cid = next(self.ids)
        self.channels.append(ChannelState(id=cid, name=name, type=kind, category_id=category_id, topic=topic, overwrites=dict(overwrites)))
        return cid

    async def edit_channel(self, channel_id, *, topic=None, overwrites=None, category_id=None) -> None:
        self._check("edit_channel")
        c = self._channel(channel_id)
        if topic is not None:
            c.topic = topic
        if category_id is not None:
            c.category_id = category_id
        if overwrites is not None:
            c.overwrites.update(overwrites)  # merge, like the real adapter

    async def create_forum_tag(self, channel_id, name) -> None:
        self._check("create_forum_tag", name)
        c = self._channel(channel_id)
        if c.type != "forum":
            raise ApiError("not a forum")
        c.tags.append(name)

    async def edit_server(self, *, name=None, description=None, icon=None) -> None:
        self._check("edit_server")
        if name is not None:
            self.name = name
        if description is not None:
            self.description = description
        if icon is not None:
            self.icon = icon

    async def send_message(self, channel_id, content) -> int:
        self._check("send_message")
        assert len(content) <= 2000, "Discord rejects messages over 2000 characters"
        mid = next(self.ids)
        self.messages.setdefault(channel_id, []).insert(0, PostedMessage(mid, content))  # newest first
        return mid

    async def edit_message(self, channel_id, message_id, content) -> None:
        self._check("edit_message")
        for store in (self.messages, self.forum_posts):
            for msgs in store.values():
                for m in msgs:
                    if m.id == message_id:
                        m.content = content
                        return
        raise ApiError("Unknown Message")

    async def pin_message(self, channel_id, message_id) -> None:
        self._check("pin_message")

    async def create_forum_post(self, channel_id, title, content) -> int:
        self._check("create_forum_post")
        mid = next(self.ids)
        self.forum_posts.setdefault(channel_id, []).append(PostedMessage(mid, content))
        return mid

    async def add_role_to_member(self, user_id, role_id) -> None:
        self._check("add_role_to_member")
        self.member_roles.setdefault(user_id, set()).add(role_id)

    async def bot_messages(self, channel_id, limit=100):
        self.calls.append("bot_messages")
        return list(self.messages.get(channel_id, []))[:limit]

    async def bot_forum_posts(self, channel_id):
        self.calls.append("bot_forum_posts")
        return list(self.forum_posts.get(channel_id, []))
