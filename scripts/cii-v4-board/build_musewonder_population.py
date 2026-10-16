"""Build the Code quality judging population for the Muse Wonder Full Frontier v4 sweep.

Reads every finished run under runs-effort-musewonder-full-v2/<level>/ and writes
the comparison record the v3.25 runner freezes against, in the same row shape as
the Muse Spark 1.3 record (build_muse_population.py). Muse Wonder Full ran
through Muse Code 1.4.6 in Contributor mode with Muse Code's skill reminder off
(docs/DECISIONS.md, 2026-10-12 and 2026-10-13), solver
``muse-code:muse-wonder-full-1.0-contributor``, Low and Medium only, from the
tagged run worktree bench/2026-10-13-musewonder-full-v2-low.

Unfinished runs, runs whose evidence cannot be rebuilt from the saved patch and
runs that changed no recognized source file are listed under "excluded" with
the reason. A task and level with no finished run is "missing" with the
reason. Interrupted attempts moved to the sweep's quota-aborted/ folder are
outside <level>/ and are never read.

Every row is checked against the sweep's run conditions before it is kept:
the solver id, the requested effort, the Muse Code version, a clean harness
checkout and a task hash equal to the suite lock. Any mismatch stops the build.
``source_directory`` is the resolved path, so the record names the same run
directory whichever checkout builds it.

When this runs from a worktree, link the sweep folder in first:
``ln -s ~/dev/VulcanBench/runs-effort-musewonder-full-v2 runs-effort-musewonder-full-v2``.

    python scripts/cii-v4-board/build_musewonder_population.py           # write comparison.json
    python scripts/cii-v4-board/build_musewonder_population.py --check   # rebuild, compare, write nothing
    python scripts/cii-v4-board/build_musewonder_population.py --top-up  # v3.26.1: only the missing runs

``--top-up`` is for the v3.26.1 round (named v3.25.1 before the judge seat
changed): it reads the missing list frozen in
runs-code-quality-maintenance-v3.26/protocol.json and keeps only those runs.
They ran through Muse Code 1.4.9 in runs-effort-musewonder-full-v2-149, which
continues the 1.4.6 root (docs/DECISIONS.md, 2026-10-16), so the top-up checks
that root's conditions: the 1.4.9 binary and its recorded allowance for Spark 1.3
on goal and verify reminder observers, whose usage each row records. It writes
comparison-topup.json beside the main record, which it never touches.
"""

from __future__ import annotations

import argparse
import json
import sys
import tempfile
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
from harness import maintenance_review_v2 as v2  # noqa: E402
from harness import maintenance_review_v3 as v3  # noqa: E402
from harness import retrospective_judging as base  # noqa: E402
from harness.solver_receipts import solver_receipt  # noqa: E402

TASKS = ROOT / "tasks/coding-intelligence-index-v4"
SUITE_LOCK = TASKS / "suite.lock.json"
RESULTS = ROOT / "docs/results/swe-v4-musewonder-full-2026-10"
OUTPUT = RESULTS / "comparison.json"
TOPUP_OUTPUT = RESULTS / "comparison-topup.json"
RUNS = "runs-effort-musewonder-full-v2"
BASE_PROTOCOL = ROOT / "runs-code-quality-maintenance-v3.26/protocol.json"
TOPUP_RUNS = "runs-effort-musewonder-full-v2-149"
TOPUP_SOLVER_CLI = "Muse Code 1.4.9 (1.4.9-R6297.1)"
TOPUP_REMINDER_MODELS = ["muse-spark-1.3"]
MODEL = "musewonderfull"
LEVELS = ("low", "medium")
MODELS = {MODEL: (RUNS, LEVELS)}
SWEEP_MODELS = {MODEL: "muse-code:muse-wonder-full-1.0-contributor"}
SOLVER_CLI = "Muse Code 1.4.6 (1.4.6-R6000.1)"
TASK_IDS = sorted(p.name for p in TASKS.iterdir() if p.is_dir() and p.name.startswith("legacy-"))
MISSING_REASON = "No finished run for this task and level"
_QUOTA = (
    "Not yet run: the Muse Code subscription's weekly limit ran out during the Medium level "
    "(Muse returned 429 'Subscription quota exhausted' at 2026-10-14 22:20 UTC; the window "
    "resets 2026-10-19 00:00 UTC). The task runs at Medium after the reset and is judged in "
    "the v3.25.1 top-up round."
)
KNOWN_GAPS = {
    (MODEL, "medium", "legacy-cellarcore-binary-parity"): _QUOTA
    + " The interrupted attempt is kept in runs-effort-musewonder-full-v2/quota-aborted/ "
    "and is not judged.",
    (MODEL, "medium", "legacy-paddockcore-binary-parity"): _QUOTA,
}


