"""Build the judging population for the Muse Spark 1.3 Contributor-tier effort sweep.

Reads every finished run under runs-muse13-contributor-cii-v4-v2/<level>/ and writes the
comparison record the Code quality runner freezes against, in the same row
shape as the Sol record. Unfinished runs are listed under
"excluded" with the reason; a task and level with no attempt at all is listed
under "missing" with the reason (none expected).

    python scripts/cii-v4-board/build_muse_population.py
"""

from __future__ import annotations

import json
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
from harness import maintenance_review_v2 as v2  # noqa: E402
from harness import maintenance_review_v3 as v3  # noqa: E402
from harness import retrospective_judging as base  # noqa: E402
from harness.solver_receipts import solver_receipt  # noqa: E402

TASKS = ROOT / "tasks/coding-intelligence-index-v4"
OUTPUT = ROOT / "docs/results/swe-v4-muse-2026-09/comparison.json"
MODELS = {
    "muse": (
        "runs-muse13-contributor-cii-v4-v2",
        ("minimal", "low", "medium", "high", "extra-high"),
    )
}
SWEEP_MODELS = {"muse": "muse-code:muse-spark-1.3-contributor"}
TASK_IDS = sorted(p.name for p in TASKS.iterdir() if p.is_dir() and p.name.startswith("legacy-"))
MISSING_REASON = "No finished run for this task and level"


def main() -> None:  # noqa: PLR0912, one branch per exclusion reason
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
                row = {
                    "run_id": run.name,
                    "source_directory": str(run),
                    "task": summary["task_id"],
                    "source_hashes": data["source_hashes"],
                }
                try:
                    with tempfile.TemporaryDirectory() as scratch, v3._v2_writing_to(Path(scratch)):
                        v2.evidence_for(row)
                except Exception as exc:
                    # The judge sees the final files rebuilt from the saved patch; a patch the
                    # pipeline cannot apply (a binary scratch file without an index line) has no
                    # judgeable evidence, so the run is excluded with the reason.
                    excluded.append(
                        {
                            "model": model,
                            "effort": level,
                            "task": summary["task_id"],
                            "run_id": run.name,
                            "reason": f"Evidence not reconstructible from the saved patch: {type(exc).__name__}",
                            "finished": summary.get("finished"),
                            "duration_s": summary["duration_s"],
                            "functional": scores["functional"],
                        }
                    )
                    continue
                if scores["quality"] is None or scores["security"] is None:
                    # No recognized source file changed: the run produced nothing to
                    # judge (the automated quality and security metrics are undefined
                    # by construction), so it is excluded like an unfinished run.
                    excluded.append(
                        {
                            "model": model,
                            "effort": level,
                            "task": summary["task_id"],
                            "run_id": run.name,
                            "reason": "No recognized source file changed; nothing to judge",
                            "finished": summary.get("finished"),
                            "duration_s": summary["duration_s"],
                            "functional": scores["functional"],
                        }
                    )
                    continue
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
                if task not in seen:
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
        "models": {"muse": "Muse Spark 1.3 (Contributor tier) through Muse Code"},
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
