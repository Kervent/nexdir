import copy
import tomllib
from pathlib import Path

import discord
import pytest

from nexdir_discord.config import ConfigError, parse_config
from nexdir_discord.permissions import (
    ALL_FLAGS,
    BITS,
    BOT,
    EVERYONE,
    POSTING,
    channel_overwrites,
    invite_url,
    permission_bits,
    required_bot_permissions,
)

CONFIG = Path(__file__).resolve().parent.parent / "config" / "server.toml"


def raw():
    with CONFIG.open("rb") as f:
        return tomllib.load(f)


def test_shipped_config_is_valid_and_complete(spec):
    names = [r.name for r in spec.roles]
    assert names[:9] == ["Founder", "Administrator", "Moderator", "Core Developer", "Developer", "Support Team", "Contributor", "Beta Tester", "Member"]
    assert {"Release Notifications", "Beta Updates", "Development News"} <= set(names)
    assert [c.name for c in spec.categories] == ["INFORMATION", "COMMUNITY", "SUPPORT & FEEDBACK", "DEVELOPMENT", "TEAM"]
    channels = {ch.name: ch for _, ch in spec.channels()}
    assert channels["bug-reports"].tags == ("Open", "Investigating", "Confirmed", "Needs Info", "Fixed", "Closed")
    assert channels["feature-requests"].tags == ("Suggestion", "Under Review", "Planned", "In Progress", "Implemented", "Declined")
    assert all(channels[n].type == "forum" for n in ("support", "bug-reports", "feature-requests", "development-ideas"))
    assert spec.icon is not None and spec.icon.name == "nexdir-discord-1024.png"


def test_no_role_ever_gets_administrator(spec):
    assert all("administrator" not in r.permissions for r in spec.roles)
    bad = copy.deepcopy(raw())
    bad["roles"][0]["permissions"].append("administrator")
    with pytest.raises(ConfigError, match="administrator"):
        parse_config(bad, CONFIG.parent)


def test_interest_and_notification_roles_have_no_permissions(spec):
    assert all(not r.permissions for r in spec.roles if r.group in ("interest", "notification"))
    bad = copy.deepcopy(raw())
    interest = next(r for r in bad["roles"] if r["group"] == "interest")
    interest["permissions"] = ["kick_members"]
    with pytest.raises(ConfigError, match="must not carry permissions"):
        parse_config(bad, CONFIG.parent)


@pytest.mark.parametrize(
    "mutate, message",
    [
        (lambda c: c["roles"].append(dict(c["roles"][0])), "duplicate role"),
        (lambda c: c["categories"][0]["channels"][0].update(writers=["Nobody"]), "unknown role"),
        (lambda c: c["categories"][0]["channels"][0].update(name="Bad Name"), "lowercase"),
        (lambda c: c["categories"][-1]["channels"][0].pop("visible_to"), "need 'visible_to'"),
        (lambda c: c["categories"][1]["channels"][0].update(tags=["x"]), "only forum channels"),
        (lambda c: c["roles"][0]["permissions"].append("fly"), "unknown permissions"),
        (lambda c: c["roles"][0].update(color="blue"), "color"),
    ],
)
def test_invalid_configs_are_rejected(mutate, message):
    bad = copy.deepcopy(raw())
    mutate(bad)
    with pytest.raises(ConfigError, match=message):
        parse_config(bad, CONFIG.parent)


def _ch(spec, name):
    return next((c, ch) for c, ch in spec.channels() if ch.name == name)


def test_private_team_channels_follow_the_access_matrix(spec):
    def visible(name, role):
        ow = channel_overwrites(*_ch(spec, name)).normalized()
        return "view_channel" in ow.get(role.lower(), type("O", (), {"allow": frozenset()})).allow

    for name in ("team-general", "developer-internal", "support-internal", "moderation", "staff-applications", "mod-logs"):
        ow = channel_overwrites(*_ch(spec, name)).normalized()
        assert "view_channel" in ow[EVERYONE].deny, name
        assert "view_channel" in ow[BOT].allow, "the bot keeps access to manage the channel"
    assert visible("developer-internal", "Developer") and not visible("developer-internal", "Support Team")
    assert visible("support-internal", "Support Team") and not visible("support-internal", "Developer")
    assert visible("moderation", "Moderator") and not visible("moderation", "Core Developer")
    assert not visible("team-general", "Contributor") and not visible("team-general", "Member")


def test_read_only_channels(spec):
    ow = channel_overwrites(*_ch(spec, "announcements")).normalized()
    assert POSTING <= ow[EVERYONE].deny
    assert "view_channel" in ow[EVERYONE].allow
    assert "send_messages" in ow["core developer"].allow
    assert "moderator" not in ow
    # mod-logs: moderators can read but not write; only writers post.
    logs = channel_overwrites(*_ch(spec, "mod-logs")).normalized()
    assert "view_channel" in logs["moderator"].allow and "send_messages" not in logs["moderator"].allow
    assert "send_messages" in logs["administrator"].allow


def test_forums_let_everyone_post_and_moderators_manage(spec):
    ow = channel_overwrites(*_ch(spec, "bug-reports")).normalized()
    assert "create_public_threads" in ow[EVERYONE].allow
    assert "manage_threads" in ow["support team"].allow


def test_no_channel_overwrite_touches_manage_permissions(spec):
    # Discord rejects Manage Permissions (manage_roles) in a channel overwrite from a bot without
    # Administrator. This failed on the real server for every read-only and private channel.
    for cat, ch in spec.channels():
        for target, o in channel_overwrites(cat, ch).entries.items():
            assert "manage_roles" not in o.allow | o.deny, f"#{ch.name} / {target}"


def test_bot_permissions_cover_everything_it_grants(spec):
    needed = required_bot_permissions(spec)
    assert "administrator" not in needed
    for role in spec.roles:
        assert role.permissions <= needed
    assert {"manage_roles", "manage_channels", "kick_members", "ban_members", "moderate_members"} <= needed
    url = invite_url("123", spec)
    assert url.startswith("https://discord.com/oauth2/authorize?client_id=123&scope=bot&permissions=")
    assert int(url.rsplit("=", 1)[1]) == permission_bits(needed)


def test_permission_bits_match_discord_py():
    for name, bit in BITS.items():
        assert discord.Permissions(**{name: True}).value == 1 << bit, name
    assert set(BITS) <= ALL_FLAGS
    assert ALL_FLAGS == set(discord.Permissions.VALID_FLAGS)
