import json

import pytest
from fake import FakeGuild

from nexdir_discord.executor import Executor
from nexdir_discord.export import DISCLAIMER, export_snapshot
from nexdir_discord.model import ApiError, ChannelState, RoleState
from nexdir_discord.permissions import ALL_FLAGS, Overwrite
from nexdir_discord.planner import plan_changes
from nexdir_discord.verify import verify


async def apply(spec, guild, *, allow_updates=False, no_sleep=None, journal=None):
    state = await guild.snapshot()
    plan = plan_changes(spec, state)
    results = await Executor(guild, state, sleep=no_sleep, pause=0, journal=journal).run(plan, allow_updates=allow_updates)
    return plan, results


async def test_empty_server_gets_full_structure_and_second_run_is_a_noop(spec, no_sleep):
    guild = FakeGuild(name="Nexdir Community")
    plan, results = await apply(spec, guild, no_sleep=no_sleep)
    assert not plan.blockers
    assert all(r.status == "done" for r in results), [(r.change.key, r.message) for r in results if r.status != "done"]
    state = await guild.snapshot()
    assert {r.name for r in state.roles} >= {r.name for r in spec.roles}
    assert {c.name for c in state.channels} >= {ch.name for _, ch in spec.channels()} | {c.name for c in spec.categories}
    bugs = state.channel("bug-reports")
    assert bugs.type == "forum" and bugs.tags == list(next(ch for _, ch in spec.channels() if ch.name == "bug-reports").tags)
    # Roles are ordered as configured, below the bot's role.
    ordered = sorted((r for r in state.roles if spec.role(r.name)), key=lambda r: -r.position)
    assert [r.name for r in ordered] == [r.name for r in spec.roles]
    assert all(r.position < state.roles[1].position for r in ordered)

    # Idempotent: nothing left to do, no duplicates.
    second = plan_changes(spec, state)
    assert second.is_empty, [c.description for c in second.changes]
    names = [c.name for c in state.channels]
    assert len(names) == len(set(names))
    ok, problems = verify(spec, state)
    assert problems == ["The server has no icon yet (menu: Update server icon)."]


async def test_dry_run_makes_no_changes(spec):
    guild = FakeGuild()
    plan = plan_changes(spec, await guild.snapshot())
    assert plan.changes
    assert guild.mutations == []


async def test_existing_items_are_reused_never_deleted_and_updates_need_permission(spec, no_sleep):
    guild = FakeGuild(name="Old Name")
    # Pre-existing content: a configured role with other settings, an extra role, an
    # extra channel and a configured channel in the wrong place.
    guild.roles.append(RoleState(id=10, name="moderator", position=5, permissions=frozenset({"kick_members"})))
    guild.roles.append(RoleState(id=11, name="Cool People", position=4, permissions=frozenset()))
    guild.channels.append(ChannelState(id=20, name="memes", type="text"))
    guild.channels.append(ChannelState(id=21, name="general", type="text", topic="old topic", overwrites={"member:5": Overwrite(allow=frozenset({"manage_messages"}))}))

    plan, results = await apply(spec, guild, no_sleep=no_sleep)
    keys = {c.key for c in plan.updates}
    assert "server_name:server" in keys and "update_role:Moderator" in keys and "update_channel:general" in keys
    assert all(r.status == "skipped" for r in results if r.change.existing)
    state = await guild.snapshot()
    assert state.name == "Old Name", "renaming the server needs explicit permission"
    assert len([r for r in state.roles if r.name.lower() == "moderator"]) == 1, "no duplicate role"
    assert state.role("Moderator").permissions == frozenset({"kick_members"}), "existing role unchanged without permission"
    assert state.channel("memes") and state.role("Cool People"), "nothing is deleted"
    assert guild.mutations.count("create_channel") == len(spec.channels()) - 1

    # With permission, updates are applied and unrelated overwrites are kept.
    plan, results = await apply(spec, guild, allow_updates=True, no_sleep=no_sleep)
    state = await guild.snapshot()
    assert state.name == "Nexdir Community"
    assert state.role("Moderator").permissions == spec.role("Moderator").permissions
    general = state.channel("general")
    assert general.topic == "General discussion about Nexdir." and general.category_id == state.category("COMMUNITY").id
    assert "member:5" in general.overwrites, "overwrites for other members are kept"
    assert plan_changes(spec, state).is_empty


