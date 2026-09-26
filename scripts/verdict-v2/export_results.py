#!/usr/bin/env python3
"""Export publishable Verdict v2 results: metrics only, no item content.

Reads the private item and prediction files and writes the JSON that the
report, the card and the site page are built from. Every field is safe to
commit: states, questions and answers stay in verdict-v2-items/.

    python scripts/verdict-v2/export_results.py
"""

from __future__ import annotations

import argparse
import json
import statistics
import sys
from collections import defaultdict
from pathlib import Path
from typing import Any

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO))

from harness.verdict.v2.gate import option_text_shortcuts, shortcut_skills  # noqa: E402
from harness.verdict.v2.items import load_items  # noqa: E402
from harness.verdict.v2.registry import BY_ID, FAMILIES  # noqa: E402
from harness.verdict.v2.scoring import item_rows, score, separated  # noqa: E402

ITEMS = REPO / "verdict-v2-items"
OUT = REPO / "docs" / "results" / "verdict-v2-2026-09"
JEV_LABEL = "jev-1.13.0"
REFERENCE_LABEL = "gpt-6-astra-high (reference)"


def load_predictions(path: Path) -> tuple[list[dict[str, Any]], int]:
    rows = [json.loads(line) for line in path.read_text().splitlines() if line.strip()]
    # A reference answer that used a tool did not see the same inputs as Jev.
    kept = [r for r in rows if not r.get("tool_calls")]
    return kept, len(rows) - len(kept)


def probability_ranges(
    items: dict[str, dict[str, Any]], predictions: list[dict[str, Any]]
) -> dict[str, dict[str, float]]:
    """For yes/no families: the range of stated p(true), the check v1 needed."""
    by_family: dict[str, list[float]] = defaultdict(list)
    for p in predictions:
        item = items.get(p["item_id"])
        if item and item["split"] == "test" and item["question"]["type"] == "noul":
            by_family[item["family"]].append(float(p["probs"].get("true", 0.0)))
    return {
        family: {
            "min": min(values),
            "median": statistics.median(values),
            "max": max(values),
            "share_above_half": sum(v > 0.5 for v in values) / len(values),
        }
        for family, values in sorted(by_family.items())
    }


def run_stats(predictions: list[dict[str, Any]], items: dict[str, dict[str, Any]]) -> dict:
    test = [p for p in predictions if items.get(p["item_id"], {}).get("split") == "test"]
    latencies = sorted(p["latency_ms"] for p in test if p.get("latency_ms") is not None)
    costs = [p["cost_usd"] for p in test if p.get("cost_usd") is not None]
    return {
        "answered": len(test),
        "latency_ms_p50": latencies[len(latencies) // 2] if latencies else None,
        "latency_ms_p95": latencies[int(0.95 * (len(latencies) - 1))] if latencies else None,
        "total_cost_usd": sum(costs) if costs else None,
        "models_served": sorted({str(p.get("model")) for p in test}),
    }


def shortcut_table(test_items: list[dict[str, Any]]) -> dict[str, dict[str, Any]]:
    with_text = [
        {**i, "shortcuts": {**i.get("shortcuts", {}), **option_text_shortcuts(i)}}
        for i in test_items
    ]
    by_family: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for row in item_rows(with_text, []):
        by_family[row["family"]].append(row)
    out = {}
    for family, rows in sorted(by_family.items()):
        skills = shortcut_skills(rows)
        best = max(skills, key=lambda k: skills[k]) if skills else None
        out[family] = {
            "best_shortcut": best,
            "best_shortcut_skill": skills[best] if best else None,
            "shortcuts_checked": len(skills),
        }
    return out


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--items", type=Path, default=ITEMS / "items.jsonl")
    parser.add_argument("--jev", type=Path, default=ITEMS / "predictions-jev-1.13.0-test.jsonl")
    parser.add_argument(
        "--reference", type=Path, default=ITEMS / "reference-gpt-6-astra-high-test.jsonl"
    )
    parser.add_argument("--samples", type=int, default=2000)
    parser.add_argument("-o", "--out", type=Path, default=OUT / "verdict-v2-jev.json")
    args = parser.parse_args()

    items = load_items(args.items)
    by_id = {i["item_id"]: i for i in items}
    test_items = [i for i in items if i["split"] == "test"]
    manifest = json.loads((OUT / "freeze-manifest.json").read_text())

    jev, _ = load_predictions(args.jev)
    reference, reference_tool_answers = load_predictions(args.reference)
    results = {
        JEV_LABEL: score(items, jev, split="test", samples=args.samples),
        REFERENCE_LABEL: score(items, reference, split="test", samples=args.samples),
    }
    families_meta = {}
    for family in FAMILIES:
        rows = [i for i in test_items if i["family"] == family.family_id]
        families_meta[family.family_id] = {
            "pillar": family.pillar,
            "area": family.area,
            "question_type": family.question_type,
            "reference": family.reference,
            "summary": family.summary,
            "test_items": len(rows),
            "test_source_units": len({i["source_unit"] for i in rows}),
        }
    export = {
        "suite": "verdict-v2",
        "suite_name": "VulcanBench Verdict v2",
        "split": "test",
        "freeze": {
            "items_sha256": manifest["items_sha256"],
            "git_commit": manifest["git_commit"],
            "seed": manifest["seed"],
            "items_total": manifest["items"],
        },
        "rows": {
            JEV_LABEL: {"role": "subject"},
            REFERENCE_LABEL: {
                "role": "reference",
                "note": "Shows each family is answerable from the same inputs; not a leaderboard entry.",
                "tool_answers_dropped": reference_tool_answers,
            },
        },
        "families": families_meta,
        "pillars": {
            p: [f for f, m in families_meta.items() if m["pillar"] == p]
            for p in sorted({BY_ID[f].pillar for f in BY_ID})
        },
        "shortcuts": shortcut_table(test_items),
        "results": results,
        "separated": {
            index: separated(results[JEV_LABEL], results[REFERENCE_LABEL], index)
            for index in ("verdict_index", "software_index", "general_index")
        },
        "probability_ranges": {
            JEV_LABEL: probability_ranges(by_id, jev),
            REFERENCE_LABEL: probability_ranges(by_id, reference),
        },
        "run": {JEV_LABEL: run_stats(jev, by_id), REFERENCE_LABEL: run_stats(reference, by_id)},
        "notes": [
            "Skill is 100 * (accuracy - floor) / (1 - floor); floor is the best of majority "
            "position, uniform guess and the family's shortcut baselines on the test split.",
            "Intervals are 95% bootstrap intervals resampling source units.",
            "The admission gate was amended after the pilot (DECISIONS 2026-09-25): the "
            "reference only has to reach skill 40, and a family is too easy when Jev reaches 90.",
            "Jev latency is wall clock from the operator's machine in California and includes "
            "the network round trip; indicative only.",
        ],
    }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(export, indent=2) + "\n")
    for label, result in results.items():
        index = result["indices"]["verdict_index"]
        print(f"{label:32} Verdict Index {index['value']:.1f}  95% {index['ci95']}")
    print(f"wrote {args.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
