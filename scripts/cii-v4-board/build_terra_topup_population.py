"""Build the v3.6.1 top-up population: Terra runs recorded missing under v3.6 that now exist.

Reads every finished run under runs-effort-terra/<level>/ and writes the
comparison record the Code quality runner freezes against, in the same row
shape as the GPT-5.5 and Luna record. Unfinished runs are listed under
"excluded" with the reason; a task and level with no attempt at all is listed
under "missing" with the reason (the Codex subscription quota window).

    python scripts/cii-v4-board/build_terra_topup_population.py
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
from harness import retrospective_judging as base  # noqa: E402
from harness.solver_receipts import solver_receipt  # noqa: E402

TASKS = ROOT / "tasks/coding-intelligence-index-v4"
OUTPUT = ROOT / "docs/results/swe-v4-terra-2026-09/comparison-topup.json"
BASE = ROOT / "runs-code-quality-maintenance-v3.6"
MODELS = {"terra": ("runs-effort-terra", tuple(base.LEVELS))}
SWEEP_MODELS = {"terra": "codex:gpt-5.6-terra"}
TASK_IDS = sorted(p.name for p in TASKS.iterdir() if p.is_dir() and p.name.startswith("legacy-"))
MISSING_REASON = (
    "Not yet run: every launch from 2026-09-15 06:00 PDT was refused by the Codex API with "
    "'You've hit your usage limit ... try again at Sep 19th, 2026 1:10 AM' before any work; one refused "
    "attempt is kept under runs-effort-terra/max-quota-refused/ as evidence"
)


def main() -> None:  # noqa: PLR0912, one linear scan
    wanted = {
        (m["model"], m["effort"], m["task"])
        for m in json.loads((BASE / "protocol.json").read_text())["population"]["missing"]
    }
    rows, excluded, missing = [], [], []
    for model, (root, levels) in MODELS.items():
        for level in levels:
            seen = set()
            for run in sorted((ROOT / root / level).glob("legacy-*/")):
                summary_path = run / "summary.json"
                if not summary_path.exists():
                    continue
                summary = json.loads(summary_path.read_text())
                if summary["model"] != SWEEP_MODELS[model]:
                    raise ValueError(f"{run}: unexpected solver {summary['model']}")
                if (model, level, summary["task_id"]) not in wanted:
                    continue  # judged under v3.6 already
                seen.add(summary["task_id"])
                try:
                    data = base.inputs(run, TASKS)
                except ValueError as exc:
                    excluded.append(
                        {
                            "model": model,
                            "effort": level,
                            "task": summary["task_id"],
                            "run_id": run.name,
                            "reason": str(exc),
                            "finished": summary.get("finished"),
                            "duration_s": summary["duration_s"],
                            "functional": summary["scores"]["functional"],
                        }
                    )
                    continue
                scores = summary["scores"]
                rows.append(
                    {
                        "model": model,
                        "effort": level,
                        "task": summary["task_id"],
                        "run_id": run.name,
                        "functional": scores["functional"],
                        "quality": scores["quality"],
                        "security": scores["security"],
                        "fallback": False,
                        "duration_s": summary["duration_s"],
                        "reported_tokens": summary["total_tokens"],
                        "solver_receipt": solver_receipt(run, summary),
                        "source_directory": str(run),
                        "source_hashes": data["source_hashes"],
                        "started_at": summary["started_at"],
                        "finished_at": summary["finished_at"],
                        "solver_cli_version": summary["cli_agent"]["harness_version"],
                        "api_equivalent_cost_usd": summary["economics"]["api_equivalent_cost_usd"],
                    }
                )
            for task in TASK_IDS:
                if (model, level, task) in wanted and task not in seen:
                    missing.append(
                        {"model": model, "effort": level, "task": task, "reason": MISSING_REASON}
                    )
    cells = {}
    for r in rows:
        cells.setdefault(f"{r['model']}/{r['effort']}", 0)
        cells[f"{r['model']}/{r['effort']}"] += 1
    dupes = len(rows) - len({(r["model"], r["effort"], r["task"]) for r in rows})
    if dupes:
        raise ValueError(f"{dupes} duplicate model/effort/task rows")
    record = {
        "suite": "coding-intelligence-index-v4",
        "models": {"terra": "GPT-5.6 Terra in Codex"},
        "top_up_of": "runs-code-quality-maintenance-v3.6",
        "levels": {m: list(v[1]) for m, v in MODELS.items()},
        "cells": cells,
        "rows": rows,
        "excluded": excluded,
        "missing": missing,
    }
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_text(json.dumps(record, indent=1, sort_keys=True) + "\n")
    print(f"{len(rows)} rows, {len(excluded)} excluded, {len(missing)} missing; cells: {cells}")
    for e in excluded:
        print("excluded:", e["model"], e["effort"], e["task"], e["reason"])
    for e in missing:
        print("missing:", e["model"], e["effort"], e["task"])


if __name__ == "__main__":
    main()
