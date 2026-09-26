"""Which lines of the changed files the change itself added.

New-language security scanners count only findings on added lines: the C and
C++ static analyzers report false positives on untouched reference code, and a
whole-file count would charge every run for them alike. The workspace is a git
repository whose HEAD is the task's starting commit, so ``git diff HEAD`` is
exactly the agent's change (staged or not). Outside a git workspace the scope
is unknown and callers fall back to counting every finding.
"""

from __future__ import annotations

import re
import subprocess
from pathlib import Path

_HUNK_RE = re.compile(r"^@@ -\S+ \+(\d+)(?:,(\d+))? @@")


def added_lines(workspace: Path, files: list[str]) -> dict[str, set[int]] | None:
    """Map each changed file to the line numbers the change added, or ``None``."""
    if not files:
        return {}
    try:
        proc = subprocess.run(
            ["git", "diff", "HEAD", "--unified=0", "--no-color", "--no-ext-diff", "--", *files],
            cwd=workspace,
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            timeout=60,
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired):
        return None
    if proc.returncode != 0:
        return None
    added: dict[str, set[int]] = {}
    current: str | None = None
    for line in proc.stdout.splitlines():
        if line.startswith("+++ "):
            target = line[4:]
            current = None if target == "/dev/null" else target.removeprefix("b/")
            continue
        match = _HUNK_RE.match(line)
        if match and current is not None:
            start, count = int(match.group(1)), int(match.group(2) or 1)
            added.setdefault(current, set()).update(range(start, start + count))
    return added


def is_added(scope: dict[str, set[int]] | None, file: str, line: int) -> bool:
    """True when ``scope`` is unknown or ``file:line`` was added by the change."""
    return scope is None or line in scope.get(file, set())
