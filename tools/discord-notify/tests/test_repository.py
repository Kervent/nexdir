"""Guards for the GitHub side: workflow safety properties and labels the notifier relies on.

Text checks only (no YAML library), so they stay dependency-free."""

import json
import re

from conftest import REPO_ROOT

from nexdir_notify.cli import WEBHOOK_ENV
from nexdir_notify.model import ANNOUNCE_LABEL

WORKFLOWS = REPO_ROOT / ".github" / "workflows"
NOTIFY = (WORKFLOWS / "discord-notify.yml").read_text(encoding="utf-8")


def run_blocks(text: str) -> list[str]:
    """Contents of every `run:` step (single line or block)."""
    blocks, lines = [], text.splitlines()
    for i, line in enumerate(lines):
        match = re.match(r"^(\s*)(?:- )?run:\s*(.*)$", line)
        if not match:
            continue
        indent, rest = len(match.group(1)), match.group(2)
        if rest not in ("|", ">", "|-", ">-"):
            blocks.append(rest)
            continue
        body = []
        for nxt in lines[i + 1 :]:
            if nxt.strip() and len(nxt) - len(nxt.lstrip()) <= indent:
                break
            body.append(nxt)
        blocks.append("\n".join(body))
    return blocks


def test_no_untrusted_expressions_inside_shell_scripts():
    # Event data (titles, branch names, inputs) must reach scripts through env vars,
    # never through ${{ }} interpolation, which would allow command injection.
    for path in WORKFLOWS.glob("*.yml"):
        for block in run_blocks(path.read_text(encoding="utf-8")):
            assert "${{" not in block, f"{path.name}: expression inside run: {block!r}"


def test_every_workflow_declares_permissions():
    for path in WORKFLOWS.glob("*.yml"):
        assert re.search(r"^permissions:", path.read_text(encoding="utf-8"), re.M), path.name


def test_actions_are_pinned_to_commit_shas():
    for path in WORKFLOWS.glob("*.yml"):
        for ref in re.findall(r"uses:\s*([^\s#]+)", path.read_text(encoding="utf-8")):
            assert re.search(r"@[0-9a-f]{40}$", ref), f"{path.name}: {ref} is not pinned to a SHA"


def test_notify_workflow_never_runs_pull_request_code():
    assert "pull_request_target" in NOTIFY
    assert "github.event.pull_request.head" not in NOTIFY
    assert not re.search(r"^\s+ref:", NOTIFY, re.M), "checkout must use the base branch"
    assert "persist-credentials: false" in NOTIFY
    assert re.search(r"^permissions: \{\}", NOTIFY, re.M)
    assert "write" not in "".join(re.findall(r"permissions:\n((?:\s{6}.*\n)+)", NOTIFY))


def test_notify_workflow_passes_every_webhook_secret():
    for var in WEBHOOK_ENV.values():
        assert f"{var}: ${{{{ secrets.{var} }}}}" in NOTIFY


def test_announce_label_is_defined_and_used():
    labels = {label["name"] for label in json.loads((REPO_ROOT / ".github" / "labels.json").read_text(encoding="utf-8"))}
    assert ANNOUNCE_LABEL in labels
    assert NOTIFY.count(f"'{ANNOUNCE_LABEL}'") == 2
    for template in (REPO_ROOT / ".github" / "ISSUE_TEMPLATE").glob("*.yml"):
        for line in template.read_text(encoding="utf-8").splitlines():
            if line.startswith("labels:"):
                for name in re.findall(r'"([^"]+)"', line):
                    assert name in labels, f"{template.name} uses unknown label {name}"
