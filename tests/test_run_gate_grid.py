"""run_gate.sh --grid against a fake harbor (budgeted sessions, fixed batches).

Reuses the fake tools of test_run_gate_script.py; the fake harbor here
writes one trial per ``-i`` environment with a grid reward.json and, for
environments named in FAKE_TIMEOUT, Harbor's AgentTimeoutError record.
"""

from __future__ import annotations

import json
import shutil
import signal
from pathlib import Path

import pytest

from tests.test_run_gate_script import SCRIPT, Gate

pytestmark = pytest.mark.skipif(shutil.which("bash") is None, reason="needs bash")

GRID_HARBOR = r"""#!/usr/bin/env python3
import json, os, signal, sys, time
args = sys.argv[1:]
if args == ["--version"]:
    print("fake-harbor"); sys.exit(0)
job = args[args.index("--job-name") + 1]
out = args[args.index("-o") + 1]
envs = [args[i + 1] for i, a in enumerate(args) if a == "-i"]
mode = os.environ.get("FAKE_HARBOR_MODE", "finish")
jd = os.path.join(out, job)
os.makedirs(jd)
with open(os.path.join(jd, "argv.json"), "w") as fh:
    json.dump({"argv": args}, fh)
with open(os.path.join(jd, "config.json"), "w") as fh:
    json.dump({"job_name": job, "n_concurrent_trials": int(args[args.index("-n") + 1])}, fh)

def on_int(*_):
    open(os.path.join(jd, "cancelled"), "w").write("sigint")
    sys.exit(130)

signal.signal(signal.SIGINT, on_int)
timeouts = os.environ.get("FAKE_TIMEOUT", "").split()
for k, env in enumerate(envs):
    td = os.path.join(jd, "%s__g%d" % (env[:32], k))
    os.makedirs(os.path.join(td, "verifier")); os.makedirs(os.path.join(td, "agent"))
    log = "hit your usage limit\n" if (mode == "infra" and k == 1) else '{"type":"item.completed"}\n'
    open(os.path.join(td, "agent", "codex.txt"), "w").write(log)
    if mode == "hang":
        continue
    done = k + 1
    json.dump({"reward": done / 8, "items_done": done, "items_total": 8},
              open(os.path.join(td, "verifier", "reward.json"), "w"))
    res = {"started_at": "2026-11-21T12:00:00", "finished_at": "2026-11-21T12:21:00"}
    if env in timeouts:
        res["exception_info"] = {"exception_type": "AgentTimeoutError",
                                 "exception_message": "Agent execution timed out after 1200.0 seconds"}
    json.dump(res, open(os.path.join(td, "result.json"), "w"))
if mode in ("hang", "infra"):
    time.sleep(600)
sys.exit(0)
"""

FREEZE = '[[verifier.collect]]\ncommand = "kill -KILL -1 2>/dev/null; true"\nuser = "root"\n'


class GridGate(Gate):
    ENVS = ("cli11-grid", "black-grid", "curl-grid", "ruint-grid", "mpmath-grid")

    def __init__(self, tmp_path: Path) -> None:
        super().__init__(tmp_path)
        shutil.copy(SCRIPT.parent / "grid_plan.py", self.script.parent / "grid_plan.py")
        (self.bin / "harbor").write_text(GRID_HARBOR)
        self.tasks = self.root / "tasks" / "frontier-v5"
        for e in self.ENVS:
            self.env_dir(e)
        self.grid = {
            "schema": 1,
            "budget_min": 20,
            "budget_instruction": "You have {minutes} minutes; the tree is graded as it stands.",
            "environments": {
                "cli11-grid": {"language": "cpp", "tier": "smaller"},
                "black-grid": {"language": "python", "tier": "smaller"},
                "curl-grid": {"language": "c", "tier": "larger"},
                "ruint-grid": {"language": "rust", "tier": "smaller"},
                "mpmath-grid": {"language": "python", "tier": "larger"},
            },
            "batches": [["cli11-grid", "black-grid", "curl-grid", "ruint-grid"], ["mpmath-grid"]],
        }
        self.write_grid()

    def env_dir(self, name: str, *, timeout: float = 3600.0, freeze: bool = True) -> None:
        d = self.tasks / name
        d.mkdir(parents=True, exist_ok=True)
        (d / "task.toml").write_text(
            f'[task]\nname = "vulcanbench/{name}"\n\n[agent]\ntimeout_sec = {timeout}\n\n'
            '[verifier]\ntimeout_sec = 1800.0\nenvironment_mode = "separate"\n\n'
            + (FREEZE if freeze else "")
        )

    def write_grid(self) -> None:
        (self.tasks / "grid.json").write_text(json.dumps(self.grid))

    def argv(self, job: str) -> list[str]:
        data: list[str] = json.loads((self.out / "jobs" / job / "argv.json").read_text())["argv"]
        return data


@pytest.fixture
def gg(tmp_path: Path) -> GridGate:
    return GridGate(tmp_path)


