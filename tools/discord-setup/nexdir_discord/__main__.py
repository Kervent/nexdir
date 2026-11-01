import sys

from .cli import main

if __name__ == "__main__":
    # Windows consoles may not default to UTF-8; the menu uses symbols like ✔ and box lines.
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    sys.exit(main())