async def test_without_community_forums_become_text_channels(spec, no_sleep):
    guild = FakeGuild(community=False)
    plan, results = await apply(spec, guild, no_sleep=no_sleep)
    assert any("Community is not enabled" in w for w in plan.warnings)
    assert all(r.status == "done" for r in results)
    state = await guild.snapshot()
    assert state.channel("bug-reports").type == "text"
    assert not any(c.kind == "create_tag" for c in plan.changes)
    # After enabling Community, the text channel is reported, not silently converted.
    guild.community = True
    again = plan_changes(spec, await guild.snapshot())
    assert any('"#bug-reports" exists as a text channel' in w for w in again.warnings)


async def test_missing_bot_permissions_block_the_plan(spec):
    guild = FakeGuild(bot_permissions=frozenset({"manage_roles", "manage_channels", "view_channel"}))
    plan = plan_changes(spec, await guild.snapshot())
    assert plan.blockers and "kick_members" in plan.missing_bot_permissions
    full = FakeGuild(bot_permissions=ALL_FLAGS)
    assert any("Administrator" in w for w in plan_changes(spec, await full.snapshot()).warnings)


async def test_roles_above_the_bot_are_not_touched(spec):
    guild = FakeGuild()
    guild.roles.append(RoleState(id=10, name="Founder", position=60, permissions=frozenset()))
    plan = plan_changes(spec, await guild.snapshot())
    assert any('"Founder" is above the bot' in w for w in plan.warnings)
    assert not any(c.target == "Founder" for c in plan.changes)


async def test_failures_are_recorded_and_the_run_continues(spec, no_sleep, tmp_path):
    guild = FakeGuild()
    guild.fail["create_channel:welcome"] = ApiError("Missing Permissions", missing_permissions=True)
    guild.fail["create_role:Member"] = ApiError("Service unavailable", retryable=True)
    journal = tmp_path / "apply.jsonl"
    plan, results = await apply(spec, guild, no_sleep=no_sleep, journal=journal)
    failed = [r for r in results if r.status == "failed"]
    assert [r.change.key for r in failed] == ["create_channel:welcome"]
    assert "missing a permission" in failed[0].message
    member = next(r for r in results if r.change.key == "create_role:Member")
    assert member.status == "done", "temporary errors are retried"
    lines = [json.loads(line) for line in journal.read_text(encoding="utf-8").splitlines()]
    assert len(lines) == len(results)
    # Resume: the next run only creates what failed.
    retry = plan_changes(spec, await guild.snapshot())
    assert [c.key for c in retry.changes] == ["create_channel:welcome"]


async def test_channel_failure_when_category_failed(spec, no_sleep):
    guild = FakeGuild()
    guild.fail["create_category:TEAM"] = ApiError("nope")
    plan, results = await apply(spec, guild, no_sleep=no_sleep)
    team_channels = [r for r in results if r.change.kind == "create_channel" and r.change.data["category"].name == "TEAM"]
    assert team_channels and all(r.status == "failed" for r in team_channels)
    assert all("category" in r.message for r in team_channels)
    state = await guild.snapshot()
    assert state.channel("moderation") is None, "private channels are never created outside their private category"


async def test_export_contains_readable_config_and_disclaimer(spec, no_sleep, tmp_path):
    guild = FakeGuild()
    await apply(spec, guild, no_sleep=no_sleep)
    path = export_snapshot(await guild.snapshot(), tmp_path)
    data = json.loads(path.read_text(encoding="utf-8"))
    assert data["_note"] == DISCLAIMER and "not a complete backup" in DISCLAIMER
    roles = {r["name"]: r for r in data["roles"]}
    assert "kick_members" in roles["Moderator"]["permissions"]
    channels = {c["name"]: c for c in data["channels"]}
    assert channels["moderation"]["category"] == "TEAM"
    assert "view_channel" in channels["moderation"]["overwrites"]["@everyone"]["deny"]
    assert channels["bug-reports"]["tags"][0] == "Open"


async def test_verify_detects_weakened_private_channel(spec, no_sleep):
    guild = FakeGuild()
    await apply(spec, guild, no_sleep=no_sleep)
    mod = next(c for c in guild.channels if c.name == "moderation")
    mod.overwrites["@everyone"] = Overwrite()
    ok, problems = verify(spec, await guild.snapshot())
    assert any("#moderation" in p and "hidden" in p for p in problems)


@pytest.mark.parametrize("forbidden", ["administrator"])
async def test_verify_flags_admin_role(spec, no_sleep, forbidden):
    guild = FakeGuild()
    await apply(spec, guild, no_sleep=no_sleep)
    next(r for r in guild.roles if r.name == "Developer").permissions = frozenset({forbidden})
    _, problems = verify(spec, await guild.snapshot())
    assert any("Administrator permission" in p for p in problems)