def solver_conditions(
    sweep_protocol: Path, solver_cli: str = SOLVER_CLI, top_up: bool = False
) -> dict:
    """The sweep's own frozen record of its run conditions, checked against this population."""
    raw = sweep_protocol.read_bytes()
    protocol = json.loads(raw)
    expected = {
        "model": SWEEP_MODELS[MODEL],
        "harness_version": solver_cli,
        "contributor_mode": "on",
        "muse_settings_plugins": {"skill-reminder": {"enabled": False}},
    }
    if top_up:
        expected["reminder_observer_models"] = TOPUP_REMINDER_MODELS
    for key, value in expected.items():
        if protocol.get(key) != value:
            raise ValueError(f"Sweep protocol {key} is {protocol.get(key)!r}, expected {value!r}")
    extra = {}
    if top_up:
        continues = protocol.get("continues") or {}
        if Path(continues.get("root", "")).name != RUNS:
            raise ValueError(f"Top-up sweep does not continue {RUNS}")
        extra = {
            "continues": continues,
            "reminder_observer_rule": protocol["reminder_observer_rule"],
        }
    return {
        **expected,
        "cli_model": protocol["cli_model"],
        "binary_sha256": protocol["binary_sha256"],
        "muse_settings_sha256": protocol["muse_settings_sha256"],
        "sweep_protocol_sha256": base.digest(raw),
        **extra,
    }


def reminder_usage(run: Path) -> dict:
    """Tokens the run's goal and verify reminder observers spent on another model, from its trace."""
    for line in (run / "trace.jsonl").read_text().splitlines():
        event = json.loads(line)
        if event.get("type") == "muse_usage":
            return (event.get("data") or {}).get("auxiliary") or {}
    raise ValueError(f"{run}: no muse_usage event in trace.jsonl")


def _receipt(run: Path, summary: dict) -> dict:
    try:
        return solver_receipt(run, summary)
    except (ValueError, KeyError, OSError) as exc:
        return {"error": f"{type(exc).__name__}: {exc}"}


def _lock_hashes() -> dict[str, str]:
    tasks = json.loads(SUITE_LOCK.read_text())["tasks"]
    return {name: (h if isinstance(h, str) else h["task_hash"]) for name, h in tasks.items()}


def _check_conditions(
    run: Path, summary: dict, model: str, level: str, lock: dict, solver_cli: str = SOLVER_CLI
) -> None:
    """Stop on any run that was not made under the sweep's recorded conditions."""
    if summary["model"] != SWEEP_MODELS[model]:
        raise ValueError(f"{run}: unexpected solver {summary['model']}")
    requested = (summary.get("effort") or {}).get("requested")
    if requested != level:
        raise ValueError(f"{run}: effort {requested!r} in a {level} folder")
    version = (summary.get("cli_agent") or {}).get("harness_version")
    if version != solver_cli:
        raise ValueError(f"{run}: solver CLI {version!r}, expected {solver_cli!r}")
    harness = ((summary.get("source") or {}).get("harness")) or {}
    if harness.get("dirty") is not False:
        raise ValueError(f"{run}: run from a dirty or unrecorded checkout")
    if summary.get("task_hash") != lock.get(summary["task_id"]):
        raise ValueError(f"{run}: task hash differs from the suite lock")


def _exclusion(model: str, level: str, run: Path, summary: dict, reason: str) -> dict:
    return {
        "model": model,
        "effort": level,
        "task": summary["task_id"],
        "run_id": run.name,
        "reason": reason,
        "finished": summary.get("finished"),
        "duration_s": summary["duration_s"],
        "functional": summary["scores"]["functional"],
    }


def _evidence_rebuilds(run: Path, summary: dict, data: dict) -> bool:
    row = {
        "run_id": run.name,
        "source_directory": str(run.resolve()),
        "task": summary["task_id"],
        "source_hashes": data["source_hashes"],
    }
    try:
        with tempfile.TemporaryDirectory() as scratch, v3._v2_writing_to(Path(scratch)):
            v2.evidence_for(row)
    except Exception:
        return False
    return True


