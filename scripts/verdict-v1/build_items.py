#!/usr/bin/env python3
"""Build the VulcanBench Verdict v1 item file from finished Frontier v4 runs.

    python scripts/verdict-v1/build_items.py --runs-root ~/dev/VulcanBench \
        -o verdict-v1-items/items.jsonl

The output embeds task issues and patches: it is benchmark data, is
gitignored, and must not be published.
"""

from __future__ import annotations

import argparse
import json
import sys
from collections import Counter
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO))

from harness.verdict.items import (  # noqa: E402
    DEFAULT_MAX_PATCH_CHARS,
    SOURCE_SUITE,
    build_items,
    load_items,
)
from harness.verdict.scoring import base_rate_predictions, score  # noqa: E402

# Completed Frontier v4 sweeps; in-flight (Devin), stopped (Astra rerun) and partial sweeps stay out.
COMPLETED_RUN_DIRS = (
    "runs-effort",
    "runs-astra-cii-v4",
    "runs-effort-terra",
    "runs-effort-luna",
    "runs-effort-gpt55",
    "runs-effort-sol",
    "runs-muse13-contributor-cii-v4-v2",
)


# Judged populations, oldest protocol first; a pair judged twice keeps the newest verdict.
REVIEW_DIRS = tuple(
    f"runs-code-quality-maintenance-v{v}" for v in ("3.4", "3.5", "3.6", "3.6.1", "3.7")
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "--runs-root", type=Path, default=REPO, help="checkout that holds the runs-* directories"
    )
    parser.add_argument(
        "--run-dir",
        action="append",
        help="run directory name; repeatable (default: completed sweeps)",
    )
    parser.add_argument("--tasks-root", type=Path, default=REPO / "tasks" / SOURCE_SUITE)
    parser.add_argument("--max-patch-chars", type=int, default=DEFAULT_MAX_PATCH_CHARS)
    parser.add_argument(
        "-o", "--out", type=Path, default=REPO / "verdict-v1-items" / "items.jsonl"
    )
    args = parser.parse_args()

    roots = [args.runs_root / name for name in (args.run_dir or COMPLETED_RUN_DIRS)]
    missing = [str(r) for r in roots if not r.is_dir()]
    if missing:
        parser.error(f"run directories not found: {', '.join(missing)}")

    reviews = [args.runs_root / name for name in REVIEW_DIRS if (args.runs_root / name).is_dir()]
    items = build_items(roots, args.tasks_root, args.max_patch_chars, reviews)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text("".join(item.to_json() + "\n" for item in items))

    rows = load_items(args.out)
    counts = Counter((r["family"], r["split"]) for r in rows)
    answers = Counter(
        (r["family"], str(r["answer"]))
        for r in rows
        if r["family"] != "fix-localization"  # its options differ per item
    )
    print(f"wrote {len(rows)} items to {args.out}")
    for (family, split), n in sorted(counts.items()):
        print(f"  {family:18s} {split:5s} {n}")
    print("label balance:", dict(sorted(answers.items())))
    floor = score(rows, base_rate_predictions(rows))
    print("base-rate floor (test split):")
    print(
        json.dumps(
            {
                f: {k: round(v, 4) for k, v in m.items() if k in ("n", "accuracy", "brier", "ece")}
                for f, m in floor["families"].items()
            },
            indent=2,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
