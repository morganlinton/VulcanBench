"""Sequential, resumable Muse Spark 1.3 / Muse Code sweep. Stop on any error."""

import argparse
import fcntl
import hashlib
import json
import os
import shutil
import subprocess
import sys
from datetime import UTC, datetime
from pathlib import Path

from harness.agent.cli_agents import get_cli_agent_adapter
from harness.agent.loop import run_agent
from harness.agent.muse_code import STREAM_IDLE_TIMEOUT_SECS, pinned_executable
from harness.tasks import load_task, task_hash

ROOT = Path(__file__).resolve().parents[2]
SUITE = ROOT / "tasks/coding-intelligence-index-v4"
OUT = ROOT / "runs-muse13-contributor-cii-v4-v2"
LEVELS = ["minimal", "low", "medium", "high", "extra-high"]
MODEL = "muse-code:muse-spark-1.3-contributor"
BINARY = Path.home() / ".local/bin/muse-bin-1.0.3-R2198.1"
BINARY_SHA256 = "4c0f960028b603174af7df7bd5051d8c35d6c1aa372a37d18bc770926a0577a7"
VERSION = "Muse Code 1.0.3 (1.0.3-R2198.1)"


def configure_binary():
    os.environ["VULCANBENCH_MUSE_BINARY"] = str(BINARY)
    os.environ["VULCANBENCH_MUSE_SHA256"] = BINARY_SHA256
    return pinned_executable()


def save_protocol(path, protocol):
    """Never blend changed solver configurations or overwrite historical protocols."""
    if path.exists():
        assert json.loads(path.read_text()) == protocol, "Protocol changed; use a separate run root"
    else:
        path.write_text(json.dumps(protocol, indent=2) + "\n")


def now():
    return datetime.now(UTC).isoformat()


def grader_preflight():
    """Fail before spending subscription usage if grader tools cannot run."""
    os.environ["PATH"] = str(ROOT / ".venv/bin") + os.pathsep + os.environ.get("PATH", "")
    for tool in ("python", "ruff", "bandit"):
        resolved = shutil.which(tool)
        assert resolved and Path(resolved).parent == ROOT / ".venv/bin", (
            f"Wrong grader tool: {tool}={resolved}"
        )
    subprocess.run(
        ["python", "-c", "import pytest, radon, bandit; print('Grader imports OK')"], check=True
    )
    subprocess.run(["ruff", "--version"], check=True)


def result_summary(result):
    """run_agent returns an envelope, not the summary itself."""
    summary = result["summary"]
    assert result["run_id"] == summary["run_id"]
    scores = summary["scores"]
    details = scores.get("metric_details") or {}
    no_source = (details.get("quality") or {}).get("reason") == "no recognized source files changed"
    if no_source and scores["functional"] == 0.0:
        # The harness scores a run whose patch touches no recognized source
        # file as functional 0.0 and skips the analyzers. That is a scored
        # failure, not a grading gap.
        return summary
    if scores.get("budget_exceeded"):
        # The harness scores a run that exhausts its wall-clock budget as a
        # failure (functional 0.0) and skips the quality and security
        # analyzers by design. Accept it as a scored result rather than
        # pausing for grading inspection.
        assert scores["functional"] == 0.0 and scores["total"] == 0.0, (
            "Unexpected budget-exceeded scores"
        )
        return summary
    for name in ("functional", "quality", "security"):
        assert scores[name] is not None, f"Missing {name}; pause for grading inspection"
    return summary


def record_result(summary, level):
    event = {
        "at": now(),
        "effort": level,
        "task": summary["task_id"],
        "run_id": summary["run_id"],
        "functional": summary["scores"]["functional"],
        "tokens": summary["total_tokens"],
        "duration_s": summary["duration_s"],
    }
    path = OUT / "results.jsonl"
    seen = (
        {json.loads(line)["run_id"] for line in path.read_text().splitlines()}
        if path.exists()
        else set()
    )
    if summary["run_id"] not in seen:
        with path.open("a") as log:
            log.write(json.dumps(event) + "\n")
        print("RESULT " + json.dumps(event), flush=True)


