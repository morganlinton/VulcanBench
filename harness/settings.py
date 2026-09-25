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
