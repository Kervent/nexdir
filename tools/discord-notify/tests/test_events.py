import dataclasses
import json

import pytest
from conftest import BETA_ROLE, NEWS_ROLE, ROLE, release_event

from nexdir_notify import events
from nexdir_notify.model import ANNOUNCE_LABEL, GITHUB_UPDATES, RELEASES, ROADMAP, Announcement, Skip


def text_of(payload) -> str:
    return json.dumps(payload, ensure_ascii=False)


# --- releases ---------------------------------------------------------------------------


def test_stable_release_contains_required_information(settings):
    result = events.build("release", release_event(), settings)
    assert isinstance(result, Announcement)
    assert result.channel == RELEASES and result.key == "release-v1.2.0"
    embed = result.payload["embeds"][0]
    assert embed["title"] == "Nexdir 1.2.0 is out"
    assert embed["url"].endswith("/releases/tag/v1.2.0")
    fields = {f["name"]: f["value"] for f in embed["fields"]}
    assert fields["Version"] == "1.2.0"
    assert fields["Released"] == "3 November 2026"
    assert "Nexdir-1.2.0-Setup.exe" in fields["Download"] and "Portable" in fields["Download"]
    assert "SHA256SUMS.txt" in fields["Download"]
    assert "docs/ROADMAP.md" in fields["Links"]
    assert "• Thumbnails in icon view" in embed["description"]
    assert "• Copy dialog flicker" in embed["description"]
    assert "nested detail" not in embed["description"]
    assert "pre-release" not in text_of(result.payload).lower()


def test_release_without_role_mentions_nobody(settings):
    payload = events.build("release", release_event(), settings).payload
    assert "content" not in payload
    assert payload["allowed_mentions"] == {"parse": []}


def test_release_role_mention_is_opt_in_and_never_everyone(settings):
    s = dataclasses.replace(settings, release_role_id=ROLE)
    payload = events.build("release", release_event(), s).payload
    assert payload["content"] == f"<@&{ROLE}>"
    assert payload["allowed_mentions"] == {"parse": [], "roles": [ROLE]}
    assert "everyone" not in text_of(payload) and "@here" not in text_of(payload)


def test_release_without_assets_links_to_release_page(settings):
    embed = events.build("release", release_event(assets=[]), settings).payload["embeds"][0]
    download = next(f["value"] for f in embed["fields"] if f["name"] == "Download")
    assert download.startswith("[Download from GitHub](https://github.com/nexdir-org/nexdir/releases/tag/v1.2.0")


def test_prerelease_is_skipped_by_default(settings):
    result = events.build("release", release_event(tag_name="v1.3.0-beta.1", prerelease=True), settings)
    assert isinstance(result, Skip) and "pre-release" in result.reason


@pytest.mark.parametrize("flag", [True, False])
def test_prerelease_is_never_presented_as_stable(settings, flag):
    # Even if the GitHub checkbox is missing, a "-beta" version is a pre-release.
    s = dataclasses.replace(settings, announce_prereleases=True, release_role_id=ROLE, beta_role_id=BETA_ROLE)
    result = events.build("release", release_event(tag_name="v1.3.0-beta.1", prerelease=flag), s)
    embed = result.payload["embeds"][0]
    assert embed["title"] == "Nexdir 1.3.0-beta.1 pre-release"
    assert "not a stable version" in embed["description"]
    assert result.payload["allowed_mentions"]["roles"] == [BETA_ROLE]  # never the release role


def test_github_prerelease_flag_with_plain_version_is_prerelease(settings):
    result = events.build("release", release_event(prerelease=True), settings)
    assert isinstance(result, Skip)


@pytest.mark.parametrize(
    "overrides, reason",
    [
        ({"draft": True}, "draft"),
        ({"tag_name": "nightly"}, "not a version tag"),
        ({"tag_name": "test-build"}, "not a version tag"),
        ({"body": "Internal test build [skip discord]"}, "[skip discord]"),
    ],
)
def test_releases_that_are_never_announced(settings, overrides, reason):
    result = events.build("release", release_event(**overrides), settings)
    assert isinstance(result, Skip) and reason in result.reason


@pytest.mark.parametrize("action", ["created", "edited", "prereleased", "released", "deleted"])
def test_only_the_published_action_announces(settings, action):
    event = release_event()
    event["action"] = action
    assert isinstance(events.build("release", event, settings), Skip)


def test_release_notes_without_bullets_use_first_paragraph(settings):
    result = events.build("release", release_event(body="Bug fix release.\n\nDetails below."), settings)
    assert "Bug fix release." in result.payload["embeds"][0]["description"]


