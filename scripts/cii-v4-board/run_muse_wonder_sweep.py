"""Sequential, resumable Muse Wonder sweep on VulcanBench Frontier v4. Stop on any error.

Runs one model (Wonder 1.0 or Wonder Full) in Contributor mode through a
pinned Muse Code binary (1.4.6 by default, or 1.4.9 with --muse-version), one
task at a time, low to extra-high. Max is
excluded under Contributor and ultra is blocked board-wide (docs/DECISIONS.md,
2026-10-12 and 2026-09-16). Each model gets its own run root and frozen
protocol; a changed protocol fails closed on resume.

Run it from a bench-tag run worktree (docs/HOW_WE_WORK.md) with --out pointing
at the main checkout's gitignored runs-effort-* folder.

A new Muse Code version gets its own run root. --continue-from names an earlier
root of the same model: at each level, tasks that root already completed are
skipped, so a sweep that changes version part-way keeps one version per root
(owner decision 2026-10-16, docs/DECISIONS.md).
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
from harness.agent.muse_code import CLI_MODELS, STREAM_IDLE_TIMEOUT_SECS, pinned_executable
from harness.tasks import load_task, task_hash

ROOT = Path(__file__).resolve().parents[2]
SUITE = ROOT / "tasks/coding-intelligence-index-v4"
LEVELS = ["low", "medium", "high", "extra-high"]
MODELS = {
    "wonder": "muse-code:muse-wonder-1.0-contributor",
    "wonder-full": "muse-code:muse-wonder-full-1.0-contributor",
}
# Muse Code release -> (pinned binary, its SHA256, the version string preflight reports).
PINS = {
    "1.4.6": (
        Path.home() / ".local/bin/muse-bin-1.4.6-R6000.1",
        "c713165a287d64afb8816f8351a240b457a23524f4ade06bca0e41dfde410460",
        "Muse Code 1.4.6 (1.4.6-R6000.1)",
    ),
    # Meta's muse-stable channel on 2026-10-16; SHA256 equals its release manifest's.
    "1.4.9": (
        Path.home() / ".local/bin/muse-bin-1.4.9-R6297.1",
        "2b856618ae3de3a63956e6d510b849ad707b16e5d2ed196299f3fe5ef2e3ff5e",
        "Muse Code 1.4.9 (1.4.9-R6297.1)",
    ),
}
BINARY, BINARY_SHA256, VERSION = PINS["1.4.6"]
# Models a release may run on goal and verify reminder observers only. 1.4.9 runs them
# on Spark 1.3 whatever --model or the saved default say; 1.4.6 ran them on the main model.
REMINDER_MODELS = {"1.4.6": (), "1.4.9": ("muse-spark-1.3",)}
# Muse Code reads user settings (plugins included) from here; they shape the
# solver's context, so the protocol pins their exact content.
MUSE_SETTINGS = Path.home() / ".config/muse/settings.json"
SOURCES = [
    "harness/agent/muse_code.py",
    "harness/effort.py",
    "harness/pricing.py",
    "scripts/cii-v4-board/run_muse_sweep.py",
    "scripts/cii-v4-board/run_muse_wonder_sweep.py",
]


def configure_binary(release="1.4.6"):
    binary, digest, _ = PINS[release]
    os.environ["VULCANBENCH_MUSE_BINARY"] = str(binary)
    os.environ["VULCANBENCH_MUSE_SHA256"] = digest
    os.environ["VULCANBENCH_MUSE_REMINDER_MODELS"] = ",".join(REMINDER_MODELS[release])
    return pinned_executable()


def completed_in(root, level, model, hashes):
    """Tasks an earlier run root of the same model finished at this level, with matching task hashes."""
    done = set()
    for p in (root / level).glob("*/summary.json"):
        r = json.loads(p.read_text())
        assert r["model"] == model and r["effort"]["requested"] == level, p
        assert r["task_hash"] == hashes[r["task_id"]], f"{p}: task changed since that run"
        done.add(r["task_id"])
    return done


def continuation(root, model, out):
    """What a continued root records about the root it continues, which must be the same model's."""
    if root is None:
        return None
    previous = json.loads((root / "protocol.json").read_text())
    assert root != out and previous["model"] == model, "continue from the same model's root"
    return {
        "root": str(root),
        "protocol_sha256": hashlib.sha256((root / "protocol.json").read_bytes()).hexdigest(),
        "harness_version": previous["harness_version"],
        "rule": "at each level, tasks completed in that root are skipped here",
    }


