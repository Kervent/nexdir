from __future__ import annotations

from dataclasses import dataclass, field

# Channels that receive announcements. Each one has its own webhook secret.
RELEASES = "releases"
GITHUB_UPDATES = "github-updates"
ROADMAP = "roadmap"
CHANNELS = (RELEASES, GITHUB_UPDATES, ROADMAP)

# Only maintainers (triage permission or higher) can add labels, so this label is how
# an issue or pull request gets selected for an announcement.
ANNOUNCE_LABEL = "discord: announce"

# A release whose notes contain this text is never announced.
SKIP_MARKER = "[skip discord]"


@dataclass(frozen=True)
class Announcement:
    key: str  # deduplication key, e.g. "release-v1.2.0"
    channel: str  # one of CHANNELS
    payload: dict  # Discord webhook JSON body
    summary: str  # one line for the workflow log


@dataclass(frozen=True)
class Skip:
    reason: str


@dataclass(frozen=True)
class Settings:
    repository: str  # "owner/name"
    server_url: str = "https://github.com"
    webhooks: dict[str, str] = field(default_factory=dict)  # channel -> webhook URL
    release_role_id: str | None = None  # "Release Notifications"
    beta_role_id: str | None = None  # "Beta Updates"
    news_role_id: str | None = None  # "Development News"
    announce_prereleases: bool = False

    @property
    def repo_url(self) -> str:
        return f"{self.server_url.rstrip('/')}/{self.repository}"