def build(wanted: set[tuple[str, str, str]] | None = None, top_up: bool = False) -> dict[str, Any]:
    """The population record; ``wanted`` limits it to those (model, effort, task) cells.

    ``top_up`` reads the 1.4.9 continuation root instead of the 1.4.6 one.
    """
    lock = _lock_hashes()
    rows, excluded, missing, tags = [], [], [], set()
    solver_cli = TOPUP_SOLVER_CLI if top_up else SOLVER_CLI
    models = {MODEL: (TOPUP_RUNS, LEVELS)} if top_up else MODELS
    for model, (root, levels) in models.items():
        for level in levels:
            seen = set()
            for run in sorted((ROOT / root / level).glob("legacy-*/")):
                summary_path = run / "summary.json"
                if not summary_path.exists():
                    continue
                summary = json.loads(summary_path.read_text())
                if wanted is not None and (model, level, summary["task_id"]) not in wanted:
                    continue  # judged under v3.25 already
                _check_conditions(run, summary, model, level, lock, solver_cli)
                if summary["task_id"] in seen:
                    raise ValueError(f"Two finished runs for {summary['task_id']} at {level}")
                seen.add(summary["task_id"])
                tags.add(summary["source"]["harness"].get("describe"))
                try:
                    data = base.inputs(run, TASKS)
                except ValueError as exc:
                    excluded.append(_exclusion(model, level, run, summary, str(exc)))
                    continue
                if not _evidence_rebuilds(run, summary, data):
                    excluded.append(
                        _exclusion(
                            model,
                            level,
                            run,
                            summary,
                            "Evidence not reconstructible from the saved patch",
                        )
                    )
                    continue
                scores = summary["scores"]
                if scores["quality"] is None or scores["security"] is None:
                    excluded.append(
                        _exclusion(
                            model,
                            level,
                            run,
                            summary,
                            "No recognized source file changed; nothing to judge",
                        )
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
                        "solver_receipt": _receipt(run, summary),
                        "source_directory": str(run.resolve()),
                        "source_hashes": data["source_hashes"],
                        "started_at": summary["started_at"],
                        "finished_at": summary["finished_at"],
                        "solver_cli_version": summary["cli_agent"]["harness_version"],
                        "api_equivalent_cost_usd": summary["economics"]["api_equivalent_cost_usd"],
                        **({"reminder_observer_usage": reminder_usage(run)} if top_up else {}),
                    }
                )
            for task in TASK_IDS:
                if task in seen or (wanted is not None and (model, level, task) not in wanted):
                    continue
                reason = KNOWN_GAPS.get((model, level, task), MISSING_REASON)
                missing.append({"model": model, "effort": level, "task": task, "reason": reason})
    cells: dict[str, int] = {}
    for r in rows:
        cells[f"{r['model']}/{r['effort']}"] = cells.get(f"{r['model']}/{r['effort']}", 0) + 1
    described = (
        "Muse Wonder Full through Muse Code 1.4.9 (Contributor mode, skill reminder off; goal and "
        "verify reminder observers on Muse Spark 1.3, recorded per row)"
        if top_up
        else "Muse Wonder Full through Muse Code 1.4.6 (Contributor mode, skill reminder off)"
    )
    return {
        "suite": "coding-intelligence-index-v4",
        "models": {MODEL: described},
        "levels": {m: list(v[1]) for m, v in models.items()},
        "solver": {
            **solver_conditions(ROOT / models[MODEL][0] / "protocol.json", solver_cli, top_up),
            "run_from": sorted(t for t in tags if t),
        },
        "cells": cells,
        "rows": rows,
        "excluded": excluded,
        "missing": missing,
    }


def render(record: dict) -> str:
    return json.dumps(record, indent=1, sort_keys=True) + "\n"


def frozen_missing(protocol_path: Path = BASE_PROTOCOL) -> set[tuple[str, str, str]]:
    """The cells v3.26 froze as missing (v3.25's list, verbatim): the only runs the top-up may add."""
    population = json.loads(protocol_path.read_text())["population"]
    return {(m["model"], m["effort"], m["task"]) for m in population["missing"]}


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--check", action="store_true", help="rebuild and compare; write nothing")
    mode.add_argument("--top-up", action="store_true", help="v3.26.1: only the frozen missing runs")
    args = parser.parse_args(argv)
    if args.top_up:
        record, output = build(frozen_missing(), top_up=True), TOPUP_OUTPUT
    else:
        record, output = build(), OUTPUT
    text = render(record)
    if args.check:
        if not output.exists() or output.read_text() != text:
            print(f"{output.relative_to(ROOT)} differs from a fresh build", file=sys.stderr)
            return 1
        print(f"{output.relative_to(ROOT)} matches a fresh build")
        return 0
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(text)
    rows, excluded, missing = record["rows"], record["excluded"], record["missing"]
    print(
        f"{len(rows)} rows, {len(excluded)} excluded, {len(missing)} missing; cells: {record['cells']}"
    )
    for e in excluded:
        print("excluded:", e["model"], e["effort"], e["task"], e["reason"])
    for e in missing:
        print("missing:", e["model"], e["effort"], e["task"])
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
