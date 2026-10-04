"""Build the judging population for the Grok 4.7 effort sweep in Cursor.

Reads every finished run under runs-effort-grok47-cursor/<level>/ and writes
the comparison record the Code quality runner freezes against, in the same row
shape as the GPT-6.1 Sol record. Cursor exposes four effort variants (low,
medium, high, extra-high), so there is no max level. Unfinished runs (the
3-hour timeouts) are listed under "excluded" with the reason; a task and level
with no attempt at all is listed under "missing". Directories without a
summary (Cursor infrastructure failures the harness retried) are not runs.

Cursor's run summaries record 0 tokens because the adapter does not read the
stream's usage block. The solver receipt here reads that block from the
stream's single result event: input, output, cache reads and cache writes,
summed as raw tokens.

    python scripts/cii-v4-board/build_grok47cursor_population.py
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
OUTPUT = ROOT / "docs/results/swe-v4-grok47-cursor-2026-10/comparison.json"
LEVELS = ("low", "medium", "high", "extra-high")
MODELS = {"grok47cursor": ("runs-effort-grok47-cursor", LEVELS)}
SWEEP_MODELS = {
    "grok47cursor": {
        "low": "cursor:grok-4.7-low",
        "medium": "cursor:grok-4.7-medium",
        "high": "cursor:grok-4.7-high",
        "extra-high": "cursor:grok-4.7-xhigh",
    }
}
CURSOR_KEYS = ("inputTokens", "outputTokens", "cacheReadTokens", "cacheWriteTokens")
TASK_IDS = sorted(p.name for p in TASKS.iterdir() if p.is_dir() and p.name.startswith("legacy-"))
MISSING_REASON = "No finished run for this task and level"


def solver_receipt(run: Path) -> dict:
    """Raw tokens from the Cursor stream's single successful result event."""
    path = run / "cli-agent-stream.jsonl"
    events = [json.loads(line) for line in path.read_text().splitlines() if line.strip()]
    results = [e for e in events if e.get("type") == "result"]
    if len(results) != 1 or results[0].get("subtype") != "success" or results[0].get("is_error"):
        raise ValueError(f"{run}: expected one successful Cursor result")
    usage = results[0]["usage"]
    for key in CURSOR_KEYS:
        if type(usage.get(key)) is not int or usage[key] < 0:
            raise ValueError(f"{run}: invalid Cursor usage {key}")
    return {
        "raw_tokens": sum(usage[key] for key in CURSOR_KEYS),
        "usage": {key: usage[key] for key in CURSOR_KEYS},
        "result_receipts": 1,
        "historical_summary_unit": "Cursor stream usage: input, output, cache reads, cache writes",
        "stream_sha256": digest(path.read_bytes()),
    }


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
                if summary["model"] != SWEEP_MODELS[model][level]:
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
                        "solver_receipt": solver_receipt(run),
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
        "models": {"grok47cursor": "Grok 4.7 in Cursor"},
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
