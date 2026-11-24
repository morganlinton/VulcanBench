"""A thin wrapper over the operator's ``gh`` login (read calls and clones only).

Every GitHub call the builder makes goes through :func:`gh_json` or
:func:`gh_run`. ``gh`` keeps its own token; nothing here reads, prints or
passes it, and stderr is passed through :func:`harness.redaction.redact`
before it reaches an error message. Failures are classified so the CLI can
exit with a stable code: ``gh_missing`` and ``gh_auth`` (3), ``not_found``
(5), anything else ``gh_failed`` (4).
"""

from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
import time
from pathlib import PurePosixPath
from typing import Any

from harness.redaction import redact

#: Directory names whose files are tests or test fixtures.
TEST_DIRS = frozenset(
    {"test", "tests", "__tests__", "spec", "specs", "testing", "testdata", "test_data"}
)
_TEST_NAME = re.compile(
    r"^(test_.+|conftest\.py|.+(_test|\.test|_spec|\.spec)\.[a-z0-9]+|.+tests?\.(java|kt|cs|swift|scala))$"
)
_TRANSIENT = ("503", "502", "rate limit", "abuse", "secondary")
_AUTH = ("gh auth login", "not logged", "authentication", "HTTP 401", "bad credentials")
_NOT_FOUND = ("Could not resolve to a Repository", "Could not resolve to a PullRequest", "HTTP 404")


class GhError(Exception):
    """A classified ``gh`` failure: ``code`` is gh_missing, gh_auth, not_found or gh_failed."""

    def __init__(self, code: str, message: str) -> None:
        super().__init__(message)
        self.code = code
        self.message = message


def is_test_path(path: str) -> bool:
    """A test file: under a test directory, or named like one (``test_x.py``, ``x_test.go``,
    ``x.test.ts``, ``XTest.java``). Stricter than a substring match, so ``latest.py`` is code."""
    p = PurePosixPath(path.lower())
    return bool(TEST_DIRS.intersection(p.parts[:-1])) or bool(_TEST_NAME.match(p.name))


def gh_env() -> dict[str, str]:
    """``gh`` and git never prompt: a missing login is an error, not a hang."""
    env = dict(os.environ)
    env["GIT_TERMINAL_PROMPT"] = "0"
    env["GH_PROMPT_DISABLED"] = "1"
    env["GH_NO_UPDATE_NOTIFIER"] = "1"
    return env


def _classify(stderr: str) -> GhError:
    text = redact(stderr.strip())
    last = text.splitlines()[-1] if text else "gh failed with no message"
    if any(m.lower() in text.lower() for m in _AUTH):
        return GhError("gh_auth", f"gh is not logged in on this machine: {last}")
    if any(m in text for m in _NOT_FOUND):
        return GhError("not_found", last)
    return GhError("gh_failed", last)


def gh_run(args: list[str], *, timeout: float = 120, retries: int = 3) -> str:
    """Run ``gh <args>`` and return stdout; raise :class:`GhError` on failure."""
    if shutil.which("gh") is None:
        raise GhError("gh_missing", "the GitHub CLI (gh) is not installed or not on PATH")
    for attempt in range(retries):
        try:
            proc = subprocess.run(
                ["gh", *args],
                capture_output=True,
                text=True,
                encoding="utf-8",
                errors="replace",
                check=False,
                timeout=timeout,
                env=gh_env(),
            )
        except subprocess.TimeoutExpired as exc:
            raise GhError("gh_failed", f"gh {args[0]} timed out after {timeout:.0f} s") from exc
        except OSError as exc:
            raise GhError("gh_missing", f"gh could not start: {exc}") from exc
        if proc.returncode == 0:
            return proc.stdout
        transient = any(t in proc.stderr for t in _TRANSIENT)
        if transient and attempt < retries - 1:
            time.sleep(1.5 * (attempt + 1))
            continue
        raise _classify(proc.stderr)
    raise GhError("gh_failed", "gh failed")  # pragma: no cover, the loop always returns or raises


def gh_json(args: list[str], *, timeout: float = 120) -> Any:
    """``gh <args>`` whose stdout is JSON; an empty stdout is ``[]``."""
    out = gh_run(args, timeout=timeout)
    if not out.strip():
        return []
    try:
        return json.loads(out)
    except json.JSONDecodeError as exc:
        raise GhError("gh_failed", f"gh {args[0]} printed something that is not JSON") from exc


def require_login() -> None:
    """Fail fast with ``gh_auth`` when gh has no login (``gh auth status`` never prints the token)."""
    try:
        gh_run(["auth", "status"], timeout=30, retries=1)
    except GhError as exc:
        if exc.code == "gh_failed":
            raise GhError("gh_auth", f"gh is not logged in on this machine: {exc.message}") from exc
        raise