def muse_settings():
    """User settings Muse Code loads for every run, and their SHA256."""
    raw = MUSE_SETTINGS.read_bytes() if MUSE_SETTINGS.is_file() else b""
    settings = json.loads(raw) if raw else {}
    return settings, hashlib.sha256(raw).hexdigest()


def source_hashes():
    return {p: hashlib.sha256((ROOT / p).read_bytes()).hexdigest() for p in SOURCES}


def other_sweep_active():
    """Sweeps run one task at a time across the whole host (docs/DECISIONS.md)."""
    found = subprocess.run(
        [
            "pgrep",
            "-f",
            "vulcanbench run|vulcanconduct run|run_muse_sweep.py|run_effort_sweep|"
            "run_wonder_api_sweep.py|run_wonder_harness_pilot.py",
        ],
        capture_output=True,
        text=True,
        check=False,
    )
    return [pid for pid in found.stdout.split() if int(pid) != os.getpid()]


def build_protocol(
    model, binary, digest, version, hashes, timeouts, continues=None, reminder_models=()
):
    spec = model.split(":", 1)[1]
    cli_model, contributor_mode = CLI_MODELS[spec]
    return {
        "model": model,
        "cli_model": cli_model,
        "contributor_mode": contributor_mode,
        "harness_version": version,
        "levels": LEVELS,
        "binary": str(binary),
        "binary_sha256": digest,
        "pricing_tier": "contributor",
        "data_use": "Contributor permits Meta to train on submitted prompts and completions; "
        "accepted by the owner in chat 2026-10-12 (docs/DECISIONS.md)",
        "suite": "VulcanBench Frontier v4 (tasks/coding-intelligence-index-v4)",
        "task_hashes": hashes,
        "tasks_per_effort": len(hashes),
        "billing": "Muse Code subscription; API keys excluded from the child environment",
        "concurrency": 1,
        "judging": "Saved separately after solving",
        "timeouts": timeouts,
        "effort_provenance": "Explicit --reasoning-effort flag; synthetic probes passed "
        "2026-10-12; Muse receipts do not echo the effective effort",
        "contributor_provenance": "Explicit --contributor-mode on; Muse receipts do not "
        "record the data mode",
        "excluded_efforts": {
            "max": "Meta documents max as Standard-only for Spark Contributor and does not "
            "document it for Wonder Contributor",
            "ultra": "Blocked board-wide by vulcanbench.toml [effort].blocked (2026-09-16)",
        },
        "price_source": "https://dev.meta.ai/docs/pricing-rate-limits/",
        "stream_idle_timeout_secs": STREAM_IDLE_TIMEOUT_SECS,
        "muse_settings_plugins": muse_settings()[0].get("plugins", {}),
        "muse_settings_sha256": muse_settings()[1],
        "boundary": "Kernel-denied checkout/prior agent sessions/shared temp artifacts; "
        "isolated writes and explicit TMPDIR guidance; web tools disabled; shell network "
        "not isolated",
        "source_hashes": source_hashes(),
        **({"continues": continues} if continues else {}),
        **(
            {
                "reminder_observer_models": list(reminder_models),
                "reminder_observer_rule": "goal and verify reminder observers may run on these "
                "models; their tokens are recorded apart under auxiliary and every other call "
                "must be the requested model (owner decision 2026-10-16)",
            }
            if reminder_models
            else {}
        ),
    }


def record_result(out, summary, level):
    event = {
        "at": now(),
        "effort": level,
        "task": summary["task_id"],
        "run_id": summary["run_id"],
        "functional": summary["scores"]["functional"],
        "tokens": summary["total_tokens"],
        "duration_s": summary["duration_s"],
    }
    path = out / "results.jsonl"
    seen = (
        {json.loads(line)["run_id"] for line in path.read_text().splitlines()}
        if path.exists()
        else set()
    )
    if summary["run_id"] not in seen:
        with path.open("a") as log:
            log.write(json.dumps(event) + "\n")
        print("RESULT " + json.dumps(event), flush=True)


