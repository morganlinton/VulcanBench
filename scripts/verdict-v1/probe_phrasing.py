#!/usr/bin/env python3
"""Phrasing probe: the same dev-split patches, the verdict question worded four ways.

Checks whether a model's answer follows the code or the wording. The
inverted variant flips the reference answer, so a model that reads the code
should agree with itself across variants; one that keys on the words will
not. Dev split only, so nothing here touches the published test numbers.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO))

from dotenv import load_dotenv  # noqa: E402

from harness.verdict.items import load_items  # noqa: E402
from harness.verdict.scoring import score  # noqa: E402
from harness.verdict.typesafe_adapter import DEFAULT_MODEL, predict  # noqa: E402

VARIANTS = {
    "original": (
        "This patch makes every hidden target test pass without breaking any existing test.",
        False,
    ),
    "correct": ("This patch is a correct and complete fix for the issue.", False),
    "would-merge": ("A careful maintainer would merge this patch as fixing the issue.", False),
    "inverted": (
        "This patch is broken: it does not fully fix the issue, or it breaks existing behaviour.",
        True,
    ),
}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--items", type=Path, default=REPO / "verdict-v1-items" / "items.jsonl")
    parser.add_argument("--env", type=Path, default=REPO / ".env")
    parser.add_argument("--model", default=DEFAULT_MODEL)
    parser.add_argument(
        "-o", "--out", type=Path, default=REPO / "verdict-v1-items" / "probe-phrasing.json"
    )
    args = parser.parse_args()
    load_dotenv(args.env)

    base = [
        i for i in load_items(args.items) if i["family"] == "patch-verdict" and i["split"] == "dev"
    ]
    results: dict[str, dict] = {}
    tops: dict[str, dict[str, bool]] = {}
    cost = 0.0
    for name, (instructions, inverted) in VARIANTS.items():
        variant = [
            {
                **i,
                "question": {**i["question"], "instructions": instructions},
                "answer": i["answer"] != inverted,
            }
            for i in base
        ]
        predictions = [predict(v, model=args.model) for v in variant]
        cost += sum(p["cost_usd"] or 0 for p in predictions)
        # Fold the inverted variant back to "does the model think the patch passes".
        tops[name] = {p["item_id"]: (p["probs"]["true"] > 0.5) != inverted for p in predictions}
        metrics = score(variant, predictions, split="dev")["families"]["patch-verdict"]
        yes_rate = sum(p["probs"]["true"] > 0.5 for p in predictions) / len(predictions)
        mean_p = sum(p["probs"]["true"] for p in predictions) / len(predictions)
        results[name] = {
            "instructions": instructions,
            "inverted": inverted,
            "yes_rate": yes_rate,
            "mean_p_true": mean_p,
            **{k: metrics[k] for k in ("n", "accuracy", "brier", "ece")},
        }
        print(
            f"{name:12s} says-yes {yes_rate:.2f} mean p(yes) {mean_p:.2f} acc {metrics['accuracy']:.3f} brier {metrics['brier']:.3f} ece {metrics['ece']:.3f}",
            flush=True,
        )

    names = list(VARIANTS)
    agreement = {
        f"{a} vs {b}": sum(tops[a][k] == tops[b][k] for k in tops[a]) / len(tops[a])
        for i, a in enumerate(names)
        for b in names[i + 1 :]
    }
    true_rate = sum(i["answer"] for i in base) / len(base)
    print(f"true pass rate in this split: {true_rate:.2f}")
    print(
        "pairwise agreement on 'passes' (folded):", {k: round(v, 3) for k, v in agreement.items()}
    )
    args.out.write_text(
        json.dumps(
            {
                "model": args.model,
                "true_pass_rate": true_rate,
                "variants": results,
                "agreement": agreement,
                "cost_usd": cost,
            },
            indent=2,
        )
    )
    print(f"cost ${cost:.3f}; wrote {args.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
