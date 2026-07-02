#!/usr/bin/env python3
"""Per-(model, effort) cost-quality Pareto view over an effort-sweep output dir.

``vulcanbench leaderboard`` rolls runs up by model only, and ``vulcanbench
report`` summarizes effort sensitivity within strata. Neither prints the explicit
model x effort grid an effort sweep is run to read: the cost-quality Pareto
frontier. This groups the same run summaries by (model, effort) and reports
pass@1 alongside cost / tokens / latency / steps so you can answer "does
Sonnet-5-high dominate Opus-4.8-medium?".

Two axes to read each cell on (see the sonnet5-effort-sweep design notes):
  - by effort *name* (low/medium/high) -- the product-facing grid;
  - by avg tokens -- the rigorous normalization, since effort names are not
    equal-thinking across models. Sonnet 5 and Opus 4.8 share a tokenizer, so the
    token column is directly comparable between them.

Usage:
    python scripts/effort_pareto.py ../vb-effort
    python scripts/effort_pareto.py ../vb-effort --json
"""

from __future__ import annotations

import argparse
import json
from collections import defaultdict
from pathlib import Path
from typing import Any

from harness.leaderboard import scan_leaderboard

# Sort efforts low -> high; unknown/untagged last.
_EFFORT_ORDER = {"low": 0, "medium": 1, "high": 2, "extra-high": 3}


def _solved(row: dict[str, Any]) -> bool:
    return (row.get("functional") or 0) >= 1.0


def build_cells(rows: list[dict[str, Any]]) -> list[dict[str, Any]]:
    """Group leaderboard rows into one cell per (model, effort)."""
    by_cell: dict[tuple[str, Any], list[dict[str, Any]]] = defaultdict(list)
    for r in rows:
        by_cell[(r.get("model") or "?", r.get("effort_requested"))].append(r)

    cells: list[dict[str, Any]] = []
    for (model, effort), cell_rows in by_cell.items():
        by_task: dict[Any, list[dict[str, Any]]] = defaultdict(list)
        for r in cell_rows:
            by_task[r.get("task_id")].append(r)

        # pass@1: per task, fraction of attempts solved, averaged over tasks.
        per_task_rate = [
            sum(1 for a in attempts if _solved(a)) / len(attempts)
            for attempts in by_task.values()
        ]
        n_runs = len(cell_rows)
        solved_runs = sum(1 for r in cell_rows if _solved(r))
        costs = [r["cost_usd"] for r in cell_rows if r.get("cost_usd") is not None]
        toks = [r["total_tokens"] for r in cell_rows if r.get("total_tokens") is not None]
        durs = [r["duration_s"] for r in cell_rows if r.get("duration_s") is not None]
        steps = [r["steps"] for r in cell_rows if r.get("steps") is not None]
        total_cost = round(sum(costs), 4) if costs else None

        cells.append(
            {
                "model": model,
                "effort": effort,
                "n_tasks": len(by_task),
                "n_runs": n_runs,
                "pass_at_1": round(sum(per_task_rate) / len(per_task_rate), 4)
                if per_task_rate
                else None,
                "solved_runs": solved_runs,
                "total_cost_usd": total_cost,
                "cost_known": len(costs) == n_runs,  # False -> some runs unpriced
                "avg_cost_usd": round(sum(costs) / len(costs), 4) if costs else None,
                "cost_per_solved_usd": round(total_cost / solved_runs, 4)
                if (total_cost and solved_runs)
                else None,
                "avg_total_tokens": round(sum(toks) / len(toks)) if toks else None,
                "avg_duration_s": round(sum(durs) / len(durs), 1) if durs else None,
                "avg_steps": round(sum(steps) / len(steps), 1) if steps else None,
            }
        )

    cells.sort(key=lambda c: (c["model"], _EFFORT_ORDER.get(c["effort"], 9)))
    return cells


def _fmt(value: Any, width: int, prec: int | None = None) -> str:
    if value is None:
        return "-".rjust(width)
    if prec is not None and isinstance(value, (int, float)):
        return f"{value:.{prec}f}".rjust(width)
    return str(value).rjust(width)


def print_table(cells: list[dict[str, Any]]) -> None:
    header = (
        f"{'model':28} {'effort':7} {'tasks':>5} {'runs':>4} "
        f"{'pass@1':>7} {'$total':>8} {'$/run':>7} {'$/solved':>9} "
        f"{'tokens':>8} {'dur_s':>7} {'steps':>6}"
    )
    print(header)
    print("-" * len(header))
    for c in cells:
        flag = "" if c["cost_known"] else " (cost partial)"
        print(
            f"{(c['model'] or '?')[:28]:28} {str(c['effort'] or '-'):7} "
            f"{_fmt(c['n_tasks'], 5)} {_fmt(c['n_runs'], 4)} "
            f"{_fmt(c['pass_at_1'], 7, 3)} {_fmt(c['total_cost_usd'], 8, 3)} "
            f"{_fmt(c['avg_cost_usd'], 7, 4)} {_fmt(c['cost_per_solved_usd'], 9, 4)} "
            f"{_fmt(c['avg_total_tokens'], 8)} {_fmt(c['avg_duration_s'], 7, 1)} "
            f"{_fmt(c['avg_steps'], 6, 1)}{flag}"
        )
    print(
        "\nRead each cell two ways: by effort name (the grid) and by avg tokens "
        "(effort names\nare not equal-thinking across models). Pareto win = higher "
        "pass@1 at lower $/solved."
    )


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("output_dir", type=Path, help="effort-sweep -o dir (holds run summaries)")
    ap.add_argument("--json", action="store_true", help="emit JSON instead of a table")
    args = ap.parse_args()

    rows = scan_leaderboard(args.output_dir)
    if not rows:
        raise SystemExit(f"no run summaries found under {args.output_dir}")
    cells = build_cells(rows)
    if args.json:
        print(json.dumps(cells, indent=2))
    else:
        print_table(cells)


if __name__ == "__main__":
    main()