def run(model, out, levels, release="1.4.6", continue_from=None):  # noqa: PLR0915, one linear sweep loop
    os.chdir(ROOT)
    out.mkdir(parents=True, exist_ok=True)
    lock = (out / "sweep.lock").open("a")
    fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    busy = other_sweep_active()
    assert not busy, f"Another sweep is running (pids {busy}); sweeps stay serial"
    grader_preflight()
    tasks = json.loads((SUITE / "suite.json").read_text())["tasks"]
    assert len(tasks) == len(set(tasks)) == 23
    hashes = {t: task_hash(load_task(t, SUITE)) for t in tasks}
    timeouts = {
        t: load_task(t, SUITE).metadata["agent_hints"]["suggested_timeout_s"] for t in tasks
    }
    version = PINS[release][2]
    binary, digest = configure_binary(release)
    checked = get_cli_agent_adapter("muse-code").preflight()
    assert checked.ready and checked.version == version, checked
    protocol = build_protocol(
        model,
        binary,
        digest,
        checked.version,
        hashes,
        timeouts,
        continuation(continue_from, model, out),
        REMINDER_MODELS[release],
    )
    save_protocol(out / "protocol.json", protocol)
    status_path = out / "status.json"

    def status(**values):
        values["updated_at"] = now()
        status_path.write_text(json.dumps(values, indent=2) + "\n")
        print(json.dumps(values), flush=True)

    for level in levels:
        output = out / level
        output.mkdir(exist_ok=True)
        completed = {}
        earlier = completed_in(continue_from, level, model, hashes) if continue_from else set()
        for p in output.glob("*/summary.json"):
            r = json.loads(p.read_text())
            t = r["task_id"]
            assert t in hashes and r["task_hash"] == hashes[t] and r["model"] == model
            assert r["effort"]["requested"] == level and r["effort"]["supported"]
            assert t not in completed, "Duplicate task results"
            result_summary({"run_id": r["run_id"], "summary": r})
            completed[t] = r
            record_result(out, r, level)
        assert not earlier & set(completed), "Task completed in both roots"
        for task in tasks:
            if task in completed or task in earlier:
                continue
            assert task_hash(load_task(task, SUITE)) == hashes[task]
            status(state="running", effort=level, task=task, completed_in_effort=len(completed))
            try:
                configure_binary(release)
                assert get_cli_agent_adapter("muse-code").preflight().version == version
                assert muse_settings()[1] == protocol["muse_settings_sha256"], (
                    "Muse settings changed during sweep"
                )
                assert source_hashes() == protocol["source_hashes"], (
                    "Solver source changed during sweep"
                )
                result = result_summary(
                    run_agent(
                        task,
                        model,
                        output_dir=output,
                        tasks_root=SUITE,
                        judges=False,
                        sandbox="local",
                        network=False,
                        suite="coding-intelligence-index-v4",
                        effort=level,
                    )
                )
                record_result(out, result, level)
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
        status(
            state="effort_complete",
            effort=level,
            completed_in_effort=len(completed),
            completed_in_earlier_root=len(earlier),
        )
    status(state="complete", completed=len(tasks) * len(levels))


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--model", choices=sorted(MODELS), required=True)
    parser.add_argument("--out", type=Path, required=True, help="Run root, one per model")
    parser.add_argument("--efforts", nargs="+", choices=LEVELS, default=LEVELS)
    parser.add_argument("--muse-version", choices=sorted(PINS), default="1.4.6")
    parser.add_argument(
        "--continue-from",
        type=Path,
        help="An earlier run root of the same model; tasks it completed are skipped",
    )
    parser.add_argument(
        "--accept-contributor-training",
        action="store_true",
        help="Explicitly approve sending benchmark content to Meta's training-eligible "
        "Contributor mode",
    )
    args = parser.parse_args(argv)
    if not args.accept_contributor_training:
        parser.error(
            "Benchmark paused: Contributor permits training on submitted content; "
            "explicit approval is required"
        )
    out = args.out.resolve()
    try:
        run(
            MODELS[args.model],
            out,
            args.efforts,
            args.muse_version,
            args.continue_from.resolve() if args.continue_from else None,
        )
    except Exception as exc:
        print(f"Sweep stopped: {exc}", flush=True)
        status_path = out / "status.json"
        if out.is_dir():
            previous = json.loads(status_path.read_text()) if status_path.exists() else {}
            previous.update(state="paused_error", error=str(exc), updated_at=now())
            status_path.write_text(json.dumps(previous, indent=2) + "\n")
        sys.exit(1)


if __name__ == "__main__":
    main()
