#!/usr/bin/env python3
"""Time-sliced pass@1 over recorded runs: pass@1 within a wall-clock slice.

Computed post-hoc from run durations, so run conditions never change: a run
counts as passing at slice T only if it solved (functional == 1.0) AND its
recorded duration is at most T. This is the honest form of clock-based
difficulty: the full-budget score is always reported alongside it, and no
model was ever cut off by the slice.

Usage: python scripts/time_sliced.py [--suite cii-v2] [--slices 600,1800,3600]
"""

from __future__ import annotations

import argparse
import glob
import json
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--suite", default="cii-v2")
    parser.add_argument("--slices", default="600,1800,3600")
    args = parser.parse_args()
    slices = [int(s) for s in args.slices.split(",")]

    suite = json.loads((ROOT / f"tasks/{args.suite}/suite.json").read_text())
    tasks = suite["tasks"]

    runs: dict[str, dict[str, list[tuple[float, float, int]]]] = defaultdict(
        lambda: defaultdict(list)
    )
    for task in tasks:
        for path in glob.glob(str(ROOT / f"runs/{task}-*/summary.json")):
            summary = json.loads(Path(path).read_text())
            functional = (summary.get("scores") or {}).get("functional")
            if functional is None:
                continue
            runs[summary["model"]][task].append(
                (
                    functional,
                    summary.get("duration_s") or 0.0,
                    summary.get("total_tokens") or 0,
                )
            )

    header = (
        f"{'model':30s} {'tasks':>5s} {'full':>6s}"
        + "".join(f" {'@' + str(s // 60) + 'm':>7s}" for s in slices)
        + f" {'med tok/run':>12s}"
    )
    print(f"suite {args.suite}: {len(tasks)} tasks, time-sliced pass@1\n")
    print(header)
    for model, per_task in sorted(runs.items()):
        full_rates, slice_rates = [], {s: [] for s in slices}
        all_tokens = []
        for entries in per_task.values():
            full_rates.append(sum(1 for f, _, _ in entries if f == 1.0) / len(entries))
            all_tokens.extend(t for _, _, t in entries if t)
            for s in slices:
                slice_rates[s].append(
                    sum(1 for f, d, _ in entries if f == 1.0 and d <= s) / len(entries)
                )
        n = len(per_task)
        row = f"{model:30s} {n:5d} {100 * sum(full_rates) / n:5.1f}%"
        for s in slices:
            row += f" {100 * sum(slice_rates[s]) / n:6.1f}%"
        med_tok = sorted(all_tokens)[len(all_tokens) // 2] if all_tokens else 0
        row += f" {med_tok:12,d}"
        print(row)
    print(
        "\nSlices are analytic cuts of recorded durations; no run was "
        "terminated by a slice. Full-budget pass@1 is the measurement, "
        "sliced columns are the difficulty view."
    )


if __name__ == "__main__":
    main()
