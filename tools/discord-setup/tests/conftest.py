import sys
from pathlib import Path

import pytest

TOOL_DIR = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(TOOL_DIR))
sys.path.insert(0, str(Path(__file__).resolve().parent))

from nexdir_discord.config import load_config  # noqa: E402


@pytest.fixture
def spec():
    return load_config(TOOL_DIR / "config" / "server.toml")


@pytest.fixture
def no_sleep():
    async def sleep(_seconds: float) -> None:
        return None

    return sleep
