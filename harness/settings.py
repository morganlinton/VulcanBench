"""Operator settings from ``vulcanbench.toml`` at the repository root.

The file holds decisions that must hold across every entry point, such as
effort levels that are never allowed to run. Read lazily and cached; a
missing file means no restrictions.
"""

from __future__ import annotations

import tomllib
from functools import lru_cache
from pathlib import Path
from typing import Any

SETTINGS_PATH = Path(__file__).resolve().parents[1] / "vulcanbench.toml"


class BlockedEffortError(ValueError):
    """Raised when a run asks for an effort level the settings file forbids."""


@lru_cache(maxsize=1)
def load_settings(path: Path = SETTINGS_PATH) -> dict[str, Any]:
    if not path.exists():
        return {}
    with path.open("rb") as handle:
        return tomllib.load(handle)


def blocked_efforts(path: Path = SETTINGS_PATH) -> frozenset[str]:
    values = load_settings(path).get("effort", {}).get("blocked", [])
    if not isinstance(values, list) or not all(isinstance(v, str) for v in values):
        raise ValueError(f"{path}: [effort].blocked must be a list of strings")
    return frozenset(v.strip().lower() for v in values)


def check_effort_allowed(effort: str | None, path: Path = SETTINGS_PATH) -> None:
    """Refuse a blocked effort level before any run starts."""
    if effort is None:
        return
    if effort.strip().lower() in blocked_efforts(path):
        raise BlockedEffortError(
            f"effort {effort!r} is blocked by {path.name} ([effort].blocked); "
            "VulcanBench never runs this level"
        )


# ---------------------------------------------------------------------------
# [runner]: the local run GUI (backend/runner). Paths are kept as strings
# here; the backend expands "~" and resolves relative paths against the repo
# root. Every key can be overridden with VULCANBENCH_RUNNER_<KEY> (the
# backend applies those), so this function only reads the file.
# ---------------------------------------------------------------------------

RUNNER_DEFAULTS: dict[str, Any] = {
    "home": "~/vulcanbench-runner",
    "gate_out": "~/vulcanbench-gate",
    "run_worktrees": "../VulcanBench-runs",
    "extra_run_roots": [],
    "first_check_after_s": 180,
}


def runner_settings(path: Path = SETTINGS_PATH) -> dict[str, Any]:
    """The ``[runner]`` table with defaults filled in and types checked."""
    table = load_settings(path).get("runner", {})
    if not isinstance(table, dict):
        raise ValueError(f"{path}: [runner] must be a table")
    merged: dict[str, Any] = dict(RUNNER_DEFAULTS)
    merged.update(table)
    for key in ("home", "gate_out", "run_worktrees"):
        if not isinstance(merged[key], str) or not merged[key]:
            raise ValueError(f"{path}: [runner].{key} must be a non-empty string")
    roots = merged["extra_run_roots"]
    if not isinstance(roots, list) or not all(isinstance(r, str) for r in roots):
        raise ValueError(f"{path}: [runner].extra_run_roots must be a list of strings")
    if isinstance(merged["first_check_after_s"], bool) or not isinstance(
        merged["first_check_after_s"], int
    ):
        raise ValueError(f"{path}: [runner].first_check_after_s must be an integer")
    return merged
