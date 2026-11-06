#!/usr/bin/env python3
"""Analyse router-eval results: routing sensitivity and real cost.

Inputs:
  --ladder FILE          probe_ladder.py JSONL (repeatable)
  --runs DIR --leg NAME  a folder of run dirs (``<task>-<hex>/summary.json``)
                         labelled as one leg, e.g. ``--runs runs-router-martian
                         --leg martian`` (pairs, repeatable, in order)
  --suite DIR            the suite (cells come from each task's ``router_cell``)
  --plan-fee LEG=USD     monthly seat price for a subscription leg
  --manual-capacity LEG=USD  list-price dollars one WEEK of that seat buys,
                         read by hand from the product's usage page (Cursor,
                         Codex) when the stream carries no usage window
  --out analysis.json    and ``--csv-dir`` for the CSV tables

Statistics are plain Python (no scipy): chi-square independence tests with
the p-value from the regularised upper incomplete gamma function, and
normalised mutual information I(cell; model) / sqrt(H(cell) H(model)).
Subscription and API legs are never summed into one claim; every row keeps
its ``cost_basis``.
"""

from __future__ import annotations

import argparse
import csv
import json
import math
import statistics
import sys
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))

CELL_KEYS = ("language", "scale", "difficulty")
WEEK_WINDOW_HINTS = ("seven", "7", "week")


# ----------------------------------------------------------------------------
# statistics


def _gammainc_upper_regularized(s: float, x: float) -> float:
    """Q(s, x) = Gamma(s, x) / Gamma(s) for s > 0, x >= 0."""
    if x <= 0:
        return 1.0
    if x < s + 1:
        # series for P(s, x), then Q = 1 - P
        term = 1.0 / s
        total = term
        n = 1
        while n < 10000:
            term *= x / (s + n)
            total += term
            if abs(term) < abs(total) * 1e-15:
                break
            n += 1
        p = total * math.exp(-x + s * math.log(x) - math.lgamma(s))
        return max(0.0, min(1.0, 1.0 - p))
    # continued fraction (Lentz) for Q(s, x)
    tiny = 1e-300
    b = x + 1.0 - s
    c = 1.0 / tiny
    d = 1.0 / b
    h = d
    for i in range(1, 10000):
        an = -i * (i - s)
        b += 2.0
        d = an * d + b
        if abs(d) < tiny:
            d = tiny
        c = b + an / c
        if abs(c) < tiny:
            c = tiny
        d = 1.0 / d
        delta = d * c
        h *= delta
        if abs(delta - 1.0) < 1e-15:
            break
    return max(0.0, min(1.0, math.exp(-x + s * math.log(x) - math.lgamma(s)) * h))


def chi2_sf(stat: float, dof: int) -> float:
    """Survival function of the chi-square distribution."""
    if dof <= 0:
        return 1.0
    return _gammainc_upper_regularized(dof / 2.0, stat / 2.0)


def chi_square(table: dict[str, Counter[str]]) -> dict[str, Any]:
    """Independence test on ``{row_label: Counter(col_label)}``."""
    rows = [r for r in table if sum(table[r].values()) > 0]
    cols = sorted({c for r in rows for c in table[r]})
    if len(rows) < 2 or len(cols) < 2:
        return {
            "statistic": 0.0,
            "dof": 0,
            "p_value": 1.0,
            "n": sum(sum(table[r].values()) for r in rows),
        }
    n = sum(sum(table[r].values()) for r in rows)
    row_tot = {r: sum(table[r].values()) for r in rows}
    col_tot = {c: sum(table[r].get(c, 0) for r in rows) for c in cols}
    stat = 0.0
    for r in rows:
        for c in cols:
            expected = row_tot[r] * col_tot[c] / n
            if expected > 0:
                stat += (table[r].get(c, 0) - expected) ** 2 / expected
    dof = (len(rows) - 1) * (len(cols) - 1)
    return {"statistic": round(stat, 4), "dof": dof, "p_value": chi2_sf(stat, dof), "n": n}


