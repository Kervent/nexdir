from pathlib import Path

import pytest
from fake import FakeGuild
from PIL import Image

from nexdir_discord.executor import Executor
from nexdir_discord.icon import IconError, circular_preview, find_icon, inspect_icon, upload_bytes
from nexdir_discord.planner import plan_changes
from nexdir_discord.settings import Credentials, SettingsError, load_credentials, mask
from nexdir_discord.texts import MAX_MESSAGE, load_templates, plan_texts, publish, render, split_message

TOOL_DIR = Path(__file__).resolve().parent.parent
REPO_ROOT = TOOL_DIR.parent.parent


# --- texts ----------------------------------------------------------------------------


def test_every_configured_template_exists_and_renders(spec):
    templates = load_templates(TOOL_DIR / "templates")
    for _, ch in spec.channels():
        if ch.template:
            assert ch.template in templates, ch.template
            text = render(templates[ch.template], spec)
            assert "{links." not in text, f"unresolved placeholder in {ch.template}"
            assert "@everyone" not in text and "@here" not in text
            if ch.type == "forum":
                assert len(split_message(text, ch.template)) == 1, f"{ch.template} must fit into one forum post"


def test_split_message_respects_limit_and_marks_last_chunk():
    text = "\n\n".join(f"Paragraph {i} " + "x" * 300 for i in range(20))
    chunks = split_message(text, "welcome")
    assert len(chunks) > 1
    assert all(len(c) <= MAX_MESSAGE for c in chunks)
    assert "-# nexdir-setup:welcome:" in chunks[-1] and all("nexdir-setup" not in c for c in chunks[:-1])
    long_line = "y" * 5000
    assert all(len(c) <= MAX_MESSAGE for c in split_message(long_line, "t"))


async def test_texts_are_published_once_then_updated_in_place(spec, no_sleep):
    guild = FakeGuild()
    state = await guild.snapshot()
    await Executor(guild, state, sleep=no_sleep, pause=0).run(plan_changes(spec, state), allow_updates=False)
    state = await guild.snapshot()
    templates = load_templates(TOOL_DIR / "templates")
    guild.calls.clear()

    plans = await plan_texts(spec, state, guild, templates)
    assert guild.mutations == [], "planning texts posts nothing"
    assert {p.status for p in plans} == {"new"}
    for p in plans:
        await publish(p, guild)
    forum = state.channel("bug-reports")
    assert len(guild.forum_posts[forum.id]) == 1
    assert "pin_message" in guild.mutations

    again = await plan_texts(spec, state, guild, templates)
    assert {p.status for p in again} == {"up to date"}
    before = len(guild.mutations)
    for p in again:
        assert (await publish(p, guild)).startswith("skipped")
    assert len(guild.mutations) == before, "no duplicate posts"

    templates["welcome"] = templates["welcome"] + "\nNew line."
    changed = {p.template: p for p in await plan_texts(spec, state, guild, templates)}
    assert changed["welcome"].status == "outdated"
    assert await publish(changed["welcome"], guild) == "updated"
    welcome = state.channel("welcome")
    assert len(guild.messages[welcome.id]) == len(changed["welcome"].chunks)


# --- icon -----------------------------------------------------------------------------


def test_official_logo_is_found_and_valid(spec, tmp_path):
    path = find_icon(spec.icon, REPO_ROOT)
    assert path is not None and path.name == "nexdir-discord-1024.png"
    info = inspect_icon(path)
    assert info.width == info.height >= 512
    data = upload_bytes(path)
    assert data.startswith(b"\x89PNG") and len(data) < 10 * 1024 * 1024
    preview = circular_preview(path, tmp_path / "preview.png")
    with Image.open(preview) as img:
        assert img.size == (256 * 2 + 48, 256 + 32)
        # Corner of the circular version is the background (cropped away).
        assert img.getpixel((256 + 32 + 2, 16 + 2))[:3] == (49, 51, 56)


def test_icon_validation(tmp_path):
    Image.new("RGBA", (600, 400)).save(tmp_path / "wide.png")
    Image.new("RGBA", (256, 256)).save(tmp_path / "small.png")
    (tmp_path / "broken.png").write_bytes(b"not an image")
    for name, message in [("wide.png", "square"), ("small.png", "at least 512"), ("broken.png", "not a readable image")]:
        with pytest.raises(IconError, match=message):
            inspect_icon(tmp_path / name)
    assert find_icon(None, tmp_path) is None, "missing logo is reported, upload can be done later"


async def test_icon_upload_goes_through_edit_server(spec):
    guild = FakeGuild()
    await guild.edit_server(icon=upload_bytes(find_icon(spec.icon, REPO_ROOT)))
    assert guild.icon is not None and (await guild.snapshot()).has_icon


# --- settings -------------------------------------------------------------------------


def test_credentials_from_env_file(tmp_path, monkeypatch):
    for k in ("DISCORD_BOT_TOKEN", "DISCORD_GUILD_ID", "DISCORD_CLIENT_ID"):
        monkeypatch.delenv(k, raising=False)
    env = tmp_path / ".env"
    env.write_text('# comment\nDISCORD_BOT_TOKEN="abc.def.ghi"\nDISCORD_GUILD_ID=123456789012345678\n', encoding="utf-8")
    creds = load_credentials(env)
    assert creds.token == "abc.def.ghi" and creds.guild_id == 123456789012345678
    assert "abc" not in repr(creds), "the token never appears in logs"
    assert mask("error for abc.def.ghi", creds.token) == "error for ***"


def test_placeholder_credentials_are_rejected(tmp_path, monkeypatch):
    for k in ("DISCORD_BOT_TOKEN", "DISCORD_GUILD_ID"):
        monkeypatch.delenv(k, raising=False)
    example = TOOL_DIR / ".env.example"
    with pytest.raises(SettingsError):
        load_credentials(example)
    assert isinstance(Credentials("t", 1, None), Credentials)


def test_env_file_is_ignored_by_git():
    gitignore = (REPO_ROOT / ".gitignore").read_text(encoding="utf-8")
    assert ".env" in gitignore.splitlines() and "!.env.example" in gitignore.splitlines()
