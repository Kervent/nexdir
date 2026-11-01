"""Credentials from environment variables or an untracked .env file. Tokens are never printed."""

from __future__ import annotations

import os
from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class Credentials:
    token: str
    guild_id: int
    client_id: str | None

    def __repr__(self) -> str:  # never leak the token in logs or tracebacks
        return f"Credentials(token=***, guild_id={self.guild_id}, client_id={self.client_id})"


class SettingsError(ValueError):
    pass


def read_env_file(path: Path) -> dict[str, str]:
    values: dict[str, str] = {}
    if not path.is_file():
        return values
    for line in path.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        key, _, value = line.partition("=")
        values[key.strip()] = value.strip().strip('"').strip("'")
    return values


def load_credentials(env_file: Path) -> Credentials:
    file_values = read_env_file(env_file)

    def get(name: str) -> str:
        return os.environ.get(name) or file_values.get(name, "")

    token = get("DISCORD_BOT_TOKEN")
    guild = get("DISCORD_GUILD_ID")
    problems = []
    if not token or token.startswith("your-"):
        problems.append("DISCORD_BOT_TOKEN is not set")
    if not guild.isdigit():
        problems.append("DISCORD_GUILD_ID must be the numeric server ID")
    if problems:
        raise SettingsError("; ".join(problems) + f". Set them as environment variables or in {env_file} (see .env.example).")
    client_id = get("DISCORD_CLIENT_ID") or None
    return Credentials(token=token, guild_id=int(guild), client_id=client_id)


def mask(text: str, secret: str) -> str:
    return text.replace(secret, "***") if secret else text