def main():
    os.chdir(ROOT)
    OUT.mkdir(exist_ok=True)
    lock = (OUT / "sweep.lock").open("a")
    fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    grader_preflight()
    tasks = json.loads((SUITE / "suite.json").read_text())["tasks"]
    assert len(tasks) == len(set(tasks)) == 23
    hashes = {t: task_hash(load_task(t, SUITE)) for t in tasks}
    binary, digest = configure_binary()
    checked = get_cli_agent_adapter("muse-code").preflight()
    assert checked.ready and checked.version == VERSION, checked
    protocol = {
        "model": MODEL,
        "harness_version": checked.version,
        "levels": LEVELS,
        "binary": str(binary),
        "binary_sha256": digest,
        "pricing_tier": "contributor",
        "data_use": "Contributor permits Meta to train on submitted prompts and completions",
        "historical_run": "runs-muse13-cii-v4 preserved separately; no old results reused",
        "suite": str(SUITE),
        "task_hashes": hashes,
        "tasks_per_effort": 23,
        "billing": "account subscription, API key excluded",
        "concurrency": 1,
        "judging": "Saved separately after solving; same equal Astra/Claude panel as comparison",
        "final_weights": {"functional": 0.5, "quality": 0.15, "security": 0.15, "human_like": 0.2},
        "timeouts": {
            t: load_task(t, SUITE).metadata["agent_hints"]["suggested_timeout_s"] for t in tasks
        },
        "effort_provenance": "Explicit CLI flags, live no-task probes passed; provider does not echo effort",
        "ultra": "Client-side mode mapped to provider's highest supported reasoning tier; may change delegation; not a distinct higher API effort",
        "excluded_efforts": {
            "max": "Meta documents max as Standard-only; successful CLI acceptance does not prove effective max reasoning",
            "ultra": "Blocked board-wide by vulcanbench.toml [effort].blocked (decision 2026-09-16): a client-side mode mapped onto the provider's highest tier, not a distinct API effort, and the board publishes Low to Max only",
        },
        "price_source": "https://dev.meta.ai/docs/pricing-rate-limits/",
        "runtime_caveat": "Local host, concurrent Fable sweep; no CPU pinning",
        "stream_idle_timeout_secs": STREAM_IDLE_TIMEOUT_SECS,
        "amendments": [
            {
                "at": "2026-09-08T17:50:00+00:00",
                "change": "Raised Muse Code model stream idle timeout from the 180 s default to 900 s via TBH_STREAM_IDLE_TIMEOUT_SECS",
                "reason": "Meta Contributor stream stalls aborted 5 consecutive lodgecore attempts (minimal) plus blendcore x2 and stampcore x1; no completed run was affected",
                "state_at_amendment": "minimal: 20/23 complete; low through ultra not started",
                "approved_by": "user, in chat",
                "original_protocol": "protocol-original-2026-09-06.json",
            },
            {
                "at": "2026-09-09T07:10:00+00:00",
                "change": "Sweep driver accepts harness budget-exceeded summaries (functional 0.0, quality and security skipped by design) as scored failures instead of pausing",
                "reason": "lodgecore (minimal) exhausted its 36000 s task timeout; the driver's grading guard did not anticipate the harness's standard timeout outcome",
                "state_at_amendment": "minimal: 20/23 complete plus lodgecore timed out; low through ultra not started",
                "approved_by": "follows the user's standing instruction to keep the sweep going; timeouts were already described to the user as scored failures",
                "solver_conditions_changed": False,
            },
            {
                "at": "2026-09-09T21:20:00+00:00",
                "change": "Re-pinned source hashes to the versions merged in PR #107 (commit 0faabb8c): type annotations, a run_task wrapper delegating to _run, and ruff formatting",
                "reason": "The shared checkout moved to a branch carrying the merged files; the diff against the previously pinned sources is cosmetic and the Muse tests pass unchanged",
                "state_at_amendment": "minimal: 23/23 complete; low through ultra not started",
                "solver_conditions_changed": False,
            },
            {
                "at": "2026-09-15T15:50:00+00:00",
                "change": "Task timeouts follow the suite restamp in PR #114 (commit ff3bf1ec): flat 10800 s (3 h) per task instead of 36000 s (blendcore 5400 s)",
                "reason": "Suite-wide policy change recorded in docs/DECISIONS.md on 2026-09-13; the same bound was applied mid-sweep to the in-flight GPT-5.6 Luna run. Task hashes are unchanged; only agent_hints budgets moved",
                "state_at_amendment": "minimal: 23/23 complete under 10 h; low: 17/23 complete under 10 h; remaining 6 low tasks and medium through ultra run under 3 h",
                "solver_conditions_changed": True,
                "comparability_note": "Under a 3 h cap, 6 minimal runs (tallycore, granarycore, depotcore, lodgecore, cellarcore, paddockcore) and 1 low run (tallycore) would have scored 0; four of them scored 0.43 to 0.64 and low tallycore 0.875 as recorded. Minimal mean 0.717 as scored versus 0.620 if capped; low-so-far 0.842 versus 0.791",
            },
            {
                "at": "2026-09-18T06:10:00+00:00",
                "change": "Sweep driver accepts harness summaries whose patch changed no recognized source file (functional 0.0, analyzers skipped) as scored failures instead of pausing",
                "reason": "paddockcore (high) finished inside its budget but its patch contained only probe text files; the harness scored it 0/15 and skipped quality and security by design",
                "state_at_amendment": "minimal, low, medium complete; high 22/23 complete plus paddockcore scored 0; extra-high and ultra not started",
                "solver_conditions_changed": False,
            },
            {
                "at": "2026-09-18T16:30:00+00:00",
                "change": "Re-pinned harness/effort.py to the working-tree version carrying an in-progress Devin CLI effort map (uncommitted work by another session on branch frontier-rename)",
                "reason": "The added _DEVIN_EFFORT_VALUES table and its registry entry do not touch the muse-code effort map; the Muse tests pass against the tree",
                "state_at_amendment": "extra-high 21/23 complete; ultra not started",
                "solver_conditions_changed": False,
            },
            {
                "at": "2026-09-18T20:30:00+00:00",
                "change": "Dropped ultra from the sweep levels; the sweep ends at extra-high with 5 levels x 23 tasks",
                "reason": "vulcanbench.toml [effort].blocked refuses ultra at every entry point (owner decision 2026-09-16, docs/DECISIONS.md), and that record directs this protocol to drop ultra at its next stop",
                "state_at_amendment": "minimal, low, medium, high, extra-high complete (115 scored runs); no ultra run was ever attempted",
                "solver_conditions_changed": False,
            },
        ],
        "boundary": "Kernel-denied checkout/prior agent sessions/shared temp artifacts; isolated writes and explicit TMPDIR guidance; web tools disabled; shell network not isolated",
        "source_hashes": {
            p: hashlib.sha256((ROOT / p).read_bytes()).hexdigest()
            for p in [
                "harness/agent/muse_code.py",
                "harness/effort.py",
                "scripts/cii-v4-board/run_muse_sweep.py",
            ]
        },
    }
    path = OUT / "protocol.json"
    save_protocol(path, protocol)
    status_path = OUT / "status.json"

    def status(**values):
        values["updated_at"] = now()
        status_path.write_text(json.dumps(values, indent=2) + "\n")
        print(json.dumps(values), flush=True)

    for level in LEVELS:
        output = OUT / level
        output.mkdir(exist_ok=True)
        completed = {}
        for p in output.glob("*/summary.json"):
            r = json.loads(p.read_text())
            t = r["task_id"]
            assert t in hashes and r["task_hash"] == hashes[t] and r["model"] == MODEL
            assert r["effort"]["requested"] == level and r["effort"]["supported"]
            assert t not in completed, "Duplicate task results"
            result_summary({"run_id": r["run_id"], "summary": r})
            completed[t] = r
            record_result(r, level)
        for task in tasks:
            if task in completed:
                continue
            assert task_hash(load_task(task, SUITE)) == hashes[task]
            status(state="running", effort=level, task=task, completed_in_effort=len(completed))
            try:
                configure_binary()
                assert get_cli_agent_adapter("muse-code").preflight().version == VERSION
                assert all(
                    hashlib.sha256((ROOT / p).read_bytes()).hexdigest() == h
                    for p, h in protocol["source_hashes"].items()
                ), "Solver source changed during sweep"
                result = result_summary(
                    run_agent(
                        task,
                        MODEL,
                        output_dir=output,
                        tasks_root=SUITE,
                        judges=False,
                        sandbox="local",
                        network=False,
                        suite="coding-intelligence-index-v4",
                        effort=level,
                    )
                )
                record_result(result, level)
            except Exception as exc:
                status(
                    state="paused_error",
                    effort=level,
                    task=task,
                    completed_in_effort=len(completed),
                    error=str(exc),
                )
                raise
            completed[task] = result
        status(state="effort_complete", effort=level, completed_in_effort=len(completed))
    status(state="complete", completed=len(tasks) * len(LEVELS))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--accept-contributor-training",
        action="store_true",
        help="Explicitly approve sending benchmark content to Meta's training-eligible Contributor tier",
    )
    args = parser.parse_args()
    if not args.accept_contributor_training:
        parser.error(
            "Benchmark paused: Contributor permits training on submitted content; explicit approval is required"
        )
    try:
        main()
    except Exception as exc:
        print(f"Sweep stopped: {exc}", flush=True)
        status_path = OUT / "status.json"
        if OUT.is_dir():
            previous = json.loads(status_path.read_text()) if status_path.exists() else {}
            previous.update(state="paused_error", error=str(exc), updated_at=now())
            status_path.write_text(json.dumps(previous, indent=2) + "\n")
        sys.exit(1)
