#!/usr/bin/env python3
"""Aggregate Frontier v5 run records into the release 1 board (CSV and JSON for the site).

usage: scripts/frontier-v5/board.py --out DIR [--attempts 3] <runs-dir> [...]

Reads every ``run.json`` under the given directories (written by
``import_harbor.py``) and writes, in ``--out``:

  board.json        {"suite", "suite_version", "progress_rule_sha256", "generated_at",
                     "rows": [...], "tasks": [...], "notes": [...]}
  board.csv         one row per model and effort
  board-tasks.csv   one row per model, effort and task

One row per (reference, model, agent, effort), from scored runs only
(``scored`` in the record; infrastructure trials are counted under
``excluded`` and never scored as zeros):

- **pass rate**: passes over attempts (decision R3's headline).
- **progress**: the mean progress score over the same attempts.
- **n=**: scored attempts, with the number of tasks they cover.
- **±1 stderr** for both, the board's convention (scripts/rankings-chart):
  the sample standard deviation of the per-task means over sqrt(n_tasks).
  With one task, or none, it is null.
- **median agent minutes**, from serial runs only (job concurrency 1), with
  its own count; null when no serial run exists.
- **API-equivalent cost** at list price: the total and the mean per
  attempt, over the attempts that have a cost.
- **coverage**: a row is ``partial`` when any board task has fewer scored
  attempts than ``--attempts``; the site marks it with an asterisk.
- flags: how many scored attempts carry each flag (``safety-refusal``), and
  every task checksum seen, so a row that mixes task versions shows it.

The per-task table has the same measures for each task. Rows sort by pass
rate, then progress, both descending.
"""

from __future__ import annotations

import argparse
import csv
import json
import math
import statistics
import sys
from collections import Counter, defaultdict
from collections.abc import Iterable
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
RULE = HERE / "progress_rule.json"
SUITE_JSON = REPO / "tasks" / "frontier-v5" / "suite.json"

ROW_FIELDS = [
    "ref",
    "model",
    "agent",
    "agent_version",
    "effort",
    "n",
    "n_tasks",
    "passes",
    "pass_rate",
    "pass_rate_se",
    "progress",
    "progress_se",
    "median_agent_minutes",
    "n_speed",
    "cost_total_usd",
    "cost_per_attempt_usd",
    "n_cost",
    "partial",
    "excluded",
    "flags",
]
TASK_FIELDS = [
    "ref",
    "model",
    "agent",
    "effort",
    "task",
    "n",
    "passes",
    "pass_rate",
    "progress",
    "median_agent_minutes",
    "n_speed",
    "cost_total_usd",
    "task_checksums",
]


def load_runs(paths: Iterable[Path]) -> list[dict[str, Any]]:
    runs = []
    for root in paths:
        files = [root] if root.is_file() else sorted(root.rglob("run.json"))
        for p in files:
            d = json.loads(p.read_text())
            if d.get("schema") == "vulcanbench-frontier-v5-run/1":
                runs.append(d)
    return runs


def _se(per_task: list[float]) -> float | None:
    n = len(per_task)
    if n < 2:
        return None
    mean = sum(per_task) / n
    var = sum((x - mean) ** 2 for x in per_task) / (n - 1)
    return round(math.sqrt(var / n), 4)


def _round(x: float | None, nd: int = 4) -> float | None:
    return None if x is None else round(x, nd)


def _measures(runs: list[dict[str, Any]]) -> dict[str, Any]:
    n = len(runs)
    passes = sum(1 for r in runs if r["passed"])
    progress = [float(r["progress"] or 0.0) for r in runs]
    serial = [
        float(r["agent_minutes"])
        for r in runs
        if r.get("concurrency") == 1 and r.get("agent_minutes") is not None
    ]
    costs = [
        float(r["cost"]["api_equivalent_usd"])
        for r in runs
        if (r.get("cost") or {}).get("api_equivalent_usd") is not None
    ]
    return {
        "n": n,
        "passes": passes,
        "pass_rate": _round(passes / n) if n else None,
        "progress": _round(sum(progress) / n) if n else None,
        "median_agent_minutes": _round(statistics.median(serial), 2) if serial else None,
        "n_speed": len(serial),
        "cost_total_usd": _round(sum(costs), 4) if costs else None,
        "cost_per_attempt_usd": _round(sum(costs) / len(costs), 4) if costs else None,
        "n_cost": len(costs),
    }


