"""Muse Wonder 1.0 on VulcanBench Frontier v4 through the Model API, Pi and Muse Code.

Runs ONE effort level per invocation, then stops and reports spend; the owner
approves each next level (docs/DECISIONS.md, 2026-10-13). Within a level each
task runs in Pi, then in Muse Code, one run at a time, so the two harnesses
stay paired if the level stops early. A per-level --budget stops the level
before the next run once summed API spend passes it; Pi also has a per-run
kill cap. Both harnesses bill the Model API key in Standard mode.

Each harness gets its own run root and frozen protocol; a changed protocol,
pinned binary, provider config or solver source fails closed on resume.

Run from a bench-tag run worktree (docs/HOW_WE_WORK.md) with --out pointing at
the main checkout's gitignored runs-effort-* folder:

    python scripts/cii-v4-board/run_wonder_api_sweep.py --effort low --budget 150 \\
        --out /Users/vulcanbench/dev/VulcanBench/runs-effort-musewonder10-api --accept-api-spend
"""

import argparse
import fcntl
import hashlib
import json
import os
import subprocess
import sys
from pathlib import Path

from run_muse_sweep import grader_preflight, now, result_summary, save_protocol

from harness.agent.cli_agents import get_cli_agent_adapter
from harness.agent.loop import run_agent
from harness.tasks import load_task, task_hash

ROOT = Path(__file__).resolve().parents[2]
SUITE = ROOT / "tasks/coding-intelligence-index-v4"
LEVELS = ["low", "medium", "high", "extra-high", "max"]
ARMS = {"pi": "pi:meta/muse-wonder-1.0", "muse-code": "muse-code:muse-wonder-1.0-api"}
VERSIONS = {"pi": "Pi 1.2.0", "muse-code": "Muse Code 1.4.6 (1.4.6-R6000.1)"}
PI_ROOT = Path.home() / ".local/share/vulcanbench"
PINS = {
    "VULCANBENCH_PI_NODE": str(Path.home() / ".local/node/bin/node"),
    "VULCANBENCH_PI_CLI": str(
        PI_ROOT / "pi-1.2.0/node_modules/@earendil-works/pi-coding-agent/dist/bundle/cli.js"
    ),
    "VULCANBENCH_PI_SHA256": "e79626f2dd6f94aa45d30f3fa63cd84319a6eefcd150b353cfaf274366926774",
    "VULCANBENCH_PI_CONFIG": str(PI_ROOT / "pi-config"),
    "VULCANBENCH_MUSE_BINARY": str(Path.home() / ".local/bin/muse-bin-1.4.6-R6000.1"),
    "VULCANBENCH_MUSE_SHA256": "c713165a287d64afb8816f8351a240b457a23524f4ade06bca0e41dfde410460",
}
SOURCES = [
    "harness/agent/pi_agent.py",
    "harness/agent/muse_code.py",
    "harness/agent/loop.py",
    "harness/effort.py",
    "harness/pricing.py",
    "scripts/cii-v4-board/run_muse_sweep.py",
    "scripts/cii-v4-board/run_wonder_api_sweep.py",
]
OTHER_SWEEPS = (
    "vulcanbench run|vulcanconduct run|run_muse_sweep.py|run_muse_wonder_sweep.py|"
    "run_effort_sweep|run_wonder_harness_pilot.py"
)