def test_grid_runs_each_batch_as_one_budgeted_harbor_job(gg: GridGate) -> None:
    r = gg.run("--grid", "sol62", GATE_EFFORT="max", FAKE_TIMEOUT="cli11-grid curl-grid")
    assert r.returncode == 0, r.stdout + r.stderr
    b1, b2 = "sol62-max-grid-b1", "sol62-max-grid-b2"
    argv = gg.argv(b1)
    assert [argv[i + 1] for i, a in enumerate(argv) if a == "-i"] == list(gg.ENVS[:4])
    assert argv[argv.index("-p") + 1] == str(gg.tasks)
    assert argv[argv.index("-k") + 1] == "1" and argv[argv.index("-n") + 1] == "4"
    assert float(argv[argv.index("--agent-timeout-multiplier") + 1]) * 3600 == pytest.approx(1200)
    assert (
        argv[argv.index("--extra-instruction") + 1]
        == "You have 20 minutes; the tree is graded as it stands."
    )
    aks = [argv[i + 1] for i, a in enumerate(argv) if a == "--ak"]
    assert "disable_web_search=true" in aks and "version=0.166.0" in aks
    st = gg.status(b1)
    assert st["phase"] == "done" and st["mode"] == "grid" and st["batch"] == 1
    assert st["environments"] == list(gg.ENVS[:4])
    assert st["budget_sec"] == 1200 and st["budget_source"] == "grid"
    assert st["items_done"] == 1 + 2 + 3 + 4 and st["items_total"] == 32
    assert st["budget_reached"] == 2 and "solves" not in st
    st2 = gg.status(b2)
    assert (
        st2["environments"] == ["mpmath-grid"] and gg.argv(b2)[gg.argv(b2).index("-n") + 1] == "1"
    )
    assert "10 of 32 items, 2 at the budget" in r.stdout
    assert "budget=1200s (grid)" in r.stdout
    # every environment's leftover sandboxes are swept
    calls = "\n".join(gg.docker_calls())
    for e in gg.ENVS:
        assert f"name=^{e[:32]}__" in calls


def test_budget_override_and_batch_selection(gg: GridGate) -> None:
    r = gg.run("--grid", "astra", "2", GATE_BUDGET_MIN="30")
    assert r.returncode == 0, r.stdout + r.stderr
    assert not (gg.out / "jobs" / "astra-medium-grid-b1").exists()
    job = "astra-medium-grid-b2"
    st = gg.status(job)
    assert st["budget_sec"] == 1800 and st["budget_source"] == "override"
    argv = gg.argv(job)
    assert float(argv[argv.index("--agent-timeout-multiplier") + 1]) == pytest.approx(0.5)
    assert "30 minutes" in argv[argv.index("--extra-instruction") + 1]


def test_no_budget_instruction_appends_nothing(gg: GridGate) -> None:
    gg.grid["budget_instruction"] = None
    gg.write_grid()
    assert gg.run("--grid", "astra", "2").returncode == 0
    assert "--extra-instruction" not in gg.argv("astra-medium-grid-b2")


@pytest.mark.parametrize(
    "breakage,message",
    [
        ("nofreeze", "no freeze hook"),
        ("timeout", "timeout_sec differs"),
        ("twice", "is in batches"),
        ("orphan", "not in any batch"),
        ("five", "1 to 4"),
        ("missing", "no environment"),
    ],
)
def test_bad_grid_files_are_refused_before_harbor(
    gg: GridGate, breakage: str, message: str
) -> None:
    if breakage == "nofreeze":
        gg.env_dir("black-grid", freeze=False)
    elif breakage == "timeout":
        gg.env_dir("curl-grid", timeout=7200.0)
    elif breakage == "twice":
        gg.grid["batches"][1].append("cli11-grid")
    elif breakage == "orphan":
        gg.grid["batches"] = [gg.grid["batches"][0]]
    elif breakage == "five":
        gg.grid["batches"] = [list(gg.ENVS)]
    elif breakage == "missing":
        shutil.rmtree(gg.tasks / "ruint-grid")
    gg.write_grid()
    r = gg.run("--grid", "sol62")
    assert r.returncode == 2 and message in r.stderr, r.stderr
    assert not (gg.out / "jobs").exists() or not any((gg.out / "jobs").iterdir())


def test_bad_batch_numbers_are_refused(gg: GridGate) -> None:
    assert gg.run("--grid", "sol62", "x").returncode == 2
    r = gg.run("--grid", "sol62", "3")
    assert r.returncode == 2 and "no batch 3" in r.stderr


def test_grid_infra_failure_stops_the_whole_batch(gg: GridGate) -> None:
    r = gg.run("--grid", "sol62", FAKE_HARBOR_MODE="infra")
    assert r.returncode == 1, r.stdout + r.stderr
    st = gg.status("sol62-medium-grid-b1")
    assert st["phase"] == "infra_failed" and str(st["stop_reason"]).startswith("infra:")
    assert (gg.out / "jobs" / "sol62-medium-grid-b1" / "cancelled").exists()
    assert not (gg.out / "jobs" / "sol62-medium-grid-b2").exists()


def test_sigint_stops_a_grid_batch(gg: GridGate) -> None:
    proc = gg.start("--grid", "sol62", FAKE_HARBOR_MODE="hang")
    try:
        gg.wait_phase("sol62-medium-grid-b1", "running")
        proc.send_signal(signal.SIGINT)
        proc.wait(timeout=30)
    finally:
        if proc.poll() is None:
            proc.kill()
    assert proc.returncode == 130
    st = gg.status("sol62-medium-grid-b1")
    assert st["phase"] == "stopped" and st["stop_reason"] == "signal-INT"


def test_task_mode_is_unchanged_by_the_grid_option(gg: GridGate) -> None:
    # A plain task name still runs the gate protocol (three attempts, -p <task>).
    (gg.bin / "harbor").write_text(GRID_HARBOR)
    r = gg.run("sol62", "cli11-grid", GATE_EARLY_EXIT="0")
    assert r.returncode == 0, r.stdout + r.stderr
    argv = gg.argv("sol62-cli11-grid")
    assert argv[argv.index("-p") + 1] == str(gg.tasks / "cli11-grid")
    assert argv[argv.index("-k") + 1] == "3" and "-i" not in argv
    assert "--agent-timeout-multiplier" not in argv