def aggregate(
    runs: list[dict[str, Any]], tasks: list[str], attempts: int
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    groups: dict[tuple[str, str, str, str], list[dict[str, Any]]] = defaultdict(list)
    for r in runs:
        if r.get("task") in tasks:
            groups[(r["ref"], r["model"], r["agent"], r["effort"])].append(r)
    rows, task_rows = [], []
    for (ref, model, agent, effort), group in groups.items():
        scored = [r for r in group if r.get("scored")]
        by_task: dict[str, list[dict[str, Any]]] = defaultdict(list)
        for r in scored:
            by_task[r["task"]].append(r)
        m = _measures(scored)
        flags = Counter(f for r in scored for f in r.get("flags") or [])
        versions = sorted({str(r.get("agent_version")) for r in group if r.get("agent_version")})
        rows.append(
            {
                "ref": ref,
                "model": model,
                "agent": agent,
                "agent_version": ",".join(versions),
                "effort": effort,
                **{k: m[k] for k in ("n", "passes", "pass_rate", "progress")},
                "n_tasks": len(by_task),
                "pass_rate_se": _se(
                    [sum(r["passed"] for r in v) / len(v) for v in by_task.values()]
                ),
                "progress_se": _se(
                    [sum(float(r["progress"] or 0) for r in v) / len(v) for v in by_task.values()]
                ),
                **{
                    k: m[k]
                    for k in (
                        "median_agent_minutes",
                        "n_speed",
                        "cost_total_usd",
                        "cost_per_attempt_usd",
                        "n_cost",
                    )
                },
                "partial": any(len(by_task.get(t, [])) < attempts for t in tasks),
                "excluded": len(group) - len(scored),
                "flags": dict(sorted(flags.items())),
            }
        )
        for t in tasks:
            tm = _measures(by_task.get(t, []))
            task_rows.append(
                {
                    "ref": ref,
                    "model": model,
                    "agent": agent,
                    "effort": effort,
                    "task": t,
                    **{
                        k: tm[k]
                        for k in (
                            "n",
                            "passes",
                            "pass_rate",
                            "progress",
                            "median_agent_minutes",
                            "n_speed",
                            "cost_total_usd",
                        )
                    },
                    "task_checksums": sorted(
                        {
                            str(r["task_checksum"])
                            for r in group
                            if r.get("task") == t and r.get("task_checksum")
                        }
                    ),
                }
            )

    def key(row: dict[str, Any]) -> tuple[float, float, str, str]:
        return (-(row["pass_rate"] or 0.0), -(row["progress"] or 0.0), row["model"], row["effort"])

    rows.sort(key=key)
    order = {(r["ref"], r["model"], r["agent"], r["effort"]): i for i, r in enumerate(rows)}
    task_rows.sort(
        key=lambda t: (
            order[(t["ref"], t["model"], t["agent"], t["effort"])],
            tasks.index(t["task"]),
        )
    )
    return rows, task_rows


def _csv_value(v: Any) -> Any:
    if isinstance(v, (dict, list)):
        return json.dumps(v, sort_keys=True)
    if v is None:
        return ""
    return v


def write_csv(path: Path, fields: list[str], rows: list[dict[str, Any]]) -> None:
    with path.open("w", newline="") as fh:
        w = csv.DictWriter(fh, fieldnames=fields)
        w.writeheader()
        for r in rows:
            w.writerow({k: _csv_value(r.get(k)) for k in fields})


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("runs", nargs="+", type=Path, help="directories written by import_harbor.py")
    ap.add_argument("--out", type=Path, required=True)
    ap.add_argument("--attempts", type=int, default=3, help="attempts per task for full coverage")
    args = ap.parse_args(argv)
    rule = json.loads(RULE.read_text())
    tasks = list(rule["tasks"])
    runs = load_runs(args.runs)
    if not runs:
        print("board: no run.json found", file=sys.stderr)
        return 1
    rows, task_rows = aggregate(runs, tasks, args.attempts)
    suite = json.loads(SUITE_JSON.read_text())
    pin = (suite.get("pins") or {}).get("progress_rule") or {}
    notes = [
        "pass rate = passes / scored attempts; progress = mean progress score (progress_rule.json)",
        "stderr = sample std of per-task means / sqrt(n_tasks)",
        "median agent minutes from serial runs only (job concurrency 1)",
        "cost = API-equivalent at list price",
        f"partial = some task has fewer than {args.attempts} scored attempts",
    ]
    mixed = [t for t in task_rows if len(t["task_checksums"]) > 1]
    if mixed:
        notes.append(
            "mixed task versions: " + ", ".join(f"{t['model']}/{t['task']}" for t in mixed)
        )
    board = {
        "suite": "frontier-v5",
        "suite_version": suite.get("version"),
        "progress_rule_sha256": pin.get("sha256"),
        "generated_at": datetime.now(UTC).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "tasks": tasks,
        "rows": rows,
        "per_task": task_rows,
        "notes": notes,
    }
    args.out.mkdir(parents=True, exist_ok=True)
    (args.out / "board.json").write_text(json.dumps(board, indent=1) + "\n")
    write_csv(args.out / "board.csv", ROW_FIELDS, rows)
    write_csv(args.out / "board-tasks.csv", TASK_FIELDS, task_rows)
    for r in rows:
        se = "" if r["pass_rate_se"] is None else f" ±{r['pass_rate_se']}"
        print(
            f"{r['model']} {r['effort']}{'*' if r['partial'] else ''}: pass {r['pass_rate']}{se} "
            f"progress {r['progress']} n={r['n']} ({r['n_tasks']} tasks) "
            f"median {r['median_agent_minutes']} min, ${r['cost_total_usd']}"
        )
    if mixed:
        print("board: " + notes[-1], file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
