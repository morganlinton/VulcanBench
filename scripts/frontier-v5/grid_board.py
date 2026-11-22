#!/usr/bin/env python3
"""Aggregate Frontier v5 grid run records into the board (JSON and CSV for the site).

usage: scripts/frontier-v5/grid_board.py --grid GRID.json --out DIR <runs-dir> [...]

Reads every ``run.json`` (schema ``vulcanbench-frontier-v5-run/2``, written by
``import_harbor.py``) under the given directories, places each run in the
grid with the grid file (``grid_plan.py``: each environment's language and
size tier), and writes in ``--out``:

  grid-board.json   {"suite", "grid", "generated_at", "rows": [...], "cells": [...], "notes": [...]}
  grid-board.csv    one row per model and effort
  grid-cells.csv    one row per model, effort and environment

One row per (reference, model, agent, effort, budget), PLAN-v6 section 5;
the budget (``budget_sec`` in the run record) splits a model's calibration
runs at different budgets into separate rows:

- **items**: items done over all items of the scored runs, with ``n=`` (the
  item count, and the number of environments it covers) and ±1 binomial
  stderr, sqrt(p(1 - p) / n). Items in one environment share a session, so
  the binomial stderr understates the spread between environments; the
  per-environment counts are published beside it.
- **per size tier** and **per language**: the same pooled ratio within each
  tier and each language, with its own ``n=`` and stderr.
- **per kind**: items done over items for defect, feature, perf and
  diagnosis items.
- **cells**: the six-by-two heatmap as counts, ``{"language": {"tier":
  {"done", "total"}}}``; never ranked.
- **tokens and cost**: input, cached input and output tokens summed over the
  scored runs, and the API-equivalent cost at list price (total and per run,
  over runs that have one).
- **coverage**: ``partial`` when a grid environment has no scored run;
  ``excluded`` counts runs that are not scored (infrastructure, or a
  verifier problem), never as zeros; ``budget_reached`` counts sessions
  stopped at the budget; ``tamper_flagged`` counts environments zeroed by
  the tamper scan; ``flags`` counts each flag (``safety-refusal``).

No speed column (R13). If an environment has more than one scored run in a
row (a rerun), the board refuses: one run per model per environment is the
protocol, and the operator removes the superseded run first. Rows sort by
items done over total, descending.
"""

from __future__ import annotations

import argparse
import csv
import json
import math
import sys
from collections import Counter, defaultdict
from collections.abc import Iterable
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

SCHEMA = "vulcanbench-frontier-v5-run/2"
LANGUAGES = ("c", "cpp", "java", "javascript", "python", "rust")
TIERS = ("smaller", "larger")
KINDS = ("defect", "feature", "perf", "diagnosis")

ROW_FIELDS = [
    "ref",
    "model",
    "agent",
    "agent_version",
    "effort",
    "budget_min",
    "items_done",
    "items_total",
    "rate",
    "rate_se",
    "n_envs",
    "tiers",
    "languages",
    "kinds",
    "cells",
    "tokens_input",
    "tokens_cached_input",
    "tokens_output",
    "cost_total_usd",
    "cost_per_run_usd",
    "n_cost",
    "partial",
    "excluded",
    "budget_reached",
    "tamper_flagged",
    "flags",
]
CELL_FIELDS = [
    "ref",
    "model",
    "agent",
    "effort",
    "budget_min",
    "task",
    "language",
    "tier",
    "items_done",
    "items_total",
    "kinds",
    "budget_reached",
    "tamper_clean",
    "guards_failed",
    "agent_minutes",
    "tokens_input",
    "tokens_output",
    "cost_usd",
    "flags",
    "scored",
    "task_checksum",
]


class BoardError(ValueError):
    pass


def load_runs(paths: Iterable[Path]) -> list[dict[str, Any]]:
    runs = []
    for root in paths:
        files = [root] if root.is_file() else sorted(root.rglob("run.json"))
        for p in files:
            d = json.loads(p.read_text())
            if d.get("schema") == SCHEMA:
                runs.append(d)
    return runs


def load_grid(path: Path) -> dict[str, dict[str, str]]:
    """{environment: {"language", "tier"}} from a grid file (validated by grid_plan.py)."""
    grid = json.loads(path.read_text())
    envs = grid.get("environments") if isinstance(grid, dict) else None
    if not isinstance(envs, dict) or not envs:
        raise BoardError(f"{path}: no environments")
    out = {}
    for name, cell in envs.items():
        if cell.get("language") not in LANGUAGES or cell.get("tier") not in TIERS:
            raise BoardError(f"{path}: {name}: bad language or tier")
        out[name] = {"language": cell["language"], "tier": cell["tier"]}
    return out


