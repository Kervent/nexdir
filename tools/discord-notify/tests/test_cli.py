import json

import pytest
from conftest import ROLE, WEBHOOK, FakeHttp, outputs, release_event

from nexdir_notify import cli
from nexdir_notify.net import NotifyError, Webhook, is_webhook_url, ledger_name


def run(env, http, **kw):
    lines = []
    code = cli.run(env, lines.append, dry_run=kw.pop("dry_run", False), transport=http, sleep=lambda s: None)
    return code, "\n".join(lines)


def test_release_is_posted_once_and_recorded(make_env):
    env = make_env("release", release_event())
    http = FakeHttp()
    code, log = run(env, http)
    assert code == 0 and len(http.posts) == 1
    method, url, headers, body = http.posts[0]
    assert url == WEBHOOK + "?wait=true"
    assert json.loads(body)["embeds"][0]["title"] == "Nexdir 1.2.0 is out"
    assert outputs(env) == {"posted": "true", "ledger": "discord-announced-release-v1.2.0"}
    assert WEBHOOK not in log


def test_second_run_for_the_same_release_does_not_post(make_env):
    env = make_env("release", release_event())
    http = FakeHttp(announced={ledger_name("release-v1.2.0")})
    code, log = run(env, http)
    assert code == 0 and http.posts == []
    assert "Already announced" in log and outputs(env) == {"posted": "false"}


def test_ledger_error_fails_closed(make_env):
    http = FakeHttp(artifacts_status=500)
    with pytest.raises(NotifyError):
        run(make_env("release", release_event()), http)
    assert http.posts == []


def test_without_token_nothing_is_posted(make_env):
    http = FakeHttp()
    with pytest.raises(NotifyError, match="GITHUB_TOKEN"):
        run(make_env("release", release_event(), GITHUB_TOKEN=None), http)
    assert http.posts == []


def test_missing_webhook_is_a_quiet_skip(make_env):
    env = make_env("release", release_event(), DISCORD_WEBHOOK_RELEASES=None)
    http = FakeHttp()
    code, log = run(env, http)
    assert code == 0 and http.requests == []
    assert "DISCORD_WEBHOOK_RELEASES" in log


def test_skipped_event_makes_no_requests(make_env):
    http = FakeHttp()
    code, log = run(make_env("release", release_event(draft=True)), http)
    assert code == 0 and http.requests == [] and "draft" in log


def test_manual_run_is_a_dry_run_by_default(make_env):
    event = {"inputs": {"kind": "release", "tag": "v1.2.0"}}
    http = FakeHttp(release=release_event()["release"])
    code, log = run(make_env("workflow_dispatch", event), http)
    assert code == 0 and http.posts == []
    assert "Dry run" in log and "Nexdir 1.2.0 is out" in log


def test_manual_release_announcement(make_env):
    event = {"inputs": {"kind": "release", "tag": "v1.2.0", "dry_run": "false"}}
    http = FakeHttp(release=release_event()["release"])
    code, _ = run(make_env("workflow_dispatch", event, DISCORD_RELEASE_ROLE_ID=ROLE), http)
    assert code == 0 and len(http.posts) == 1
    assert json.loads(http.posts[0][3])["allowed_mentions"]["roles"] == [ROLE]


def test_manual_release_for_unknown_or_draft_tag(make_env):
    event = {"inputs": {"kind": "release", "tag": "v9.9.9", "dry_run": "false"}}
    http = FakeHttp(release=None)
    code, log = run(make_env("workflow_dispatch", event), http)
    assert code == 0 and http.posts == [] and "no published release" in log


def test_manual_roadmap_post_is_deduplicated_per_day(make_env):
    event = {"inputs": {"kind": "roadmap", "dry_run": "false"}}
    env = make_env("workflow_dispatch", event)
    http = FakeHttp()
    run(env, http)
    assert len(http.posts) == 1
    ledger = outputs(env)["ledger"]
    assert ledger.startswith("discord-announced-roadmap-")
    http2 = FakeHttp(announced={ledger})
    run(make_env("workflow_dispatch", event), http2)
    assert http2.posts == []


def test_rate_limit_is_retried(make_env):
    http = FakeHttp(discord_statuses=[429, 200])
    code, _ = run(make_env("release", release_event()), http)
    assert code == 0 and len(http.posts) == 2


def test_discord_error_is_reported_without_the_webhook_url(make_env):
    http = FakeHttp(discord_statuses=[400])
    with pytest.raises(NotifyError) as err:
        run(make_env("release", release_event()), http)
    assert "HTTP 400" in str(err.value) and "webhooks" not in str(err.value)
    assert len(http.posts) == 1  # client errors are not retried


@pytest.mark.parametrize(
    "url",
    [
        "https://evil.example/api/webhooks/123456789012345678/" + "A" * 68,
        "http://discord.com/api/webhooks/123456789012345678/" + "A" * 68,
        "https://discord.com.evil.example/api/webhooks/123456789012345678/" + "A" * 68,
        "https://discord.com/api/webhooks/1/x",
        "",
    ],
)
def test_only_discord_webhook_urls_are_accepted(url):
    assert not is_webhook_url(url)
    with pytest.raises(NotifyError) as err:
        Webhook(url, transport=FakeHttp())
    if url:
        assert url not in str(err.value)


def test_webhook_url_variants():
    assert is_webhook_url(WEBHOOK)
    assert is_webhook_url(WEBHOOK.replace("discord.com", "discordapp.com"))
    assert is_webhook_url(WEBHOOK.replace("/api/", "/api/v10/"))


def test_invalid_webhook_secret_fails_without_posting(make_env):
    http = FakeHttp()
    env = make_env("release", release_event(), DISCORD_WEBHOOK_RELEASES="https://example.com/hook")
    with pytest.raises(NotifyError):
        run(env, http)
    assert http.posts == []


def test_role_must_be_an_id(make_env):
    with pytest.raises(NotifyError, match="role ID"):
        run(make_env("release", release_event(), DISCORD_RELEASE_ROLE_ID="Release Notifications"), FakeHttp())


def test_prerelease_needs_opt_in(make_env):
    event = release_event(tag_name="v1.3.0-rc.1", prerelease=True)
    http = FakeHttp()
    run(make_env("release", event), http)
    assert http.posts == []
    http = FakeHttp()
    run(make_env("release", event, DISCORD_ANNOUNCE_PRERELEASES="true"), http)
    assert len(http.posts) == 1


def test_preview_command_is_offline(tmp_path, capsys, monkeypatch):
    path = tmp_path / "event.json"
    path.write_text(json.dumps(release_event()), encoding="utf-8")
    monkeypatch.setenv("DISCORD_WEBHOOK_RELEASES", WEBHOOK)
    assert cli.main(["preview", "release", str(path)]) == 0
    out = capsys.readouterr().out
    assert "Nexdir 1.2.0 is out" in out and WEBHOOK not in out