def test_long_release_notes_stay_within_discord_limits(settings):
    body = "\n".join(f"- {'x' * 400} {i}" for i in range(50))
    payload = events.build("release", release_event(body=body), settings).payload
    embed = payload["embeds"][0]
    assert len(embed["description"]) <= 4096
    assert all(len(f["value"]) <= 1024 for f in embed["fields"])
    assert "…and 42 more" in embed["description"]


# --- issues -----------------------------------------------------------------------------


def issue_event(label=ANNOUNCE_LABEL, **issue):
    data = {
        "number": 42,
        "title": "Add Portuguese translation",
        "state": "open",
        "html_url": "https://github.com/nexdir-org/nexdir/issues/42",
        "user": {"login": "octo"},
        "labels": [{"name": ANNOUNCE_LABEL}, {"name": "help wanted"}],
    }
    data.update(issue)
    return {"action": "labeled", "label": {"name": label}, "issue": data}


def test_issue_with_announce_label(settings):
    result = events.build("issues", issue_event(), settings)
    assert isinstance(result, Announcement)
    assert result.channel == GITHUB_UPDATES and result.key == "issue-42"
    embed = result.payload["embeds"][0]
    assert embed["title"] == "#42 Add Portuguese translation"
    assert "Help wanted" in embed["description"]
    assert result.payload["allowed_mentions"] == {"parse": []} and "content" not in result.payload


def test_issue_title_from_anyone_cannot_ping_or_inject_links(settings):
    hostile = "@everyone [Free download](https://evil.example) <@&1> **now**"
    embed = events.build("issues", issue_event(title=hostile, user={"login": "@here"}), settings).payload["embeds"][0]
    blob = text_of(embed)
    assert "@everyone" not in blob and "@here" not in blob
    assert "[Free download](" not in blob and "<@&1>" not in blob


@pytest.mark.parametrize(
    "event",
    [
        issue_event(label="bug"),
        issue_event(state="closed"),
        issue_event(pull_request={"url": "x"}),
        {**issue_event(), "action": "opened"},
    ],
)
def test_issues_that_are_not_announced(settings, event):
    assert isinstance(events.build("issues", event, settings), Skip)


# --- pull requests ----------------------------------------------------------------------


def pr_event(action="closed", merged=True, labels=(ANNOUNCE_LABEL,), label=None):
    event = {
        "action": action,
        "pull_request": {
            "number": 7,
            "title": "Thumbnails in icon view",
            "merged": merged,
            "html_url": "https://github.com/nexdir-org/nexdir/pull/7",
            "user": {"login": "contributor"},
            "base": {"ref": "main"},
            "labels": [{"name": n} for n in labels],
        },
    }
    if label:
        event["label"] = {"name": label}
    return event


def test_merged_labelled_pr_is_announced(settings):
    result = events.build("pull_request_target", pr_event(), settings)
    assert isinstance(result, Announcement) and result.key == "pr-7" and result.channel == GITHUB_UPDATES
    assert "Merged: #7" in result.payload["embeds"][0]["title"]


def test_label_added_after_merge_announces(settings):
    assert isinstance(events.build("pull_request_target", pr_event("labeled", label=ANNOUNCE_LABEL), settings), Announcement)


@pytest.mark.parametrize(
    "event",
    [
        pr_event(merged=False),
        pr_event(labels=()),
        pr_event("labeled", label="docs"),
        pr_event("labeled", merged=False, label=ANNOUNCE_LABEL),
        pr_event("opened"),
    ],
)
def test_prs_that_are_not_announced(settings, event):
    assert isinstance(events.build("pull_request_target", event, settings), Skip)


# --- milestones and roadmap ---------------------------------------------------------------


def test_closed_milestone_goes_to_roadmap_with_optional_news_role(settings):
    s = dataclasses.replace(settings, news_role_id=NEWS_ROLE)
    event = {"action": "closed", "milestone": {"number": 3, "title": "v0.4", "closed_issues": 12, "description": "Thumbnails"}}
    result = events.build("milestone", event, s)
    assert result.channel == ROADMAP and result.key == "milestone-3"
    assert "12 issues closed" in result.payload["embeds"][0]["description"]
    assert "docs/ROADMAP.md" in result.payload["embeds"][0]["description"]
    assert result.payload["allowed_mentions"]["roles"] == [NEWS_ROLE]


def test_other_events_are_ignored(settings):
    assert isinstance(events.build("milestone", {"action": "created", "milestone": {"title": "x"}}, settings), Skip)
    assert isinstance(events.build("push", {}, settings), Skip)


def test_roadmap_post(settings):
    result = events.roadmap(settings, note="Planning for 0.5 has started @everyone", key="roadmap-2026-11-01")
    assert result.channel == ROADMAP
    assert "@everyone" not in text_of(result.payload)