def sha256(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def source_hashes():
    return {p: sha256(ROOT / p) for p in SOURCES}


def other_sweep_active():
    """Sweeps run one task at a time across the whole host (docs/DECISIONS.md)."""
    found = subprocess.run(
        ["pgrep", "-f", OTHER_SWEEPS], capture_output=True, text=True, check=False
    )
    return [pid for pid in found.stdout.split() if int(pid) != os.getpid()]


def build_protocol(arm, version, hashes, timeouts):
    return {
        "model": ARMS[arm],
        "served_model": "muse-wonder-1.0",
        "harness": arm,
        "harness_version": version,
        "levels": LEVELS,
        "billing": "Meta Model API key, Standard mode (no training on submitted content)",
        "pins": {
            k: v for k, v in PINS.items() if (k.startswith("VULCANBENCH_PI") == (arm == "pi"))
        },
        "pi_provider_config_sha256": sha256(Path(PINS["VULCANBENCH_PI_CONFIG"]) / "models.json")
        if arm == "pi"
        else None,
        "suite": "VulcanBench Frontier v4 (tasks/coding-intelligence-index-v4)",
        "task_hashes": hashes,
        "timeouts": timeouts,
        "concurrency": 1,
        "pairing": "Each task runs in Pi then Muse Code before the next task",
        "approval": "One effort level per launch; the owner approves each next level",
        "judging": "Saved separately after solving",
        "excluded_efforts": {
            "minimal": "Board publishes Low to Max",
            "ultra": "Blocked board-wide by vulcanbench.toml [effort].blocked (2026-09-16)",
        },
        "price_source": "https://dev.meta.ai/docs/pricing-rate-limits/ (Wonder 1.0 Standard)",
        "boundary": "Kernel-denied checkout and shared temp; isolated writes; web tools off; "
        "shell network not isolated; the API key is in the agent environment",
        "source_hashes": source_hashes(),
    }


def level_spend(out, level):
    rows = {}
    for arm in ARMS:
        cells = [json.loads(p.read_text()) for p in (out / arm / level).glob("*/summary.json")]
        usd = [c["economics"]["marginal_cash_usd"] or 0.0 for c in cells]
        rows[arm] = {
            "tasks": len(cells),
            "usd": round(sum(usd), 4),
            "usd_per_task": round(sum(usd) / len(cells), 4) if cells else None,
            "functional_mean": round(sum(c["scores"]["functional"] for c in cells) / len(cells), 4)
            if cells
            else None,
        }
    return rows


def total_spend(out):
    return round(
        sum(
            json.loads(p.read_text())["economics"]["marginal_cash_usd"] or 0.0
            for p in out.glob("*/*/*/summary.json")
        ),
        4,
    )


def run(out, level, budget, pi_run_cap):
    os.chdir(ROOT)
    out.mkdir(parents=True, exist_ok=True)
    lock = (out / "sweep.lock").open("a")
    fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    busy = other_sweep_active()
    assert not busy, f"Another sweep is running (pids {busy}); sweeps stay serial"
    os.environ.update(PINS)
    assert any(os.environ.get(k) for k in ("MUSE_API_KEY", "META_MUSE_SPARK_API")), "No API key"
    grader_preflight()
    tasks = json.loads((SUITE / "suite.json").read_text())["tasks"]
    assert len(tasks) == len(set(tasks)) == 23
    hashes = {t: task_hash(load_task(t, SUITE)) for t in tasks}
    timeouts = {
        t: load_task(t, SUITE).metadata["agent_hints"]["suggested_timeout_s"] for t in tasks
    }
    protocols = {}
    for arm, spec in ARMS.items():
        checked = get_cli_agent_adapter(spec).preflight()
        assert checked.available and checked.version == VERSIONS[arm], checked
        (out / arm).mkdir(exist_ok=True)
        protocols[arm] = build_protocol(arm, checked.version, hashes, timeouts)
        save_protocol(out / arm / "protocol.json", protocols[arm])
    status_path = out / "status.json"

    def status(**values):
        values["updated_at"] = now()
        status_path.write_text(json.dumps(values, indent=2) + "\n")
        print(json.dumps(values), flush=True)

    done = {arm: {} for arm in ARMS}
    for arm, spec in ARMS.items():
        (out / arm / level).mkdir(exist_ok=True)
        for p in (out / arm / level).glob("*/summary.json"):
            r = json.loads(p.read_text())
            t = r["task_id"]
            assert t in hashes and r["task_hash"] == hashes[t] and r["model"] == spec
            assert r["effort"]["requested"] == level and r["effort"]["supported"]
            assert t not in done[arm], "Duplicate task results"
            result_summary({"run_id": r["run_id"], "summary": r})
            done[arm][t] = r
    for task in tasks:
        for arm, spec in ARMS.items():
            if task in done[arm]:
                continue
            spent = sum(v["usd"] for v in level_spend(out, level).values())
            if spent >= budget:
                status(
                    state="budget_stop",
                    effort=level,
                    level_spend=level_spend(out, level),
                    total_spend_usd=total_spend(out),
                    budget_usd=budget,
                )
                return
            assert task_hash(load_task(task, SUITE)) == hashes[task]
            assert source_hashes() == protocols[arm]["source_hashes"], "Solver source changed"
            status(
                state="running", effort=level, arm=arm, task=task, level_spend_usd=round(spent, 4)
            )
            try:
                result = result_summary(
                    run_agent(
                        task,
                        spec,
                        output_dir=out / arm / level,
                        tasks_root=SUITE,
                        judges=False,
                        sandbox="local",
                        network=False,
                        suite="coding-intelligence-index-v4",
                        effort=level,
                        max_run_cost=pi_run_cap if arm == "pi" else None,
                    )
                )
            except Exception as exc:
                status(state="paused_error", effort=level, arm=arm, task=task, error=str(exc))
                raise
            done[arm][task] = result
            print(
                "RESULT "
                + json.dumps(
                    {
                        "effort": level,
                        "arm": arm,
                        "task": task,
                        "functional": result["scores"]["functional"],
                        "usd": result["economics"]["marginal_cash_usd"],
                        "minutes": round(result["duration_s"] / 60, 1),
                    }
                ),
                flush=True,
            )
    status(
        state="level_complete_awaiting_approval",
        effort=level,
        level_spend=level_spend(out, level),
        total_spend_usd=total_spend(out),
        next_level=(LEVELS[LEVELS.index(level) + 1] if level != LEVELS[-1] else None),
    )


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--effort", choices=LEVELS, required=True, help="The one level to run")
    parser.add_argument("--budget", type=float, required=True, help="USD ceiling for this level")
    parser.add_argument("--pi-run-cap", type=float, default=25.0, help="USD kill cap per Pi run")
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument(
        "--accept-api-spend",
        action="store_true",
        help="Confirm the owner approved metered API spend for this level",
    )
    args = parser.parse_args(argv)
    if not args.accept_api_spend:
        parser.error("Each level needs the owner's approval; pass --accept-api-spend")
    out = args.out.resolve()
    try:
        run(out, args.effort, args.budget, args.pi_run_cap)
    except Exception as exc:
        print(f"Sweep stopped: {exc}", flush=True)
        sys.exit(1)


if __name__ == "__main__":
    main()
