#!/usr/bin/env python3
"""Re-derive task counts from disk and sync the dataset floors + doc numbers.

A "real" task is a directory under ``tasks/v1/`` with both a ``metadata.json``
that declares ``tests`` and a ``gold_patch.diff`` (the same definition the
dataset guard in ``tests/test_dataset.py`` uses). After you add or remove a task
on disk (and register it in ``suite.json``), run this to keep the composition
floors and the documented counts honest without hand-editing each spot.

Usage:
    python scripts/sync_task_counts.py            # rewrite floors + docs in place
    python scripts/sync_task_counts.py --check     # report drift, exit 1 if any (no writes)

The test floors in ``tests/test_dataset.py`` are treated as a hard ratchet and
must always be found; the prose counts in README/CONTRIBUTING/ARCHITECTURE are
best-effort (a reworded sentence warns rather than fails).
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

# Run from the repo root; resolve relative to this file so it works either way.
ROOT = Path(__file__).resolve().parent.parent
TASKS = ROOT / "tasks" / "v1"


def real_tasks() -> list[dict]:
    out = []
    for d in sorted(TASKS.iterdir()):
        if not d.is_dir() or not (d / "metadata.json").exists():
            continue
        meta = json.loads((d / "metadata.json").read_text())
        if meta.get("tests") and (d / "gold_patch.diff").exists():
            out.append(meta)
    return out


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument(
        "--check",
        action="store_true",
        help="report drift and exit non-zero if anything is out of sync; do not write",
    )
    args = ap.parse_args()

    metas = real_tasks()
    n = len(metas)
    hard = sum(1 for m in metas if m.get("difficulty") == "hard")
    nonloc = sum(1 for m in metas if m.get("task_complexity") != "localized")
    print(f"real={n} hard={hard} non_localized={nonloc}")

    drift: list[str] = []

    def apply(path: Path, subs: list[tuple[str, str]], *, hard_fail: bool) -> None:
        rel = path.relative_to(ROOT)
        text = path.read_text()
        for pat, repl in subs:
            new, k = re.subn(pat, repl, text)
            if not k:
                msg = f"{rel}: pattern not found: {pat!r}"
                if hard_fail:
                    raise SystemExit(f"ERROR {msg}")
                print(f"  warn  {msg}", file=sys.stderr)
                continue
            if new != text:
                drift.append(f"{rel}: {pat!r} -> {repl!r}")
            text = new
        if not args.check:
            path.write_text(text)

    # Hard ratchet: the dataset composition floors must always be present.
    apply(
        ROOT / "tests" / "test_dataset.py",
        [
            (r"M3_MIN_REAL_TASKS = \d+", f"M3_MIN_REAL_TASKS = {n}"),
            (r"MIN_HARD_TASKS = \d+", f"MIN_HARD_TASKS = {hard}"),
            (r"MIN_NON_LOCALIZED_TASKS = \d+", f"MIN_NON_LOCALIZED_TASKS = {nonloc}"),
            (r"grew to \d+ as", f"grew to {n} as"),
        ],
        hard_fail=True,
    )
    # Best-effort prose counts.
    apply(
        ROOT / "README.md",
        [
            (r"— \d+ gold-verified tasks", f"— {n} gold-verified tasks"),
            (r"holds \*\*\d+\*\* gold-verified", f"holds **{n}** gold-verified"),
        ],
        hard_fail=False,
    )
    apply(
        ROOT / "CONTRIBUTING.md",
        [(r"validates all \d+ suite tasks", f"validates all {n} suite tasks")],
        hard_fail=False,
    )
    apply(
        ROOT / "docs" / "ARCHITECTURE.md",
        [(r"# \d+ gold-verified suite tasks", f"# {n} gold-verified suite tasks")],
        hard_fail=False,
    )

    if args.check and drift:
        print("\nout of sync (run without --check to fix):", file=sys.stderr)
        for d in drift:
            print(f"  {d}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
