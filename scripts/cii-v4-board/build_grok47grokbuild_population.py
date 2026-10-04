"""Build the judging population for the Grok 4.7 effort sweep in Grok Build.

The Grok Build counterpart of build_grok47cursor_population.py: reads every
finished run under runs-effort-grok47-grokbuild/<level>/ and writes the
comparison record the Code quality runner freezes against, in the same row
shape. Grok Build runs model grok-4.7 with --effort (extra-high maps to
xhigh), so the level is checked against the summary's requested effort.
Unfinished runs (the 3-hour timeouts) are listed under "excluded"; a task and
level with no attempt is listed under "missing". Directories without a
summary are infrastructure failures the harness retried, not runs.

The solver receipt reads the stream's end event usage (input, cache reads,
cache writes, output, the same raw sum as the Cursor record) and must equal
the run summary's total, which the adapter folds from that event.

    python scripts/cii-v4-board/build_grok47grokbuild_population.py
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
from harness import retrospective_judging as base  # noqa: E402
from harness.retrospective_judging import digest  # noqa: E402

TASKS = ROOT / "tasks/coding-intelligence-index-v4"
OUTPUT = ROOT / "docs/results/swe-v4-grok47-grokbuild-2026-10/comparison.json"
LEVELS = ("low", "medium", "high", "extra-high")
MODELS = {"grok47grokbuild": ("runs-effort-grok47-grokbuild", LEVELS)}
SWEEP_MODEL = "grok-build:grok-4.7"
USAGE_KEYS = (
    "input_tokens",
    "cache_read_input_tokens",
    "cache_creation_input_tokens",
    "output_tokens",
)
TASK_IDS = sorted(p.name for p in TASKS.iterdir() if p.is_dir() and p.name.startswith("legacy-"))
MISSING_REASON = "No finished run for this task and level"


def solver_receipt(run: Path, summary: dict) -> dict:
    """Raw tokens from the Grok Build stream's single end event, checked against the summary."""
    path = run / "cli-agent-stream.jsonl"
    events = [json.loads(line) for line in path.read_text().splitlines() if line.strip()]
    ends = [e for e in events if e.get("type") == "end"]
    if len(ends) != 1:
        raise ValueError(f"{run}: expected one Grok Build end event, found {len(ends)}")
    usage = ends[0].get("usage") or {}
    for key in USAGE_KEYS:
        if type(usage.get(key, 0)) is not int or usage.get(key, 0) < 0:
            raise ValueError(f"{run}: invalid Grok Build usage {key}")
    raw = sum(usage.get(key, 0) for key in USAGE_KEYS)
    if raw != summary["total_tokens"]:
        raise ValueError(f"{run}: receipt {raw} differs from summary {summary['total_tokens']}")
    return {
        "raw_tokens": raw,
        "usage": {key: usage.get(key, 0) for key in USAGE_KEYS},
        "result_receipts": 1,
        "historical_summary_unit": "Grok Build end-event usage: input, cache reads, cache writes, output",
        "stream_sha256": digest(path.read_bytes()),
    }


def main() -> None:  # noqa: PLR0912, one check per run field
    rows, excluded, missing = [], [], []
    for model, (root, levels) in MODELS.items():
        for level in levels:
            seen = set()
            for run in sorted((ROOT / root / level).glob("legacy-*/")):
                summary_path = run / "summary.json"
                if not summary_path.exists():
                    continue
                summary = json.loads(summary_path.read_text())
                if summary["model"] != SWEEP_MODEL:
                    raise ValueError(f"{run}: unexpected solver {summary['model']}")
                if summary["effort"]["requested"] != level:
                    raise ValueError(f"{run}: effort {summary['effort']['requested']} in {level}")
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
        "models": {"grok47grokbuild": "Grok 4.7 in Grok Build"},
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