def _entropy(counts: Counter[Any]) -> float:
    n = sum(counts.values())
    return -sum((c / n) * math.log(c / n) for c in counts.values() if c) if n else 0.0


def normalized_mutual_information(table: dict[str, Counter[str]]) -> float:
    """I(X;Y) / sqrt(H(X) H(Y)) over ``{x: Counter(y)}``; 0 when either is constant."""
    joint = Counter({(x, y): c for x, ys in table.items() for y, c in ys.items() if c})
    n = sum(joint.values())
    if not n:
        return 0.0
    px = Counter()
    py = Counter()
    for (x, y), c in joint.items():
        px[x] += c
        py[y] += c
    mi = 0.0
    for (x, y), c in joint.items():
        mi += (c / n) * math.log((c / n) / ((px[x] / n) * (py[y] / n)))
    hx, hy = _entropy(px), _entropy(py)
    if hx <= 0 or hy <= 0:
        return 0.0
    return max(0.0, round(mi / math.sqrt(hx * hy), 6))


def mean_se(values: list[float]) -> tuple[float | None, float | None, int]:
    vals = [v for v in values if v is not None]
    if not vals:
        return None, None, 0
    if len(vals) == 1:
        return vals[0], None, 1
    return statistics.fmean(vals), statistics.stdev(vals) / math.sqrt(len(vals)), len(vals)


# ----------------------------------------------------------------------------
# loading


def load_cells(suite_dir: Path) -> dict[str, dict[str, str]]:
    suite = json.loads((suite_dir / "suite.json").read_text())
    cells: dict[str, dict[str, str]] = {}
    for tid in suite.get("tasks", []):
        meta_path = suite_dir / tid / "metadata.json"
        cell = None
        if meta_path.exists():
            cell = json.loads(meta_path.read_text()).get("router_cell")
        cell = cell or suite.get("cells", {}).get(tid)
        if cell:
            cells[tid] = cell
    return cells


def cell_label(cell: dict[str, str]) -> str:
    return "/".join(cell[k] for k in CELL_KEYS)


def load_ladder(paths: list[Path]) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for p in paths:
        for line in p.read_text().splitlines():
            if line.strip():
                rows.append(json.loads(line))
    return rows


def _week_window(delta: dict[str, Any]) -> str | None:
    names = [k for k, v in delta.items() if v is not None]
    for k in names:
        if any(h in k.lower() for h in WEEK_WINDOW_HINTS):
            return k
    return max(names, key=len) if names else None


def summary_row(
    summary: dict[str, Any], leg: str, cells: dict[str, dict[str, str]]
) -> dict[str, Any]:
    scores = summary.get("scores") or {}
    econ = summary.get("economics") or {}
    cli = summary.get("cli_agent") or {}
    tokens = summary.get("tokens") or {}
    cost_basis = econ.get("cost_basis") or ("subscription" if cli else "metered-api-pricing")
    cash = econ.get("marginal_cash_usd")
    if cash is None and not cli:
        cash = summary.get("cost_usd")
    api_equiv = econ.get("api_equivalent_cost_usd")
    if api_equiv is None:
        api_equiv = cli.get("cli_reported_cost_usd") if cli else summary.get("cost_usd")
    quota = econ.get("quota") or cli.get("quota") or {}
    delta = quota.get("delta") or {}
    week = _week_window(delta) if delta else None
    task_id = summary.get("task_id")
    return {
        "leg": leg,
        "run_id": summary.get("run_id"),
        "task_id": task_id,
        "cell": cells.get(task_id),
        "solved": 1.0 if scores.get("functional") == 1.0 else 0.0,
        "cost_basis": cost_basis,
        "billing_mode": econ.get("billing_mode"),
        "cash_usd": cash,
        "api_equivalent_usd": api_equiv,
        "tokens": tokens.get("total") or summary.get("total_tokens"),
        "duration_s": summary.get("duration_s"),
        "steps": summary.get("steps"),
        "served_models": summary.get("served_models") or cli.get("served_models") or {},
        "served_model_switches": summary.get(
            "served_model_switches", cli.get("served_model_switches", 0)
        )
        or 0,
        "quota_source": quota.get("source"),
        "quota_week_window": week,
        "quota_week_delta": delta.get(week) if week else None,
        "plan_name": econ.get("plan_name") or cli.get("plan_name"),
    }


