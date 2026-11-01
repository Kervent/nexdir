"""GuildApi implementation on top of discord.py, using the official Bot API over REST only.

No gateway connection and no privileged intents are needed. Messages are sent with all
mentions disabled, so the setup can never ping @everyone or anyone else.
"""

from __future__ import annotations

import aiohttp
import discord

from .model import ApiError, ChannelState, PostedMessage, RoleState, ServerState
from .permissions import BOT, EVERYONE, Overwrite, canonical

REASON = "Nexdir Discord setup"
NO_MENTIONS = discord.AllowedMentions.none()


def _flags(perms: discord.Permissions) -> frozenset[str]:
    return canonical(name for name in discord.Permissions.VALID_FLAGS if getattr(perms, name))


def _perms(flags: frozenset[str]) -> discord.Permissions:
    p = discord.Permissions.none()
    for f in flags:
        setattr(p, f, True)
    return p


def _wrap(e: Exception) -> ApiError:
    if isinstance(e, discord.Forbidden):
        return ApiError(f"Discord refused the request: {e.text or e}", missing_permissions=True)
    if isinstance(e, discord.NotFound):
        return ApiError(f"Not found: {e.text or e}")
    if isinstance(e, discord.HTTPException):
        return ApiError(f"Discord error {e.status}: {e.text or e}", retryable=e.status >= 500)
    if isinstance(e, (aiohttp.ClientError, TimeoutError)):
        return ApiError(f"Network error: {e}", retryable=True)
    return ApiError(str(e))


