"""HTTP: the Discord webhook and the GitHub API. Uses urllib only; the transport is
injectable so tests never touch the network."""

from __future__ import annotations

import json
import re
import time
import urllib.error
import urllib.parse
import urllib.request
from dataclasses import dataclass
from typing import Callable

USER_AGENT = "nexdir-notify/0.1"
_WEBHOOK = re.compile(r"^https://(?:(?:ptb|canary)\.)?discord(?:app)?\.com/api/(?:v\d+/)?webhooks/\d{15,25}/[A-Za-z0-9_-]{20,}$")
_KEY = re.compile(r"[^A-Za-z0-9._-]+")


@dataclass
class Response:
    status: int
    headers: dict[str, str]
    body: bytes

    def json(self):
        return json.loads(self.body.decode("utf-8") or "null")


Transport = Callable[[str, str, dict[str, str], bytes | None], Response]


class NotifyError(Exception):
    """An error whose message never contains a webhook URL or token."""


def urllib_transport(method: str, url: str, headers: dict[str, str], body: bytes | None) -> Response:
    request = urllib.request.Request(url, data=body, method=method, headers=headers)
    try:
        with urllib.request.urlopen(request, timeout=30) as resp:
            return Response(resp.status, {k.lower(): v for k, v in resp.headers.items()}, resp.read())
    except urllib.error.HTTPError as err:
        return Response(err.code, {k.lower(): v for k, v in err.headers.items()}, err.read())
    except urllib.error.URLError as err:
        raise NotifyError(f"network error: {err.reason}") from None


def is_webhook_url(url: str) -> bool:
    return bool(_WEBHOOK.match(url.strip()))


def ledger_name(key: str) -> str:
    return "discord-announced-" + _KEY.sub("_", key)


class Webhook:
    def __init__(self, url: str, transport: Transport = urllib_transport, sleep: Callable[[float], None] = time.sleep):
        url = url.strip()
        if not is_webhook_url(url):
            # Refuse anything that is not a Discord webhook, so a misconfigured secret can
            # never send data to another server. The value itself is never printed.
            raise NotifyError("the webhook secret is not a Discord webhook URL (https://discord.com/api/webhooks/…)")
        self._url = url
        self._transport = transport
        self._sleep = sleep

    def post(self, payload: dict, attempts: int = 4) -> str | None:
        """Posts once (with retries for rate limits and server errors). Returns the message ID."""
        body = json.dumps(payload).encode("utf-8")
        headers = {"Content-Type": "application/json", "User-Agent": USER_AGENT}
        for attempt in range(1, attempts + 1):
            resp = self._transport("POST", self._url + "?wait=true", headers, body)
            if resp.status in (200, 204):
                try:
                    message_id = (resp.json() or {}).get("id")
                except (ValueError, AttributeError):
                    return None
                return str(message_id) if message_id else None
            if attempt < attempts and (resp.status == 429 or resp.status >= 500):
                self._sleep(_retry_after(resp, attempt))
                continue
            raise NotifyError(f"Discord rejected the message (HTTP {resp.status}){_discord_reason(resp)}")
        raise NotifyError("Discord did not accept the message")  # pragma: no cover


class GitHub:
    def __init__(self, token: str | None, repository: str, api_url: str = "https://api.github.com", transport: Transport = urllib_transport):
        self._token = token
        self._repo = repository
        self._api = api_url.rstrip("/")
        self._transport = transport

    def _get(self, path: str):
        headers = {"Accept": "application/vnd.github+json", "User-Agent": USER_AGENT, "X-GitHub-Api-Version": "2022-11-28"}
        if self._token:
            headers["Authorization"] = f"Bearer {self._token}"
        resp = self._transport("GET", f"{self._api}/repos/{self._repo}/{path}", headers, None)
        if resp.status == 404:
            return None
        if resp.status != 200:
            raise NotifyError(f"GitHub API returned HTTP {resp.status} for {path.split('?')[0]}")
        return resp.json()

    def release_by_tag(self, tag: str) -> dict | None:
        return self._get("releases/tags/" + urllib.parse.quote(tag, safe=""))

    def already_announced(self, key: str) -> bool:
        """The ledger: after each successful post the workflow uploads an empty artifact named
        after the announcement key. Any artifact with that name (even an expired one) means the
        announcement was already made. Errors propagate, so an unknown state never posts."""
        name = ledger_name(key)
        data = self._get("actions/artifacts?per_page=100&name=" + urllib.parse.quote(name, safe=""))
        if data is None:
            raise NotifyError("could not read the announcement ledger (artifacts API returned 404)")
        return any(a.get("name") == name for a in data.get("artifacts") or [])


def _retry_after(resp: Response, attempt: int) -> float:
    seconds = None
    try:
        seconds = float((resp.json() or {}).get("retry_after"))
    except (ValueError, TypeError, AttributeError):
        pass
    if seconds is None:
        try:
            seconds = float(resp.headers.get("retry-after", ""))
        except ValueError:
            seconds = None
    if seconds is None:
        seconds = 2.0**attempt
    return min(max(seconds, 0.5), 30.0)


def _discord_reason(resp: Response) -> str:
    try:
        message = (resp.json() or {}).get("message")
    except (ValueError, AttributeError):
        return ""
    return f": {str(message)[:200]}" if message else ""