def binomial(done: int, total: int) -> dict[str, Any]:
    if total <= 0:
        return {"done": done, "total": total, "rate": None, "se": None}
    p = done / total
    return {
        "done": done,
        "total": total,
        "rate": round(p, 4),
        "se": round(math.sqrt(p * (1 - p) / total), 4),
    }


def _sum(values: Iterable[Any]) -> int | None:
    vals = [int(v) for v in values if v is not None]
    return sum(vals) if vals else None


def aggregate(
    runs: list[dict[str, Any]], grid: dict[str, dict[str, str]]
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    groups: dict[tuple[str, str, str, str, float | None], list[dict[str, Any]]] = defaultdict(list)
    for r in runs:
        if r.get("task") in grid:
            groups[(r["ref"], r["model"], r["agent"], r["effort"], r.get("budget_sec"))].append(r)
    rows, cells = [], []
    for (ref, model, agent, effort, budget), group in groups.items():
        budget_min = None if budget is None else round(float(budget) / 60, 2)
        scored = [r for r in group if r.get("scored")]
        by_env: dict[str, dict[str, Any]] = {}
        for r in scored:
            if r["task"] in by_env:
                raise BoardError(
                    f"{model} {effort} budget {budget_min} min: two scored runs on {r['task']} "
                    f"({by_env[r['task']]['trial']}, {r['trial']}); keep one"
                )
            by_env[r["task"]] = r
        done = sum(int(r["items_done"]) for r in scored)
        total = sum(int(r["items_total"]) for r in scored)
        head = binomial(done, total)
        tier_counts: dict[str, list[int]] = {t: [0, 0] for t in TIERS}
        lang_counts: dict[str, list[int]] = {lang: [0, 0] for lang in LANGUAGES}
        kind_counts: dict[str, list[int]] = {k: [0, 0] for k in KINDS}
        heat: dict[str, dict[str, dict[str, int]]] = {}
        for env, r in by_env.items():
            cell = grid[env]
            for counts, slot in ((tier_counts, cell["tier"]), (lang_counts, cell["language"])):
                counts[slot][0] += int(r["items_done"])
                counts[slot][1] += int(r["items_total"])
            for k, (d, t) in (r.get("items_by_kind") or {}).items():
                kc = kind_counts.setdefault(k, [0, 0])
                kc[0] += int(d)
                kc[1] += int(t)
            hc = heat.setdefault(cell["language"], {}).setdefault(
                cell["tier"], {"done": 0, "total": 0}
            )
            hc["done"] += int(r["items_done"])
            hc["total"] += int(r["items_total"])
        costs = [
            float(r["cost"]["api_equivalent_usd"])
            for r in scored
            if (r.get("cost") or {}).get("api_equivalent_usd") is not None
        ]
        flags = Counter(f for r in scored for f in r.get("flags") or [])
        versions = sorted({str(r.get("agent_version")) for r in group if r.get("agent_version")})
        rows.append(
            {
                "ref": ref,
                "model": model,
                "agent": agent,
                "agent_version": ",".join(versions),
                "effort": effort,
                "budget_min": budget_min,
                "items_done": done,
                "items_total": total,
                "rate": head["rate"],
                "rate_se": head["se"],
                "n_envs": len(by_env),
                "tiers": {t: binomial(*c) for t, c in tier_counts.items() if c[1]},
                "languages": {lang: binomial(*c) for lang, c in lang_counts.items() if c[1]},
                "kinds": {k: binomial(*c) for k, c in kind_counts.items() if c[1]},
                "cells": heat,
                "tokens_input": _sum((r.get("tokens") or {}).get("input") for r in scored),
                "tokens_cached_input": _sum(
                    (r.get("tokens") or {}).get("cached_input") for r in scored
                ),
                "tokens_output": _sum((r.get("tokens") or {}).get("output") for r in scored),
                "cost_total_usd": round(sum(costs), 4) if costs else None,
                "cost_per_run_usd": round(sum(costs) / len(costs), 4) if costs else None,
                "n_cost": len(costs),
                "partial": any(env not in by_env for env in grid),
                "excluded": len(group) - len(scored),
                "budget_reached": sum(1 for r in scored if r.get("budget_reached")),
                "tamper_flagged": sum(1 for r in scored if r.get("tamper_clean") is False),
                "flags": dict(sorted(flags.items())),
            }
        )
        for env in grid:
            run = by_env.get(env)
            cells.append(
                {
                    "ref": ref,
                    "model": model,
                    "agent": agent,
                    "effort": effort,
                    "budget_min": budget_min,
                    "task": env,
                    "language": grid[env]["language"],
                    "tier": grid[env]["tier"],
                    "items_done": run["items_done"] if run else None,
                    "items_total": run["items_total"] if run else None,
                    "kinds": (run.get("items_by_kind") or {}) if run else {},
                    "budget_reached": run.get("budget_reached") if run else None,
                    "tamper_clean": run.get("tamper_clean") if run else None,
                    "guards_failed": run.get("guards_failed") if run else None,
                    "agent_minutes": run.get("agent_minutes") if run else None,
                    "tokens_input": (run.get("tokens") or {}).get("input") if run else None,
                    "tokens_output": (run.get("tokens") or {}).get("output") if run else None,
                    "cost_usd": (run.get("cost") or {}).get("api_equivalent_usd") if run else None,
                    "flags": (run.get("flags") or []) if run else [],
                    "scored": run is not None,
                    "task_checksum": run.get("task_checksum") if run else None,
                }
            )

    def key(row: dict[str, Any]) -> tuple[float, str, str, float]:
        return (-(row["rate"] or 0.0), row["model"], row["effort"], row["budget_min"] or 0.0)

    def ident(x: dict[str, Any]) -> tuple[Any, ...]:
        return (x["ref"], x["model"], x["agent"], x["effort"], x["budget_min"])

    rows.sort(key=key)
    order = {ident(r): i for i, r in enumerate(rows)}
    envs = list(grid)
    cells.sort(
        key=lambda c: (
            order[ident(c)],
            envs.index(str(c["task"])),
        )
    )
    return rows, cells


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
    ap.add_argument("--grid", type=Path, required=True, help="the grid file the runs used")
    ap.add_argument("--out", type=Path, required=True)
    args = ap.parse_args(argv)
    try:
        grid = load_grid(args.grid)
        runs = load_runs(args.runs)
        if not runs:
            raise BoardError("no grid run.json found")
        rows, cells = aggregate(runs, grid)
    except BoardError as exc:
        print(f"grid_board: {exc}", file=sys.stderr)
        return 1
    notes = [
        "items = items done / items over the scored runs; stderr = sqrt(p(1-p)/n) over items",
        "items in one environment share a session: the binomial stderr understates the spread"
        " between environments",
        "tiers and languages pool items the same way; cells are counts, not ranks",
        "an environment the tamper scan flags scores 0 items",
        "cost = API-equivalent at list price; no speed column (R13)",
        "partial = some grid environment has no scored run",
    ]
    mixed = sorted(
        {
            c["task"]
            for c in cells
            if c["task_checksum"]
            and any(
                o["task"] == c["task"]
                and o["task_checksum"]
                and o["task_checksum"] != c["task_checksum"]
                for o in cells
            )
        }
    )
    if mixed:
        notes.append("environment versions differ across rows: " + ", ".join(mixed))
    board = {
        "suite": "frontier-v5",
        "grid": str(args.grid),
        "generated_at": datetime.now(UTC).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "environments": grid,
        "rows": rows,
        "cells": cells,
        "notes": notes,
    }
    args.out.mkdir(parents=True, exist_ok=True)
    (args.out / "grid-board.json").write_text(json.dumps(board, indent=1) + "\n")
    write_csv(args.out / "grid-board.csv", ROW_FIELDS, rows)
    write_csv(args.out / "grid-cells.csv", CELL_FIELDS, cells)
    for r in rows:
        se = "" if r["rate_se"] is None else f" ±{r['rate_se']}"
        print(
            f"{r['model']} {r['effort']} {r['budget_min']} min{'*' if r['partial'] else ''}: "
            f"{r['items_done']}/"
            f"{r['items_total']} items = {r['rate']}{se} ({r['n_envs']} environments), "
            f"budget reached {r['budget_reached']}, ${r['cost_total_usd']}"
        )
    if mixed:
        print("grid_board: " + notes[-1], file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