class DiscordGuildApi:
    def __init__(self, client: discord.Client, guild: discord.Guild):
        self.client = client
        self.guild = guild
        self.bot_id: int = client.user.id  # type: ignore[union-attr]

    @classmethod
    async def connect(cls, token: str, guild_id: int) -> "DiscordGuildApi":
        client = discord.Client(intents=discord.Intents.none())
        try:
            await client.login(token)
        except discord.LoginFailure as e:
            await client.close()
            raise ApiError("The bot token was rejected. Check DISCORD_BOT_TOKEN.") from e
        except Exception as e:
            await client.close()
            raise _wrap(e) from e
        try:
            guild = await client.fetch_guild(guild_id)
        except Exception as e:
            await client.close()
            if isinstance(e, (discord.Forbidden, discord.NotFound)):
                raise ApiError("The bot cannot see this server. Check DISCORD_GUILD_ID and that the bot was added to the server.") from e
            raise _wrap(e) from e
        return cls(client, guild)

    async def close(self) -> None:
        await self.client.close()

    @property
    def bot_name(self) -> str:
        return str(self.client.user)

    async def _call(self, coro):
        try:
            return await coro
        except ApiError:
            raise
        except Exception as e:  # noqa: BLE001
            raise _wrap(e) from e

    # --- reading ----------------------------------------------------------------------

    def _target_key(self, target) -> str:
        if isinstance(target, discord.Role):
            return EVERYONE if target.is_default() else target.name.lower()
        if getattr(target, "id", None) == self.bot_id:
            return BOT
        return f"member:{target.id}"

    async def snapshot(self) -> ServerState:
        self.guild = await self._call(self.client.fetch_guild(self.guild.id))
        roles = await self._call(self.guild.fetch_roles())
        channels = await self._call(self.guild.fetch_channels())
        me = await self._call(self.guild.fetch_member(self.bot_id))
        role_states = [
            RoleState(
                id=r.id,
                name=r.name,
                position=r.position,
                permissions=_flags(r.permissions),
                color=r.colour.value,
                hoist=r.hoist,
                mentionable=r.mentionable,
                managed=r.managed,
                is_default=r.is_default(),
            )
            for r in roles
        ]
        channel_states = []
        for ch in channels:
            kind = (
                "category"
                if isinstance(ch, discord.CategoryChannel)
                else "forum"
                if isinstance(ch, discord.ForumChannel)
                else "text"
                if isinstance(ch, discord.TextChannel)
                else "voice"
                if isinstance(ch, (discord.VoiceChannel, discord.StageChannel))
                else "other"
            )
            overwrites = {}
            for target, ow in ch.overwrites.items():
                allow, deny = ow.pair()
                overwrites[self._target_key(target)] = Overwrite(_flags(allow), _flags(deny))
            channel_states.append(
                ChannelState(
                    id=ch.id,
                    name=ch.name,
                    type=kind,
                    category_id=getattr(ch, "category_id", None),
                    position=ch.position,
                    topic=getattr(ch, "topic", None) or "",
                    overwrites=overwrites,
                    tags=[t.name for t in getattr(ch, "available_tags", [])],
                )
            )
        return ServerState(
            id=self.guild.id,
            name=self.guild.name,
            description=self.guild.description or "",
            has_icon=self.guild.icon is not None,
            owner_id=self.guild.owner_id or 0,
            community="COMMUNITY" in self.guild.features,
            roles=role_states,
            channels=channel_states,
            bot_user_id=self.bot_id,
            bot_top_role_position=me.top_role.position,
            bot_permissions=_flags(me.guild_permissions),
        )

    async def bot_messages(self, channel_id: int, limit: int = 100) -> list[PostedMessage]:
        channel = await self._call(self.client.fetch_channel(channel_id))
        out = []
        async for m in channel.history(limit=limit):
            if m.author.id == self.bot_id:
                out.append(PostedMessage(m.id, m.content))
        return out

    async def bot_forum_posts(self, channel_id: int) -> list[PostedMessage]:
        forum = await self._call(self.client.fetch_channel(channel_id))
        threads = [t for t in await self._call(self.guild.active_threads()) if t.parent_id == channel_id]
        async for t in forum.archived_threads(limit=100):
            threads.append(t)
        out = []
        for t in threads:
            if t.owner_id != self.bot_id:
                continue
            try:
                starter = await t.fetch_message(t.id)
                out.append(PostedMessage(t.id, starter.content))
            except discord.HTTPException:
                continue
        return out

    # --- changing ---------------------------------------------------------------------

    async def _resolve(self, overwrites: dict[str, Overwrite]) -> dict:
        roles = {r.name.lower(): r for r in await self._call(self.guild.fetch_roles())}
        resolved = {}
        for key, ow in overwrites.items():
            if key == EVERYONE:
                target = self.guild.default_role
            elif key == BOT:
                target = discord.Object(id=self.bot_id, type=discord.Member)
            elif key in roles:
                target = roles[key]
            else:
                raise ApiError(f'role "{key}" does not exist (its creation failed earlier)')
            resolved[target] = discord.PermissionOverwrite.from_pair(_perms(ow.allow), _perms(ow.deny))
        return resolved

    async def create_role(self, name, permissions, color, hoist, mentionable) -> int:
        role = await self._call(
            self.guild.create_role(
                name=name,
                permissions=_perms(permissions),
                colour=discord.Colour(color) if color is not None else discord.Colour.default(),
                hoist=hoist,
                mentionable=mentionable,
                reason=REASON,
            )
        )
        return role.id

    async def edit_role(self, role_id, *, permissions=None, color=None, hoist=None, mentionable=None) -> None:
        role = next((r for r in await self._call(self.guild.fetch_roles()) if r.id == role_id), None)
        if role is None:
            raise ApiError("role no longer exists")
        kwargs = {"reason": REASON}
        if permissions is not None:
            kwargs["permissions"] = _perms(permissions)
        if color is not None:
            kwargs["colour"] = discord.Colour(color)
        if hoist is not None:
            kwargs["hoist"] = hoist
        if mentionable is not None:
            kwargs["mentionable"] = mentionable
        await self._call(role.edit(**kwargs))

    async def move_roles(self, positions: dict[int, int]) -> None:
        await self._call(self.guild.edit_role_positions({discord.Object(id=k): v for k, v in positions.items()}, reason=REASON))

    async def create_category(self, name, overwrites) -> int:
        cat = await self._call(self.guild.create_category(name, overwrites=await self._resolve(overwrites), reason=REASON))
        return cat.id

    async def create_channel(self, kind, name, category_id, topic, overwrites) -> int:
        category = await self._call(self.client.fetch_channel(category_id)) if category_id else None
        resolved = await self._resolve(overwrites)
        if kind == "forum":
            ch = await self._call(self.guild.create_forum(name, category=category, topic=topic or discord.utils.MISSING, overwrites=resolved, reason=REASON))
        else:
            ch = await self._call(self.guild.create_text_channel(name, category=category, topic=topic or discord.utils.MISSING, overwrites=resolved, reason=REASON))
        return ch.id

    async def edit_channel(self, channel_id, *, topic=None, overwrites=None, category_id=None) -> None:
        ch = await self._call(self.client.fetch_channel(channel_id))
        kwargs: dict = {"reason": REASON}
        if topic is not None:
            kwargs["topic"] = topic
        if category_id is not None:
            kwargs["category"] = await self._call(self.client.fetch_channel(category_id))
        if overwrites is not None:
            # Merge: keep every existing overwrite that this tool does not manage.
            merged = dict(ch.overwrites)
            for target, ow in (await self._resolve(overwrites)).items():
                for existing in list(merged):
                    if getattr(existing, "id", None) == target.id:
                        del merged[existing]
                merged[target] = ow
            kwargs["overwrites"] = merged
        await self._call(ch.edit(**kwargs))

    async def create_forum_tag(self, channel_id, name) -> None:
        forum = await self._call(self.client.fetch_channel(channel_id))
        await self._call(forum.create_tag(name=name, reason=REASON))

    async def edit_server(self, *, name=None, description=None, icon=None) -> None:
        kwargs: dict = {"reason": REASON}
        if name is not None:
            kwargs["name"] = name
        if description is not None:
            kwargs["description"] = description
        if icon is not None:
            kwargs["icon"] = icon
        await self._call(self.guild.edit(**kwargs))

    async def send_message(self, channel_id, content) -> int:
        channel = await self._call(self.client.fetch_channel(channel_id))
        msg = await self._call(channel.send(content, allowed_mentions=NO_MENTIONS, suppress_embeds=True))
        return msg.id

    async def edit_message(self, channel_id, message_id, content) -> None:
        channel = await self._call(self.client.fetch_channel(channel_id))
        msg = await self._call(channel.fetch_message(message_id))
        await self._call(msg.edit(content=content, allowed_mentions=NO_MENTIONS))

    async def pin_message(self, channel_id, message_id) -> None:
        channel = await self._call(self.client.fetch_channel(channel_id))
        msg = await self._call(channel.fetch_message(message_id))
        await self._call(msg.pin(reason=REASON))

    async def create_forum_post(self, channel_id, title, content) -> int:
        forum = await self._call(self.client.fetch_channel(channel_id))
        created = await self._call(forum.create_thread(name=title, content=content, allowed_mentions=NO_MENTIONS, reason=REASON))
        await self._call(created.thread.edit(pinned=True, reason=REASON))
        return created.thread.id

    async def add_role_to_member(self, user_id, role_id) -> None:
        member = await self._call(self.guild.fetch_member(user_id))
        await self._call(member.add_roles(discord.Object(id=role_id), reason=REASON))
