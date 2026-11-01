"""The console actions must never change the server without explicit confirmation."""

import pytest
from fake import FakeGuild

from nexdir_discord import cli
from nexdir_discord.settings import Credentials


@pytest.fixture
def wired(monkeypatch, tmp_path):
    guild = FakeGuild(name="My Server")

    async def connect():
        return Credentials("token", 1, None), guild

    monkeypatch.setattr(cli, "connect", connect)
    monkeypatch.setattr(cli, "EXPORTS", tmp_path / "exports")
    monkeypatch.setattr(cli, "REPORTS", tmp_path / "reports")
    return guild


def tty(monkeypatch, is_tty: bool, answers: list[str] | None = None):
    monkeypatch.setattr(cli.sys.stdin, "isatty", lambda: is_tty, raising=False)
    it = iter(answers or [])
    monkeypatch.setattr("builtins.input", lambda _prompt="": next(it))


async def test_apply_refuses_without_console_but_exports_first(spec, wired, monkeypatch, tmp_path):
    tty(monkeypatch, False)
    await cli.action_apply(spec)
    assert wired.mutations == []
    assert list((tmp_path / "exports").glob("server-config-*.json")), "configuration is exported before any change"


async def test_apply_requires_the_exact_server_name(spec, wired, monkeypatch):
    tty(monkeypatch, True, ["my server"])  # wrong case
    await cli.action_apply(spec)
    assert wired.mutations == []


async def test_apply_creates_but_leaves_existing_items_unless_allowed(spec, wired, monkeypatch, tmp_path):
    monkeypatch.setattr(cli.asyncio, "sleep", _no_sleep)
    tty(monkeypatch, True, ["My Server", "n"])
    await cli.action_apply(spec)
    state = await wired.snapshot()
    assert state.name == "My Server", "rename of the existing server was declined"
    assert state.role("Founder") and state.channel("bug-reports")
    assert list((tmp_path / "reports").glob("report-*.md")) and list((tmp_path / "reports").glob("apply-*.jsonl"))


async def test_icon_and_texts_and_founder_need_confirmation(spec, wired, monkeypatch, tmp_path):
    tty(monkeypatch, False)
    await cli.action_icon(spec)
    assert wired.icon is None
    assert (tmp_path / "reports" / "icon-preview.png").exists(), "a preview is always produced"
    await cli.action_founder(spec)
    await cli.action_texts(spec)
    assert wired.mutations == []


async def test_founder_goes_only_to_the_owner(spec, wired, monkeypatch):
    monkeypatch.setattr(cli.asyncio, "sleep", _no_sleep)
    tty(monkeypatch, True, ["My Server", "n", "y"])
    await cli.action_apply(spec)
    await cli.action_founder(spec)
    founder = (await wired.snapshot()).role("Founder")
    assert {uid for uid, roles in wired.member_roles.items() if founder.id in roles} == {42}


async def _no_sleep(_s):
    return None