def load_runs(
    pairs: list[tuple[Path, str]], cells: dict[str, dict[str, str]]
) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for run_dir, leg in pairs:
        for summary_path in sorted(run_dir.glob("*/summary.json")):
            try:
                summary = json.loads(summary_path.read_text())
            except json.JSONDecodeError:
                continue
            if summary.get("task_id") not in cells:
                continue
            rows.append(summary_row(summary, leg, cells))
    return rows


# ----------------------------------------------------------------------------
# routing


def _share_table(table: dict[str, Counter[str]]) -> dict[str, dict[str, Any]]:
    out: dict[str, dict[str, Any]] = {}
    for key, counts in table.items():
        n = sum(counts.values())
        out[key] = {
            "n": n,
            "share": {m: round(c / n, 4) for m, c in sorted(counts.items())} if n else {},
        }
    return out


def routing_from_rows(
    rows: list[dict[str, Any]], group_key: str, cell_of: Any, model_of: Any
) -> dict[str, Any]:
    """Per group (router or leg): served-model share per cell, chi-square, NMI."""
    result: dict[str, Any] = {}
    by_group: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for r in rows:
        by_group[r[group_key]].append(r)
    for group, items in sorted(by_group.items()):
        by_cell: dict[str, Counter[str]] = defaultdict(Counter)
        by_factor: dict[str, dict[str, Counter[str]]] = {k: defaultdict(Counter) for k in CELL_KEYS}
        overall: Counter[str] = Counter()
        for r in items:
            cell = cell_of(r)
            if not cell:
                continue
            for model, weight in model_of(r).items():
                by_cell[cell_label(cell)][model] += weight
                overall[model] += weight
                for k in CELL_KEYS:
                    by_factor[k][cell[k]][model] += weight
        result[group] = {
            "n": sum(overall.values()),
            "served_models": dict(sorted(overall.items())),
            "distinct_models": len(overall),
            "by_cell": _share_table(by_cell),
            "cell_test": {**chi_square(by_cell), "nmi": normalized_mutual_information(by_cell)},
            "by_factor": {
                k: {
                    "shares": _share_table(by_factor[k]),
                    "test": {
                        **chi_square(by_factor[k]),
                        "nmi": normalized_mutual_information(by_factor[k]),
                    },
                }
                for k in CELL_KEYS
            },
        }
    return result


