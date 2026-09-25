#!/usr/bin/env python3
"""Score one or more Verdict v1 prediction files and print a results table.

    python scripts/verdict-v1/score_predictions.py verdict-v1-items/predictions-jev-1.13.0.jsonl

Each file is one column, named after its stem after ``predictions-``. The
majority-answer floor is always the first column. Output is JSON (``--json``)
or a Markdown table; neither contains item content.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO))

from harness.verdict.items import load_items  # noqa: E402
from harness.verdict.scoring import base_rate_predictions, score, tuned_thresholds  # noqa: E402

COLUMNS = (
    "n",
    "majority_label_share",
    "accuracy",
    "accuracy_at_threshold",
    "auroc",
    "p_true_min",
    "p_true_max",
    "brier",
    "ece",
    "latency_ms_p50",
    "cost_usd_per_1k",
)


def fmt(value: object) -> str:
    if value is None:
        return "n/a"
    if isinstance(value, float):
        return f"{value:.3f}" if abs(value) < 100 else f"{value:.0f}"
    return str(value)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("predictions", nargs="+", type=Path)
    parser.add_argument("--items", type=Path, default=REPO / "verdict-v1-items" / "items.jsonl")
    parser.add_argument("--split", default="test")
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()

    items = load_items(args.items)
    results = {"majority-floor": score(items, base_rate_predictions(items, args.split), args.split)}
    for path in args.predictions:
        name = path.stem.removeprefix("predictions-")
        predictions = load_items(path)
        results[name] = score(items, predictions, args.split, tuned_thresholds(items, predictions))

    if args.json:
        print(json.dumps(results, indent=2))
        return 0
    families = list(next(iter(results.values()))["families"])
    for family in ["overall", *families]:
        print(f"\n### {family} ({args.split} split)\n")
        print("| model | " + " | ".join(COLUMNS) + " |")
        print("|---|" + "---|" * len(COLUMNS))
        for name, result in results.items():
            metrics = result["overall"] if family == "overall" else result["families"][family]
            print(f"| {name} | " + " | ".join(fmt(metrics.get(c)) for c in COLUMNS) + " |")
    agreement = next(iter(results.values()))["agreement_only_families"]
    print(
        f"\nOverall excludes judge-agreement families: {', '.join(agreement) or 'none'}. "
        "Unanswered items count as wrong.\naccuracy is the 0.5 decision; accuracy_at_threshold "
        "uses a cutoff fitted on the dev split; auroc needs no cutoff at all."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
