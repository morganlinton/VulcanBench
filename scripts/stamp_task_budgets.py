#!/usr/bin/env python3
"""Stamp explicit per-task agent budgets from the complexity-scaled formula.

Suites that set ``"require_explicit_budgets": true`` in their ``suite.json``
(the Coding Intelligence Index, ``tasks/cii-v1``) fail validation unless every
task carries positive ``agent_hints.suggested_max_steps`` and
``suggested_timeout_s``. This script computes those values from
``harness.task_metadata.complexity_scaled_budgets`` (repo_scale baseline x
task_complexity multiplier) and writes them into each task's ``metadata.json``.

A task may deliberately deviate from the formula (a measured hand-tune): set
``agent_hints.budget_hand_tuned: true`` and the stamper leaves its values alone
(``--check`` reports it as HAND-TUNED, not a mismatch).

Usage::

    python scripts/stamp_task_budgets.py tasks/cii-v1            # stamp all tasks
    python scripts/stamp_task_budgets.py tasks/cii-v1 --check    # verify only (CI)
    python scripts/stamp_task_budgets.py tasks/cii-v1/<task-id>  # one task

``--check`` exits 1 if any stamped value is missing or differs from the formula.
Writing normalizes metadata.json to 2-space-indented JSON with a trailing newline.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from harness.task_metadata import (
    complexity_scaled_budgets,
    repo_scale,
    task_complexity,
)


def _task_dirs(target: Path) -> list[Path]:
    if (target / "metadata.json").is_file():
        return [target]
    return sorted(p.parent for p in target.glob("*/metadata.json"))


def stamp(task_dir: Path, check: bool) -> str:
    """Stamp (or verify) one task; returns 'ok' | 'stamped' | 'hand-tuned' | 'mismatch'."""
    meta_path = task_dir / "metadata.json"
    metadata = json.loads(meta_path.read_text(encoding="utf-8"))
    hints = metadata.get("agent_hints")
    if not isinstance(hints, dict):
        hints = {}
    expected = complexity_scaled_budgets(repo_scale(metadata), task_complexity(metadata))

    if hints.get("budget_hand_tuned") is True:
        return "hand-tuned"

    current = {k: hints.get(k) for k in expected}
    if current == expected:
        return "ok"
    if check:
        return "mismatch"
    hints.update(expected)
    metadata["agent_hints"] = hints
    meta_path.write_text(json.dumps(metadata, indent=2) + "\n", encoding="utf-8")
    return "stamped"


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    p.add_argument("target", type=Path, help="suite dir or one task dir")
    p.add_argument(
        "--check", action="store_true", help="verify stamped budgets match the formula; no writes"
    )
    args = p.parse_args(argv)

    dirs = _task_dirs(args.target)
    if not dirs:
        print(f"no tasks found under {args.target}", file=sys.stderr)
        return 1

    failures = 0
    for task_dir in dirs:
        status = stamp(task_dir, check=args.check)
        metadata = json.loads((task_dir / "metadata.json").read_text(encoding="utf-8"))
        hints = metadata.get("agent_hints") or {}
        detail = (
            f"steps={hints.get('suggested_max_steps')} timeout_s={hints.get('suggested_timeout_s')}"
        )
        print(f"{status:>10}  {task_dir.name}  ({detail})")
        if status == "mismatch":
            failures += 1
    if failures:
        print(f"\n{failures} task(s) diverge from the budget formula", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