def ladder_routing(rows: list[dict[str, Any]]) -> dict[str, Any]:
    ok = [r for r in rows if r.get("served_model") and not r.get("error")]
    base = [r for r in ok if not r.get("control")]
    out = routing_from_rows(
        base, "router", lambda r: r.get("cell"), lambda r: Counter({str(r["served_model"]): 1})
    )
    # Length control: the same small+easy tasks, plain vs padded.
    by_router: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for r in ok:
        by_router[r["router"]].append(r)
    for router, items in by_router.items():
        padded_tasks = {r["task_id"] for r in items if r.get("control") == "padded"}
        table: dict[str, Counter[str]] = defaultdict(Counter)
        for r in items:
            if r["task_id"] in padded_tasks:
                table["padded" if r.get("control") == "padded" else "plain"][
                    str(r["served_model"])
                ] += 1
        errors = sum(1 for r in rows if r.get("router") == router and r.get("error"))
        if router not in out:
            out[router] = {
                "n": 0,
                "served_models": {},
                "distinct_models": 0,
                "by_cell": {},
                "cell_test": {},
                "by_factor": {},
            }
        out[router]["length_control"] = {
            "shares": _share_table(table),
            "test": {**chi_square(table), "nmi": normalized_mutual_information(table)},
        }
        # served model against prompt size, in quartiles of prompt_tokens
        sized = [r for r in items if isinstance(r.get("prompt_tokens"), (int, float))]
        sized.sort(key=lambda r: r["prompt_tokens"])
        quartiles: dict[str, Counter[str]] = defaultdict(Counter)
        if sized:
            q = max(1, len(sized) // 4)
            for i, r in enumerate(sized):
                quartiles[f"q{min(i // q, 3) + 1}"][str(r["served_model"])] += 1
        out[router]["by_prompt_quartile"] = _share_table(quartiles)
        out[router]["errors"] = errors
        costs = [
            r["reported_cost_usd"]
            for r in items
            if isinstance(r.get("reported_cost_usd"), (int, float))
        ]
        lat = [r["latency_s"] for r in items if isinstance(r.get("latency_s"), (int, float))]
        out[router]["mean_reported_cost_usd"] = round(statistics.fmean(costs), 6) if costs else None
        out[router]["mean_latency_s"] = round(statistics.fmean(lat), 3) if lat else None
        out[router]["cost_reported_share"] = round(len(costs) / len(items), 3) if items else None
    return out


def run_routing(rows: list[dict[str, Any]]) -> dict[str, Any]:
    with_models = [r for r in rows if r.get("served_models")]
    out = routing_from_rows(
        with_models, "leg", lambda r: r.get("cell"), lambda r: Counter(r["served_models"])
    )
    by_leg: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for r in rows:
        by_leg[r["leg"]].append(r)
    for leg, items in by_leg.items():
        rates = [
            r["served_model_switches"] / r["steps"]
            for r in items
            if r.get("steps") and r.get("served_models")
        ]
        out.setdefault(
            leg,
            {
                "n": 0,
                "served_models": {},
                "distinct_models": 0,
                "by_cell": {},
                "cell_test": {},
                "by_factor": {},
            },
        )
        out[leg]["runs"] = len(items)
        out[leg]["runs_with_served_models"] = len([r for r in items if r.get("served_models")])
        out[leg]["within_run_switch_rate"] = round(statistics.fmean(rates), 4) if rates else None
    return out


# ----------------------------------------------------------------------------
# cost


def _cost_stats(items: list[dict[str, Any]], key: str) -> dict[str, Any]:
    mean, se, _ = mean_se([r[key] for r in items])
    solved = sum(r["solved"] for r in items)
    total = sum(r[key] for r in items if r[key] is not None)
    priced = len([r for r in items if r[key] is not None])
    return {
        "n": len(items),
        "priced_runs": priced,
        "mean_usd": round(mean, 6) if mean is not None else None,
        "se_usd": round(se, 6) if se is not None else None,
        "per_solved_usd": round(total / solved, 6) if solved and priced else None,
        "pass_rate": round(solved / len(items), 4) if items else None,
    }


def cost_by_leg(rows: list[dict[str, Any]]) -> dict[str, Any]:
    out: dict[str, Any] = {}
    by_leg: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for r in rows:
        by_leg[r["leg"]].append(r)
    for leg, items in sorted(by_leg.items()):
        bases = Counter(r["cost_basis"] for r in items)
        minutes = [r["duration_s"] / 60 for r in items if r.get("duration_s") is not None]
        toks = [r["tokens"] for r in items if r.get("tokens") is not None]
        by_cell: dict[str, Any] = {}
        for cell in sorted({cell_label(r["cell"]) for r in items if r.get("cell")}):
            sub = [r for r in items if r.get("cell") and cell_label(r["cell"]) == cell]
            by_cell[cell] = {
                "cash": _cost_stats(sub, "cash_usd"),
                "api_equivalent": _cost_stats(sub, "api_equivalent_usd"),
            }
        out[leg] = {
            "cost_basis": dict(bases),
            "billing_mode": Counter(r.get("billing_mode") for r in items).most_common(1)[0][0],
            "cash": _cost_stats(items, "cash_usd"),
            "api_equivalent": _cost_stats(items, "api_equivalent_usd"),
            "minutes_mean": round(statistics.fmean(minutes), 3) if minutes else None,
            "tokens_mean": round(statistics.fmean(toks)) if toks else None,
            "by_cell": by_cell,
        }
    return out


def subscription_capacity(
    rows: list[dict[str, Any]],
    plan_fee: dict[str, float],
    manual_capacity: dict[str, float],
) -> dict[str, Any]:
    """List-price dollars one week of each seat buys, from per-task window deltas."""
    out: dict[str, Any] = {}
    by_leg: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for r in rows:
        by_leg[r["leg"]].append(r)
    for leg, items in sorted(by_leg.items()):
        measured = [
            r
            for r in items
            if r.get("quota_week_delta")
            and r["quota_week_delta"] > 0
            and r.get("api_equivalent_usd")
        ]
        entry: dict[str, Any] = {
            "plan_fee_usd_month": plan_fee.get(leg),
            "runs_with_window_delta": len(measured),
            "quota_source": Counter(
                r.get("quota_source") for r in items if r.get("quota_source")
            ).most_common(1)[0][0]
            if any(r.get("quota_source") for r in items)
            else None,
        }
        if measured:
            per_task = [r["api_equivalent_usd"] / r["quota_week_delta"] for r in measured]
            pooled = sum(r["api_equivalent_usd"] for r in measured) / sum(
                r["quota_week_delta"] for r in measured
            )
            mean, se, _ = mean_se(per_task)
            entry["capacity_usd_week"] = round(pooled, 2)
            entry["capacity_usd_week_mean_of_tasks"] = round(mean, 2) if mean is not None else None
            entry["capacity_usd_week_se"] = round(se, 2) if se is not None else None
            entry["capacity_method"] = "pooled list-price cost / pooled 7-day window delta"
        elif leg in manual_capacity:
            entry["capacity_usd_week"] = manual_capacity[leg]
            entry["capacity_method"] = "manual reading of the product's usage page"
        if entry.get("capacity_usd_week") and plan_fee.get(leg):
            cap_month = entry["capacity_usd_week"] * 30 / 7
            api_mean = cost_by_leg(items)[leg]["api_equivalent"]["mean_usd"]
            entry["capacity_usd_month"] = round(cap_month, 2)
            entry["effective_discount_vs_list"] = (
                round(1 - plan_fee[leg] / cap_month, 4) if cap_month else None
            )
            if api_mean:
                entry["tasks_per_month_capacity"] = round(cap_month / api_mean, 1)
                entry["effective_usd_per_task_at_capacity"] = round(
                    plan_fee[leg] / (cap_month / api_mean), 4
                )
        out[leg] = entry
    return out


def break_even(
    cost_legs: dict[str, Any],
    capacity: dict[str, Any],
    plan_fee: dict[str, float],
    demands: list[int] | None = None,
) -> dict[str, Any]:
    """Monthly cost per engineer against monthly task demand, per leg."""
    demands = demands or [5, 10, 20, 40, 80, 160, 320, 640]
    curves: dict[str, Any] = {}
    per_task: dict[str, float] = {}
    for leg, stats in cost_legs.items():
        if leg in plan_fee:
            cap = capacity.get(leg, {})
            tasks_cap = cap.get("tasks_per_month_capacity")
            fee = plan_fee[leg]
            curves[leg] = {
                "kind": "subscription",
                "points": [
                    {
                        "tasks": d,
                        "usd": round(fee * (math.ceil(d / tasks_cap) if tasks_cap else 1), 2),
                    }
                    for d in demands
                ],
                "note": "seats added when demand exceeds one seat's capacity"
                if tasks_cap
                else "capacity unknown: one seat assumed",
            }
        else:
            unit = (
                stats["cash"]["mean_usd"]
                if stats["cash"]["mean_usd"] is not None
                else stats["api_equivalent"]["mean_usd"]
            )
            if unit is None:
                continue
            per_task[leg] = unit
            curves[leg] = {
                "kind": "metered",
                "usd_per_task": unit,
                "points": [{"tasks": d, "usd": round(unit * d, 2)} for d in demands],
            }
    crossovers: list[dict[str, Any]] = []
    for sub, fee in plan_fee.items():
        cap_tasks = capacity.get(sub, {}).get("tasks_per_month_capacity")
        for leg, unit in per_task.items():
            d_star = fee / unit if unit else None
            crossovers.append(
                {
                    "subscription": sub,
                    "metered": leg,
                    "tasks_per_month_where_seat_wins": round(d_star, 1) if d_star else None,
                    "within_one_seat_capacity": (d_star <= cap_tasks)
                    if (d_star and cap_tasks)
                    else None,
                }
            )
    return {"demands": demands, "curves": curves, "crossovers": crossovers}


# ----------------------------------------------------------------------------
# cli


def _kv(items: list[str]) -> dict[str, float]:
    out: dict[str, float] = {}
    for item in items:
        k, _, v = item.partition("=")
        out[k] = float(v)
    return out


def _write_routing_csvs(analysis: dict[str, Any], csv_dir: Path) -> None:
    with (csv_dir / "routing_shares.csv").open("w", newline="") as fh:
        w = csv.writer(fh)
        w.writerow(["source", "group", "cell", "n", "served_model", "share"])
        for source in ("ladder", "runs"):
            for group, data in analysis["routing"][source].items():
                for cell, entry in data.get("by_cell", {}).items():
                    for model, share in entry["share"].items():
                        w.writerow([source, group, cell, entry["n"], model, share])
    with (csv_dir / "routing_tests.csv").open("w", newline="") as fh:
        w = csv.writer(fh)
        w.writerow(["source", "group", "factor", "n", "chi2", "dof", "p_value", "nmi"])
        for source in ("ladder", "runs"):
            for group, data in analysis["routing"][source].items():
                tests = [("cell", data.get("cell_test") or {})]
                tests += [(factor, f["test"]) for factor, f in data.get("by_factor", {}).items()]
                if data.get("length_control"):
                    tests.append(("length_control", data["length_control"]["test"]))
                for factor, t in tests:
                    if t:
                        w.writerow(
                            [
                                source,
                                group,
                                factor,
                                t.get("n"),
                                t.get("statistic"),
                                t.get("dof"),
                                t.get("p_value"),
                                t.get("nmi"),
                            ]
                        )


def _cost_row(
    leg: str,
    cell: str,
    basis: str,
    cash: dict[str, Any],
    api: dict[str, Any],
    minutes: Any = "",
    tokens: Any = "",
) -> list[Any]:
    return [
        leg,
        cell,
        basis,
        cash["n"],
        cash["pass_rate"],
        cash["mean_usd"],
        cash["se_usd"],
        cash["per_solved_usd"],
        api["mean_usd"],
        api["se_usd"],
        minutes,
        tokens,
    ]


def _write_cost_csvs(analysis: dict[str, Any], csv_dir: Path) -> None:
    with (csv_dir / "cost_by_leg.csv").open("w", newline="") as fh:
        w = csv.writer(fh)
        w.writerow(
            [
                "leg",
                "cell",
                "cost_basis",
                "n",
                "pass_rate",
                "cash_mean_usd",
                "cash_se_usd",
                "cash_per_solved_usd",
                "api_equiv_mean_usd",
                "api_equiv_se_usd",
                "minutes_mean",
                "tokens_mean",
            ]
        )
        for leg, data in analysis["cost"].items():
            basis = ",".join(sorted(data["cost_basis"]))
            w.writerow(
                _cost_row(
                    leg,
                    "all",
                    basis,
                    data["cash"],
                    data["api_equivalent"],
                    data["minutes_mean"],
                    data["tokens_mean"],
                )
            )
            for cell, c in data["by_cell"].items():
                w.writerow(_cost_row(leg, cell, basis, c["cash"], c["api_equivalent"]))
    with (csv_dir / "breakeven.csv").open("w", newline="") as fh:
        w = csv.writer(fh)
        w.writerow(["leg", "kind", "tasks_per_month", "usd_per_month"])
        for leg, curve in analysis["break_even"]["curves"].items():
            for pt in curve["points"]:
                w.writerow([leg, curve["kind"], pt["tasks"], pt["usd"]])


def write_csvs(analysis: dict[str, Any], csv_dir: Path) -> None:
    csv_dir.mkdir(parents=True, exist_ok=True)
    _write_routing_csvs(analysis, csv_dir)
    _write_cost_csvs(analysis, csv_dir)


def analyze(
    ladder_rows: list[dict[str, Any]],
    run_rows: list[dict[str, Any]],
    plan_fee: dict[str, float],
    manual_capacity: dict[str, float],
) -> dict[str, Any]:
    cost = cost_by_leg(run_rows)
    capacity = subscription_capacity(run_rows, plan_fee, manual_capacity)
    return {
        "routing": {"ladder": ladder_routing(ladder_rows), "runs": run_routing(run_rows)},
        "cost": cost,
        "subscription_capacity": capacity,
        "break_even": break_even(cost, capacity, plan_fee),
        "notes": [
            "Subscription legs are priced as API-equivalent list cost; cash paid is the seat fee, never per task.",
            "Router legs with cost_basis router-reported carry the gateway's own bill as cash; other API legs are list-price estimates.",
            "chi-square p-values are from the regularised incomplete gamma function; NMI is I(cell;model)/sqrt(H(cell)H(model)).",
        ],
    }


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    ap.add_argument("--suite", default=str(ROOT / "tasks" / "router-v1"))
    ap.add_argument("--ladder", action="append", default=[], type=Path)
    ap.add_argument("--runs", action="append", default=[], type=Path)
    ap.add_argument("--leg", action="append", default=[])
    ap.add_argument("--plan-fee", action="append", default=[], help="LEG=USD per month")
    ap.add_argument(
        "--manual-capacity", action="append", default=[], help="LEG=USD list-price per week"
    )
    ap.add_argument("--out", default="analysis.json", type=Path)
    ap.add_argument("--csv-dir", type=Path)
    args = ap.parse_args(argv)
    if len(args.runs) != len(args.leg):
        ap.error("--runs and --leg must be given in pairs")
    cells = load_cells(Path(args.suite))
    ladder_rows = load_ladder(args.ladder)
    run_rows = load_runs(list(zip(args.runs, args.leg, strict=True)), cells)
    analysis = analyze(ladder_rows, run_rows, _kv(args.plan_fee), _kv(args.manual_capacity))
    args.out.write_text(json.dumps(analysis, indent=2) + "\n")
    if args.csv_dir:
        write_csvs(analysis, args.csv_dir)
    for source in ("ladder", "runs"):
        for group, data in analysis["routing"][source].items():
            t = data.get("cell_test") or {}
            print(
                f"{source:7} {group:14} n={data.get('n', 0):4} models={data.get('distinct_models', 0)} "
                f"cell chi2 p={t.get('p_value', float('nan')):.3g} nmi={t.get('nmi', 0):.3f}"
            )
    for leg, data in analysis["cost"].items():
        print(
            f"cost    {leg:14} n={data['cash']['n']:3} pass={data['cash']['pass_rate']} "
            f"cash/task={data['cash']['mean_usd']} api-equiv/task={data['api_equivalent']['mean_usd']}"
        )
    print(f"wrote {args.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
