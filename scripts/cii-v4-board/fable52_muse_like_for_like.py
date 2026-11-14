"""Code quality and combined score from Muse Spark 1.3 alone, for the Fable 5.2 comparison set.

Claude Fable 5.2 is published from Muse Spark 1.3 and GPT-6.1 Sol (v3.30.2 and
v3.30.3, after Grok 4.6 failed its v3.30 exam); Claude Fable 5.1 and GPT-6 Astra
(v3.4) and Claude Opus 5.5 (v3.15) from Muse Spark 1.3 and Grok 4.6. Sol scores
higher than Muse at every level, so the published columns do not share a
panel. Muse sat on all four rounds with the same pinned binary, rubric, system
prompt and weights, so its reviews alone give a like-for-like comparison. This
script rebuilds each published row from both panels first (and refuses if any
row disagrees with its published Code quality or combined score), then swaps in
Muse's own L1 and L2. Nothing is re-judged.

    python scripts/cii-v4-board/fable52_muse_like_for_like.py \\
        --v34 <v3.4 dir> --v315 <v3.15 dir> --v3302 <v3.30.2 dir> --v3303 <v3.30.3 dir>

The v3.4 and v3.15 directories are in the private backup (archive `judging`).
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import math
import statistics
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
OUTPUT = ROOT / "docs/results/swe-v4-fable52-2026-11/muse-like-for-like.csv"
LEVELS = ("low", "medium", "high", "extra-high", "max")
ROUNDS = {
    "v34": ("code-quality-maintenance-v3.4", {"fable": "fable51", "astra": "astra"}),
    "v315": ("code-quality-maintenance-v3.15", {"opus55": "opus55"}),
    "v3302": ("code-quality-maintenance-v3.30.2", {"fable52": "fable52"}),
    "v3303": ("code-quality-maintenance-v3.30.3", {"fable52": "fable52"}),
}
MUSE_SHA = "4c0f960028b603174af7df7bd5051d8c35d6c1aa372a37d18bc770926a0577a7"
MODELS = ("fable52", "fable51", "opus55", "astra")


def code_quality(l1: float, l2: float | None, weights: dict) -> float:
    """The v3 fallback split (no L3): L1 and L2 at 24/9, or L1 alone when L2 is missing."""
    if l2 is None:
        return l1
    split = weights["code_quality"]["fallback_without_l3"]
    return (split["l1_reviewed"] * l1 + split["l2_intent"] * l2) / (
        split["l1_reviewed"] + split["l2_intent"]
    )


def mean_present(values):
    present = [v for v in values if v is not None]
    return statistics.mean(present) if present else None


def se(values):
    return statistics.stdev(values) / math.sqrt(len(values))


def rows_of(directory: Path, protocol_id: str, models: dict):
    summary = json.loads((directory / "summary.json").read_text())
    protocol = json.loads((directory / "protocol.json").read_text())
    if summary["protocol"] != protocol_id or protocol["id"] != protocol_id:
        raise SystemExit(f"{directory} is not {protocol_id}")
    if protocol["reviewers"]["muse"]["binary_sha256"] != MUSE_SHA:
        raise SystemExit(f"{protocol_id} ran another Muse binary")
    weights, cq_weight = protocol["weights"], protocol["weights"]["code_quality"]["total"]
    out = []
    for row in summary["rows"]:
        if row["model"] not in models or not row.get("published"):
            continue
        panels, pub = row["panels"], row["published"]
        names = summary["passing_panels"]
        l1 = statistics.mean(panels[p]["l1"]["score"] for p in names)
        l2 = mean_present(panels[p]["l2"] for p in names)
        rebuilt = code_quality(l1, l2, weights)
        if abs(rebuilt - pub["code_quality"]) > 1e-6:
            raise SystemExit(f"{protocol_id} {row['id']}: rebuilt Code quality disagrees")
        muse = panels["muse"]
        muse_cq = code_quality(muse["l1"]["score"], muse["l2"], weights)
        out.append(
            {
                "model": models[row["model"]],
                "effort": row["effort"],
                "muse_l1": muse["l1"]["score"],
                "muse_cq": muse_cq,
                "muse_combined": pub["composite_v3"] - cq_weight * (pub["code_quality"] - muse_cq),
                "published_cq": pub["code_quality"],
                "published_combined": pub["composite_v3"],
            }
        )
    digest = hashlib.sha256((directory / "summary.json").read_bytes()).hexdigest()
    return out, digest


def main():
    parser = argparse.ArgumentParser()
    for key in ROUNDS:
        parser.add_argument(f"--{key}", type=Path, required=True)
    args = parser.parse_args()
    rows, digests = [], {}
    for key, (protocol_id, models) in ROUNDS.items():
        got, digests[protocol_id] = rows_of(getattr(args, key), protocol_id, models)
        rows += got
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    with OUTPUT.open("w", newline="") as fh:
        w = csv.writer(fh, lineterminator="\n")
        w.writerow(
            [
                "model",
                "effort",
                "n",
                "muse_review",
                "muse_review_se",
                "muse_code_quality",
                "muse_code_quality_se",
                "muse_combined",
                "muse_combined_se",
                "published_code_quality",
                "published_combined",
            ]
        )
        for m in MODELS:
            for e in LEVELS:
                cell = [r for r in rows if r["model"] == m and r["effort"] == e]
                if not cell:
                    raise SystemExit(f"{m} {e} has no rows")

                def col(k, cell=cell):
                    return [r[k] for r in cell]

                w.writerow(
                    [
                        m,
                        e,
                        len(cell),
                        f"{statistics.mean(col('muse_l1')):.4f}",
                        f"{se(col('muse_l1')):.4f}",
                        f"{statistics.mean(col('muse_cq')):.4f}",
                        f"{se(col('muse_cq')):.4f}",
                        f"{statistics.mean(col('muse_combined')):.4f}",
                        f"{se(col('muse_combined')):.4f}",
                        f"{statistics.mean(col('published_cq')):.4f}",
                        f"{statistics.mean(col('published_combined')):.4f}",
                    ]
                )
    OUTPUT.with_suffix(".json").write_text(
        json.dumps({"summary_sha256": digests, "muse_binary_sha256": MUSE_SHA}, indent=2) + "\n"
    )
    print(OUTPUT.read_text())


if __name__ == "__main__":
    main()
