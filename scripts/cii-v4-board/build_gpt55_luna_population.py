"""Build the judging population for the GPT-5.5 versus GPT-5.6 Luna comparison.

Reads every finished run under runs-effort-gpt55/<level>/ and
runs-effort-luna/<level>/ and writes the comparison record the Code quality
runner freezes against, in the same row shape as the Astra and Fable record
minus the retired review fields. Unfinished runs (a patch but no clean end,
for example a run stopped at the wall-clock cap) are listed under
"excluded" with the reason and are not judged.

    python scripts/cii-v4-board/build_gpt55_luna_population.py
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
OUTPUT = ROOT / "docs/results/swe-v4-gpt55-luna-2026-09/comparison.json"
MODELS = {
    "gpt55": ("runs-effort-gpt55", ("low", "medium", "high", "extra-high")),
    "luna": ("runs-effort-luna", ("low", "medium", "high", "extra-high", "max")),
}
SWEEP_MODELS = {"gpt55": "codex:gpt-5.5", "luna": "codex:gpt-5.6-luna"}


def main() -> None:
    rows, excluded = [], []
    for model, (root, levels) in MODELS.items():
        for level in levels:
            for run in sorted((ROOT / root / level).glob("legacy-*/")):
                summary_path = run / "summary.json"
                if not summary_path.exists():
                    continue
                summary = json.loads(summary_path.read_text())
                if summary["model"] != SWEEP_MODELS[model]:
                    raise ValueError(f"{run}: unexpected solver {summary['model']}")
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
    cells = {}
    for r in rows:
        cells.setdefault(f"{r['model']}/{r['effort']}", 0)
        cells[f"{r['model']}/{r['effort']}"] += 1
    dupes = len(rows) - len({(r["model"], r["effort"], r["task"]) for r in rows})
    if dupes:
        raise ValueError(f"{dupes} duplicate model/effort/task rows")
    record = {
        "suite": "coding-intelligence-index-v4",
        "models": {
            "gpt55": "GPT-6 series predecessor GPT-5.5 in Codex",
            "luna": "GPT-5.6 Luna in Codex",
        },
        "levels": {m: list(v[1]) for m, v in MODELS.items()},
        "cells": cells,
        "rows": rows,
        "excluded": excluded,
    }
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_text(json.dumps(record, indent=1, sort_keys=True) + "\n")
    print(f"{len(rows)} rows, {len(excluded)} excluded; cells: {cells}")
    for e in excluded:
        print("excluded:", e["model"], e["effort"], e["task"], e["reason"])


if __name__ == "__main__":
    main()
