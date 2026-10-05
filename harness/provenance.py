"""Source provenance: which commit of VulcanBench produced a run.

A sweep can run for days while main keeps moving. Every run summary records
the commit its harness code and its task definitions came from, and suite runs
refuse to start from a checkout with uncommitted changes to scoring-relevant
paths, so a published number always traces back to one commit. The workflow
around this (tagged run worktrees, frozen suites) is in docs/HOW_WE_WORK.md.

Provenance is read once per process: a sweep's ``vulcanbench run`` invocation
imports its code once, so the commit it ran is the one present at start.
"""

from __future__ import annotations

import functools
import os
import subprocess
from dataclasses import dataclass
from pathlib import Path
from typing import Any

#: Paths whose contents can change what a run is asked to do or how it is
#: scored. Edits elsewhere (docs, cards, dashboard) never block a sweep.
SCORING_PATHS = ("harness", "tasks", "sandbox", "vulcanbench.toml", "pyproject.toml", "uv.lock")

#: Escape hatch for a deliberate dirty run (an experiment, never a published
#: number). The run still records ``dirty: true`` and the paths involved.
ALLOW_DIRTY_ENV = "VULCANBENCH_ALLOW_DIRTY"

#: Cap on recorded dirty paths, so a stray untracked tree cannot bloat summaries.
_MAX_DIRTY_PATHS = 50

_HARNESS_DIR = Path(__file__).resolve().parent


class DirtyTreeError(RuntimeError):
    """A suite run was asked to start from uncommitted scoring-relevant changes."""


def _git(args: list[str], cwd: Path, *, strip: bool = True) -> str | None:
    try:
        out = subprocess.run(
            ["git", *args], cwd=cwd, capture_output=True, text=True, timeout=30, check=False
        )
    except (OSError, subprocess.TimeoutExpired):
        return None
    if out.returncode != 0:
        return None
    # Porcelain status lines start with a status column that may be a space.
    return out.stdout.strip() if strip else out.stdout


@dataclass(frozen=True)
class CheckoutState:
    """One git checkout's commit and its uncommitted scoring-relevant paths."""

    root: str
    commit: str | None
    describe: str | None
    dirty_paths: tuple[str, ...]

    @property
    def dirty(self) -> bool:
        return bool(self.dirty_paths)

    def as_summary(self) -> dict[str, Any]:
        return {
            "root": self.root,
            "commit": self.commit,
            "describe": self.describe,
            "dirty": self.dirty,
            "dirty_paths": list(self.dirty_paths),
        }


def _checkout_state(path: Path) -> CheckoutState | None:
    root = _git(["rev-parse", "--show-toplevel"], path)
    if not root:
        return None
    top = Path(root)
    status = _git(["status", "--porcelain", "--", *SCORING_PATHS], top, strip=False) or ""
    dirty = tuple(line[3:] for line in status.splitlines() if line.strip())
    return CheckoutState(
        root=root,
        commit=_git(["rev-parse", "HEAD"], top),
        describe=_git(["describe", "--tags", "--always", "--dirty"], top),
        dirty_paths=dirty[:_MAX_DIRTY_PATHS],
    )


@dataclass(frozen=True)
class SourceProvenance:
    """Where the code and the tasks of a run came from.

    ``harness`` is the checkout the imported ``harness`` package lives in.
    ``tasks`` is the checkout of the working directory (task roots resolve
    relative to it), recorded only when it differs: a sibling suite repo, or
    a run worktree driven by another checkout's virtualenv.
    """

    harness: CheckoutState | None
    tasks: CheckoutState | None

    @property
    def dirty(self) -> bool:
        return any(c is not None and c.dirty for c in (self.harness, self.tasks))

    def dirty_paths(self) -> list[str]:
        out: list[str] = []
        for checkout in (self.harness, self.tasks):
            if checkout is not None:
                out.extend(f"{checkout.root}/{p}" for p in checkout.dirty_paths)
        return out

    def as_summary(self) -> dict[str, Any]:
        return {
            "harness": self.harness.as_summary() if self.harness else None,
            "tasks": self.tasks.as_summary() if self.tasks else None,
            "dirty": self.dirty,
            "allow_dirty": allow_dirty_from_env(),
        }


@functools.lru_cache(maxsize=8)
def _source_provenance(cwd: str) -> SourceProvenance:
    harness = _checkout_state(_HARNESS_DIR)
    here = _checkout_state(Path(cwd))
    tasks = here if here is not None and (harness is None or here.root != harness.root) else None
    return SourceProvenance(harness=harness, tasks=tasks)


def source_provenance(cwd: Path | None = None) -> SourceProvenance:
    """Provenance for runs launched from ``cwd`` (default: the working directory)."""
    return _source_provenance(str((cwd or Path.cwd()).resolve()))


def clear_cache() -> None:
    _source_provenance.cache_clear()


def allow_dirty_from_env() -> bool:
    return os.environ.get(ALLOW_DIRTY_ENV, "").strip().lower() in {"1", "true", "yes"}


def check_clean_for_suite(*, allow_dirty: bool = False, cwd: Path | None = None) -> None:
    """Refuse a suite run from uncommitted scoring-relevant changes.

    A checkout that is not a git repository (an installed package) has no
    commit to stamp and is not refused.
    """
    if allow_dirty or allow_dirty_from_env():
        return
    prov = source_provenance(cwd)
    if not prov.dirty:
        return
    paths = prov.dirty_paths()
    shown = "\n  ".join(paths[:10]) + ("\n  ..." if len(paths) > 10 else "")
    raise DirtyTreeError(
        "refusing to start a suite run with uncommitted changes to scoring-relevant "
        f"paths:\n  {shown}\n"
        "Run sweeps from a clean run worktree pinned to a tag "
        "(make run-worktree TAG=...; see docs/HOW_WE_WORK.md). For a deliberate "
        f"experiment, pass --allow-dirty or set {ALLOW_DIRTY_ENV}=1; the run "
        "records that it was dirty."
    )
