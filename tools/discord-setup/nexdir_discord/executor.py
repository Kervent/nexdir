"""Applies a plan through a GuildApi.

* Changes run in plan order (roles → hierarchy → categories → channels → tags).
* Updates of existing items run only with `allow_updates=True`.
* Temporary failures are retried with backoff; Discord's own rate limits are handled by
  discord.py. A failed change is recorded and the run continues with the next change.
* Every result is appended to a journal file, so an interrupted run is documented. Running
  the setup again is safe: the new plan contains only what is still missing.
"""

from __future__ import annotations

import asyncio
import json
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Awaitable, Callable

from .model import ApiError, GuildApi, ServerState
from .permissions import category_overwrites, channel_overwrites
from .planner import Change, Plan

RETRIES = 3


@dataclass
class Result:
    change: Change
    status: str  # "done", "skipped", "failed"
    message: str = ""


Sleep = Callable[[float], Awaitable[None]]


class Executor:
    def __init__(self, api: GuildApi, state: ServerState, *, journal: Path | None = None, sleep: Sleep | None = None, pause: float = 0.3):
        self.api = api
        self.state = state
        self.journal = journal
        self.sleep = sleep or asyncio.sleep
        self.pause = pause
        # Name → id maps, kept current as objects are created.
        self.role_ids = {r.name.lower(): r.id for r in state.roles if not r.is_default}
        self.category_ids = {c.name.lower(): c.id for c in state.channels if c.type == "category"}
        self.channel_ids = {c.name.lower(): c.id for c in state.channels if c.type != "category"}

    async def run(self, plan: Plan, *, allow_updates: bool, on_result: Callable[[Result], None] | None = None) -> list[Result]:
        results: list[Result] = []
        for change in plan.changes:
            if change.existing and not allow_updates:
                result = Result(change, "skipped", "update of an existing item not allowed")
            else:
                result = await self._apply_with_retry(change)
                await self.sleep(self.pause)  # stay well below Discord's rate limits
            results.append(result)
            self._journal(result)
            if on_result:
                on_result(result)
        return results

    async def _apply_with_retry(self, change: Change) -> Result:
        delay = 1.0
        for attempt in range(1, RETRIES + 1):
            try:
                await self._apply(change)
                return Result(change, "done")
            except ApiError as e:
                if e.retryable and attempt < RETRIES:
                    await self.sleep(delay)
                    delay *= 2
                    continue
                hint = " (the bot is missing a permission)" if e.missing_permissions else ""
                return Result(change, "failed", f"{e}{hint}")
            except Exception as e:  # noqa: BLE001 - one failing change must not stop the run
                return Result(change, "failed", f"unexpected error: {e}")
        return Result(change, "failed", "gave up after retries")

    async def _apply(self, c: Change) -> None:
        d = c.data
        if c.kind == "server_name":
            await self.api.edit_server(name=d["name"])
        elif c.kind == "server_description":
            await self.api.edit_server(description=d["description"])
        elif c.kind == "create_role":
            r = d["role"]
            self.role_ids[r.name.lower()] = await self.api.create_role(r.name, r.permissions, r.color, r.hoist, r.mentionable)
        elif c.kind == "update_role":
            r = d["role"]
            await self.api.edit_role(d["role_id"], permissions=r.permissions, color=r.color, hoist=r.hoist, mentionable=r.mentionable)
        elif c.kind == "order_roles":
            await self._order_roles(d["order"])
        elif c.kind == "create_category":
            cat = d["category"]
            self.category_ids[cat.name.lower()] = await self.api.create_category(cat.name, category_overwrites(cat).normalized())
        elif c.kind == "update_category":
            await self.api.edit_channel(d["channel_id"], overwrites=category_overwrites(d["category"]).normalized())
        elif c.kind == "create_channel":
            cat, ch = d["category"], d["channel"]
            cat_id = self.category_ids.get(cat.name.lower())
            if cat_id is None:
                raise ApiError(f'category "{cat.name}" does not exist (its creation failed earlier)')
            self.channel_ids[ch.name.lower()] = await self.api.create_channel(d["kind"], ch.name, cat_id, ch.topic, channel_overwrites(cat, ch).normalized())
        elif c.kind == "update_channel":
            cat, ch = d["category"], d["channel"]
            await self.api.edit_channel(
                d["channel_id"],
                topic=d.get("topic"),
                overwrites=channel_overwrites(cat, ch).normalized() if d.get("overwrites") else None,
                category_id=self.category_ids.get(cat.name.lower()) if d.get("move") else None,
            )
        elif c.kind == "create_tag":
            channel_id = self.channel_ids.get(d["channel"].lower())
            if channel_id is None:
                raise ApiError(f'forum "#{d["channel"]}" does not exist')
            await self.api.create_forum_tag(channel_id, d["tag"])
        else:
            raise ApiError(f"unknown change type {c.kind}")

    async def _order_roles(self, order: list[str]) -> None:
        """Places the configured roles (highest first) directly below the bot's top role."""
        top = self.state.bot_top_role_position
        positions: dict[int, int] = {}
        for i, name in enumerate(order):
            role_id = self.role_ids.get(name.lower())
            if role_id is not None:
                positions[role_id] = max(1, top - 1 - i)
        if positions:
            await self.api.move_roles(positions)

    def _journal(self, result: Result) -> None:
        if not self.journal:
            return
        self.journal.parent.mkdir(parents=True, exist_ok=True)
        entry = {"time": time.strftime("%Y-%m-%dT%H:%M:%S"), "change": result.change.key, "status": result.status, "message": result.message}
        with self.journal.open("a", encoding="utf-8") as f:
            f.write(json.dumps(entry, ensure_ascii=False) + "\n")
