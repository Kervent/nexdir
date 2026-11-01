import json
import sys
from pathlib import Path

import pytest

TOOL_DIR = Path(__file__).resolve().parent.parent
REPO_ROOT = TOOL_DIR.parent.parent
sys.path.insert(0, str(TOOL_DIR))

from nexdir_notify.model import Settings  # noqa: E402
from nexdir_notify.net import Response  # noqa: E402

WEBHOOK = "https://discord.com/api/webhooks/123456789012345678/" + "A" * 68
ROLE = "234567890123456789"
BETA_ROLE = "345678901234567890"
NEWS_ROLE = "456789012345678901"


@pytest.fixture
def settings():
    return Settings(repository="nexdir-org/nexdir", webhooks={"releases": WEBHOOK})


def release_event(**overrides):
    rel = {
        "tag_name": "v1.2.0",
        "name": "Nexdir 1.2.0",
        "draft": False,
        "prerelease": False,
        "html_url": "https://github.com/nexdir-org/nexdir/releases/tag/v1.2.0",
        "published_at": "2026-11-03T18:30:00Z",
        "body": "## Added\n- Thumbnails in icon view\n- Faster search index\n  - nested detail\n\n## Fixed\n- Copy dialog flicker\n",
        "assets": [
            {"name": "Nexdir-1.2.0-Setup.exe", "browser_download_url": "https://github.com/nexdir-org/nexdir/releases/download/v1.2.0/Nexdir-1.2.0-Setup.exe"},
            {"name": "Nexdir-1.2.0-Portable.zip", "browser_download_url": "https://github.com/nexdir-org/nexdir/releases/download/v1.2.0/Nexdir-1.2.0-Portable.zip"},
            {"name": "SHA256SUMS.txt", "browser_download_url": "https://github.com/nexdir-org/nexdir/releases/download/v1.2.0/SHA256SUMS.txt"},
        ],
    }
    rel.update(overrides)
    return {"action": "published", "release": rel}


class FakeHttp:
    """Records requests; answers GitHub API and Discord webhook calls from simple state."""

    def __init__(self, *, announced=(), artifacts_status=200, discord_statuses=(200,), release=None):
        self.announced = set(announced)
        self.artifacts_status = artifacts_status
        self.discord_statuses = list(discord_statuses)
        self.release = release
        self.requests = []

    @property
    def posts(self):
        return [r for r in self.requests if r[0] == "POST"]

    def __call__(self, method, url, headers, body):
        self.requests.append((method, url, headers, body))
        if "api.github.com" in url:
            if "/actions/artifacts" in url:
                if self.artifacts_status != 200:
                    return Response(self.artifacts_status, {}, b"{}")
                name = url.split("name=")[1]
                arts = [{"name": name, "expired": False}] if name in self.announced else []
                return Response(200, {}, json.dumps({"total_count": len(arts), "artifacts": arts}).encode())
            if "/releases/tags/" in url:
                if self.release is None:
                    return Response(404, {}, b"{}")
                return Response(200, {}, json.dumps(self.release).encode())
            return Response(404, {}, b"{}")
        status = self.discord_statuses.pop(0) if self.discord_statuses else 200
        if status == 429:
            return Response(429, {}, b'{"message": "You are being rate limited.", "retry_after": 1.5}')
        if status != 200:
            return Response(status, {}, b'{"message": "Invalid Form Body"}')
        return Response(200, {}, b'{"id": "999000111222333444"}')


@pytest.fixture
def make_env(tmp_path):
    def make(event_name, event, **extra):
        event_path = tmp_path / "event.json"
        event_path.write_text(json.dumps(event), encoding="utf-8")
        output = tmp_path / "output.txt"
        output.write_text("", encoding="utf-8")
        env = {
            "GITHUB_EVENT_NAME": event_name,
            "GITHUB_EVENT_PATH": str(event_path),
            "GITHUB_OUTPUT": str(output),
            "GITHUB_REPOSITORY": "nexdir-org/nexdir",
            "GITHUB_TOKEN": "ghs_test",
            "DISCORD_WEBHOOK_RELEASES": WEBHOOK,
            "DISCORD_WEBHOOK_GITHUB_UPDATES": WEBHOOK,
            "DISCORD_WEBHOOK_ROADMAP": WEBHOOK,
        }
        env.update(extra)
        return {k: v for k, v in env.items() if v is not None}

    return make


def outputs(env):
    text = Path(env["GITHUB_OUTPUT"]).read_text(encoding="utf-8")
    return dict(line.split("=", 1) for line in text.splitlines() if line)
