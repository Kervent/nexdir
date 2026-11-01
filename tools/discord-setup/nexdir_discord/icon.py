"""Server icon: uses the official Nexdir logo from assets/branding (never a separate logo)."""

from __future__ import annotations

import io
from dataclasses import dataclass
from pathlib import Path

from PIL import Image, ImageDraw

MIN_SIZE = 512
MAX_UPLOAD_BYTES = 10 * 1024 * 1024  # Discord's upload limit for server icons
UPLOAD_SIZE = 1024


class IconError(ValueError):
    pass


@dataclass
class IconInfo:
    path: Path
    width: int
    height: int
    file_bytes: int


def find_icon(configured: Path | None, repo_root: Path) -> Path | None:
    """The configured path, else the default location of the official Discord variant."""
    candidates = [configured] if configured else []
    candidates.append(repo_root / "assets" / "branding" / "png" / "nexdir-discord-1024.png")
    return next((p for p in candidates if p and p.is_file()), None)


def inspect_icon(path: Path) -> IconInfo:
    try:
        with Image.open(path) as img:
            width, height = img.size
    except Exception as e:  # noqa: BLE001
        raise IconError(f"{path} is not a readable image: {e}") from e
    if width != height:
        raise IconError(f"The icon must be square, but {path.name} is {width}×{height}.")
    if width < MIN_SIZE:
        raise IconError(f"The icon must be at least {MIN_SIZE}×{MIN_SIZE} pixels, but {path.name} is {width}×{height}.")
    return IconInfo(path, width, height, path.stat().st_size)


def upload_bytes(path: Path) -> bytes:
    """PNG bytes for Discord, scaled to 1024 px if larger and checked against the size limit."""
    inspect_icon(path)
    with Image.open(path) as img:
        img = img.convert("RGBA")
        if img.width > UPLOAD_SIZE:
            img = img.resize((UPLOAD_SIZE, UPLOAD_SIZE), Image.Resampling.LANCZOS)
        buf = io.BytesIO()
        img.save(buf, format="PNG", optimize=True)
    data = buf.getvalue()
    if len(data) > MAX_UPLOAD_BYTES:
        raise IconError("The icon is larger than Discord's 10 MB limit.")
    return data


def circular_preview(path: Path, out: Path, size: int = 256) -> Path:
    """Renders how Discord shows the icon (circle crop) next to the full square."""
    inspect_icon(path)
    with Image.open(path) as img:
        square = img.convert("RGBA").resize((size, size), Image.Resampling.LANCZOS)
    mask = Image.new("L", (size, size), 0)
    ImageDraw.Draw(mask).ellipse((0, 0, size - 1, size - 1), fill=255)
    circle = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    circle.paste(square, (0, 0), mask)
    canvas = Image.new("RGBA", (size * 2 + 48, size + 32), (49, 51, 56, 255))  # Discord dark background
    canvas.paste(square, (16, 16), square)
    canvas.paste(circle, (size + 32, 16), circle)
    out.parent.mkdir(parents=True, exist_ok=True)
    canvas.save(out)
    return out
