"""Build the Code quality judging population for the Claude Opus 5.5 Frontier v4 sweep.

Reads every finished run under runs-effort-opus55/<level>/ and writes the
comparison record the v3.15 runner freezes against, in the same row shape as
the GPT-5.6 Sol record. Unfinished runs are listed under "excluded" with the
reason; a task and level with no attempt at all is listed under "missing".

Opus 5.5 ran with Claude Code's refusal fallback on (docs/DECISIONS.md,
2026-09-24, the Artificial Analysis convention), so some runs were partly
served by claude-opus-4-8. Every run counts. Each row records the truth:
``fallback`` is True when any assistant reply came from another model, and
``fallback_replies`` counts replies by model from the stream. The solver
receipt parser has no Opus 5.5 branch yet; a receipt error is recorded on the
row and affects economics only, never judging.

    python scripts/cii-v4-board/build_opus55_population.py
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
from collections import Counter  # noqa: E402

from harness import retrospective_judging as base  # noqa: E402
from harness.solver_receipts import solver_receipt  # noqa: E402

TASKS = ROOT / "tasks/coding-intelligence-index-v4"
OUTPUT = ROOT / "docs/results/swe-v4-opus55-2026-09/comparison.json"
MODELS = {"opus55": ("runs-effort-opus55", tuple(base.LEVELS))}
SWEEP_MODELS = {"opus55": "claude-code:claude-opus-5-5"}
TASK_IDS = sorted(p.name for p in TASKS.iterdir() if p.is_dir() and p.name.startswith("legacy-"))
MISSING_REASON = "No finished run for this task and level"


def _replies(run: Path) -> dict[str, int]:
    """Assistant replies by serving model, straight from the CLI stream."""
    counts: Counter = Counter()
    for line in (run / "cli-agent-stream.jsonl").open(errors="replace"):
        if '"assistant"' not in line:
            continue
        event = json.loads(line)
        if event.get("type") == "assistant":
            counts[event["message"].get("model")] += 1
    return dict(sorted(counts.items()))


def _receipt(run: Path, summary: dict) -> dict:
    try:
        return solver_receipt(run, summary)
    except (ValueError, KeyError, OSError) as exc:
        return {"error": f"{type(exc).__name__}: {exc}"}


def main() -> None:
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
                replies = _replies(run)
                rows.append(
                    {
                        "model": model,
                        "effort": level,
                        "task": summary["task_id"],
                        "run_id": run.name,
                        "functional": scores["functional"],
                        "quality": scores["quality"],
                        "security": scores["security"],
                        "fallback": any(k != "claude-opus-5-5" for k in replies),
                        "fallback_replies": replies,
                        "duration_s": summary["duration_s"],
                        "reported_tokens": summary["total_tokens"],
                        "solver_receipt": _receipt(run, summary),
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
        "models": {"opus55": "Claude Opus 5.5 in Claude Code (refusal fallback on)"},
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
