#!/usr/bin/env python3
"""Export publishable Verdict v1 results: metrics only, no item content.

Reads the private item and prediction files and writes the JSON that the
report and the card are built from. Every field here is safe to commit: the
suite's states, questions and answers stay in verdict-v1-items/.
"""

from __future__ import annotations

import argparse
import json
import math
import statistics
import sys
from collections import Counter, defaultdict
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO))

from harness.verdict.items import load_items  # noqa: E402
from harness.verdict.scoring import (  # noqa: E402
    answer_label,
    auroc,
    base_rate_predictions,
    score,
    tuned_thresholds,
)

LENGTH_BUCKET_TOKENS = 4000
HISTOGRAM_STEP = 0.05


def item_counts(items: list[dict]) -> dict:
    counts: dict[str, dict] = {}
    for family in sorted({i["family"] for i in items}):
        rows = [i for i in items if i["family"] == family]
        counts[family] = {
            "total": len(rows),
            "dev": sum(1 for i in rows if i["split"] == "dev"),
            "test": sum(1 for i in rows if i["split"] == "test"),
            "reference": rows[0].get("reference", "verifier"),
            "question_type": rows[0]["question"]["type"],
        }
    return counts


def diagnostics(items: dict, predictions: list[dict], split: str, threshold: float | None) -> dict:
    """Confusion counts, length curve and the checks that keep families honest."""
    confusion: dict[str, Counter] = defaultdict(Counter)
    by_length: dict[str, list[bool]] = defaultdict(list)
    mean_p_pass = []
    for prediction in predictions:
        item = items.get(prediction["item_id"])
        if item is None or item["split"] != split:
            continue
        top = max(prediction["probs"], key=lambda label: prediction["probs"][label])
        confusion[item["family"]][f"{answer_label(item)}->{top}"] += 1
        if item["family"] == "patch-verdict":
            mean_p_pass.append(prediction["probs"]["true"])
            bucket = min(prediction["input_tokens"] // LENGTH_BUCKET_TOKENS, 2)
            name = f"{bucket * 4}k-{(bucket + 1) * 4}k" if bucket < 2 else "8k+"
            decided = (
                prediction["probs"]["true"] >= threshold if threshold is not None else top == "true"
            )
            by_length[name].append(
                {
                    "correct": decided == bool(item["answer"]),
                    "passes": bool(item["answer"]),
                    "p_true": prediction["probs"]["true"],
                }
            )

    # Bin counts, not per-item values: enough to draw the distribution, no item content.
    histogram: dict[str, dict[str, int]] = defaultdict(lambda: {"passes": 0, "fails": 0})
    for prediction in predictions:
        item = items.get(prediction["item_id"])
        if item is None or item["split"] != split or item["family"] != "patch-verdict":
            continue
        edge = math.floor(prediction["probs"]["true"] / HISTOGRAM_STEP) * HISTOGRAM_STEP
        histogram[f"{edge:.2f}"]["passes" if item["answer"] else "fails"] += 1

    quality = [
        i for i in items.values() if i["family"] == "quality-preference" and i["split"] == split
    ]

    def sides(state: str) -> tuple[str, str]:
        head, tail = state.split("## Patch A")[1].split("## Patch B")
        return head, tail

    longer = sum(
        (("A" if len(sides(i["state"])[0]) > len(sides(i["state"])[1]) else "B") == i["answer"])
        for i in quality
    )
    return {
        "confusion": {family: dict(counts) for family, counts in sorted(confusion.items())},
        "patch_verdict_p_true_histogram": {
            "step": HISTOGRAM_STEP,
            "bins": {k: dict(v) for k, v in sorted(histogram.items())},
        },
        "patch_verdict_mean_p_pass": statistics.mean(mean_p_pass) if mean_p_pass else None,
        # Accuracy alone is unreadable here: the share of patches that pass rises
        # with size, so the majority baseline moves with the bucket.
        "patch_verdict_by_input_tokens": {
            name: {
                "n": len(rows),
                "accuracy_at_threshold": sum(r["correct"] for r in rows) / len(rows),
                "pass_rate": sum(r["passes"] for r in rows) / len(rows),
                "majority_baseline": max(
                    sum(r["passes"] for r in rows), len(rows) - sum(r["passes"] for r in rows)
                )
                / len(rows),
                "auroc": auroc([r["p_true"] for r in rows], [r["passes"] for r in rows]),
            }
            for name, rows in sorted(by_length.items())
        },
        "quality_preference_longer_patch_baseline": longer / len(quality) if quality else None,
        "quality_preference_answer_balance": dict(Counter(i["answer"] for i in quality)),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--items", type=Path, default=REPO / "verdict-v1-items" / "items.jsonl")
    parser.add_argument(
        "--predictions",
        type=Path,
        default=REPO / "verdict-v1-items" / "predictions-jev-1.13.0.jsonl",
    )
    parser.add_argument(
        "--probe", type=Path, default=REPO / "verdict-v1-items" / "probe-phrasing.json"
    )
    parser.add_argument("--model", default="jev-1.13.0")
    parser.add_argument("--split", default="test")
    parser.add_argument(
        "-o",
        "--out",
        type=Path,
        default=REPO / "docs" / "results" / "verdict-v1-jev-2026-09" / "verdict-v1-jev.json",
    )
    args = parser.parse_args()

    items = load_items(args.items)
    predictions = load_items(args.predictions)
    thresholds = tuned_thresholds(items, predictions)
    by_id = {i["item_id"]: i for i in items}
    served = sorted({p["model"] for p in predictions})
    latencies = sorted(p["latency_ms"] for p in predictions if p.get("latency_ms") is not None)

    payload = {
        "suite": "verdict-v1",
        "suite_name": "VulcanBench Verdict v1",
        "model": args.model,
        "served_models": served,
        "split": args.split,
        "item_counts": item_counts(items),
        "source_patches": len(
            {i["source"].get("patch_sha256") for i in items if i["source"].get("patch_sha256")}
        ),
        "results": {
            "majority_floor": score(items, base_rate_predictions(items, args.split), args.split),
            args.model: score(items, predictions, args.split, thresholds),
        },
        "thresholds": {
            "values": thresholds,
            "note": (
                "Yes/no cutoffs fitted on the development split, because Jev's "
                "probabilities never cross 0.5. accuracy is the 0.5 decision, "
                "accuracy_at_threshold uses these, auroc needs neither."
            ),
        },
        "diagnostics": diagnostics(by_id, predictions, args.split, thresholds.get("patch-verdict")),
        "run": {
            "items_queried": len(predictions),
            "failures": 0,
            "retries": sum(p.get("attempts", 1) > 1 for p in predictions),
            "total_cost_usd": sum(p.get("cost_usd") or 0 for p in predictions),
            "latency_ms_p50": latencies[len(latencies) // 2] if latencies else None,
            "latency_ms_p95": latencies[int(len(latencies) * 0.95)] if latencies else None,
            "latency_note": "wall clock from the operator's machine in California; includes the network round trip",
        },
    }
    if args.probe.exists():
        payload["phrasing_probe"] = json.loads(args.probe.read_text())

    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n")
    print(f"wrote {args.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
