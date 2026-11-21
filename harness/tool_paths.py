"""Locate host-side scoring tools (ruff, bandit) that ship with the harness."""

from __future__ import annotations

import os
import shutil
import sys
from pathlib import Path


def resolve_tool(name: str) -> str | None:
    """Return the path of a host tool, or ``None`` when it is missing.

    ``PATH`` wins, so an operator's explicit choice is kept. Otherwise the
    directory of the running interpreter is tried: ruff and bandit are harness
    dependencies installed beside ``vulcanbench`` in its virtualenv, and a
    launcher that calls ``.venv/bin/vulcanbench`` without activating the venv
    would otherwise score every run's quality and security as null (DailyWork
    v1 pilots, 2026-11-19). ``sys.executable`` is not resolved, so a venv's
    symlinked ``python`` keeps pointing at the venv's ``bin``.
    """
    found = shutil.which(name)
    if found:
        return found
    candidate = Path(sys.executable).parent / name
    if candidate.is_file() and os.access(candidate, os.X_OK):
        return str(candidate)
    return None
