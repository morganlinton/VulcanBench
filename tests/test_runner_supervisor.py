"""The supervisor: start, finish, stop cascade, adoption after a restart, one at a time."""

from __future__ import annotations

import json
import os
import signal
import time
from collections.abc import Callable
from datetime import UTC, datetime, timedelta
from pathlib import Path
from typing import Any

import pytest

from backend.runner import launch, procs, sweep
from backend.runner import supervisor as sup
from backend.runner.config import RunnerConfig
from backend.runner.docker_ps import Container
from backend.runner.procs import Proc
from backend.runner.state import Job, State
from backend.runner.supervisor import STOP_GRACE_S, Supervisor, SupervisorService
from tests.runner_fixtures import fake_repo, fake_worktree, make_config, set_fake_run


class Clock:
    def __init__(self) -> None:
        self.t = datetime(2026, 10, 29, 12, 0, tzinfo=UTC)

    def __call__(self) -> datetime:
        return self.t

    def advance(self, s: float) -> None:
        self.t += timedelta(seconds=s)


@pytest.fixture
def cfg(tmp_path: Path) -> RunnerConfig:
    return make_config(tmp_path)


@pytest.fixture
def state(cfg: RunnerConfig) -> State:
    return State(cfg.home / "runner.db")


@pytest.fixture
def clock() -> Clock:
    return Clock()


@pytest.fixture
def no_validation(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(launch, "validate_before_launch", lambda _cfg, _spec: [])


def _own_procs() -> list[Proc]:
    """The real process table, limited to this test process and its descendants.

    A harbor or vulcanbench run launched by hand on the machine running the tests
    would otherwise hold the queue (that rule has its own test with an injected
    table).
    """
    table = procs.list_processes()
    me = os.getpid()
    keep = {me} | {p.pid for p in procs.descendants(me, table)}
    return [p for p in table if p.pid in keep]


def make_sup(cfg: RunnerConfig, state: State, clock: Clock, **kw: Any) -> Supervisor:
    kw.setdefault("containers", lambda: [])
    kw.setdefault("list_procs", _own_procs)
    return Supervisor(cfg, state, now=clock, **kw)


def script(tmp_path: Path, name: str, body: str) -> Path:
    p = tmp_path / name
    p.write_text("#!/bin/bash\n" + body)
    p.chmod(0o755)
    return p


def gate_spec(cfg: RunnerConfig, argv: list[str], **extra: Any) -> dict[str, Any]:
    job = "astra-demo"
    spec: dict[str, Any] = {
        "kind": "gate",
        "job_name": job,
        "tag": "bench/t",
        "worktree": str(cfg.run_worktrees / "bench-t"),
        "argv": argv,
        "cwd": str(cfg.home),
        "env": {"GATE_OUT": str(cfg.gate_out), "GATE_ATTEMPTS": "3"},
        "needs_prep": False,
        "job_dir": str(cfg.gate_jobs_dir / job),
        "status_file": str(cfg.gate_out / f"{job}.status.json"),
        "script_features": {"status_file": True, "preflight_only": True},
    }
    spec.update(extra)
    return spec


def wait_until(pred: Callable[[], bool], timeout: float = 10.0) -> None:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if pred():
            return
        time.sleep(0.05)
    raise AssertionError("condition not met in time")


def test_queued_job_starts_with_an_allowlisted_env(
    cfg: RunnerConfig,
    state: State,
    clock: Clock,
    tmp_path: Path,
    no_validation: None,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setenv("VULCANBENCH_ALLOW_DIRTY", "1")
    monkeypatch.setenv("SOME_API_KEY", "sk-no")
    dump = tmp_path / "env.json"
    sh = script(
        tmp_path,
        "dump.sh",
        f'python3 -c \'import json,os; json.dump(dict(os.environ), open("{dump}","w"))\'\nexit 0\n',
    )
    job = state.create_job("gate", "astra-demo", gate_spec(cfg, ["bash", str(sh), "astra", "demo"]))
    s = make_sup(cfg, state, clock)
    s.tick()
    started = state.get_job(job.id)
    assert started is not None and started.status == "running" and started.pid
    assert started.argv_token == "dump.sh astra demo" and started.proc_lstart and started.boot_time
    assert started.console_log and Path(started.console_log).read_text().startswith(
        "=== 2026-10-29T12:00:00+00:00 main: bash "
    )
    wait_until(dump.exists)
    env = json.loads(dump.read_text())
    assert "VULCANBENCH_ALLOW_DIRTY" not in env and "SOME_API_KEY" not in env
    assert env["GATE_OUT"] == str(cfg.gate_out) and env["GATE_ATTEMPTS"] == "3" and "HOME" in env
    wait_until(lambda: s.children[job.id].poll() is not None)
    s.tick()
    done = state.get_job(job.id)
    assert (
        done is not None and done.status == "finished" and done.exit_code == 0 and done.finished_at
    )
    assert job.id not in s.children and job.id not in s.logs


@pytest.mark.parametrize(
    ("code", "status", "note"),
    [
        (2, "failed", "refused before harbor"),
        (1, "infra_failed", "exited 1"),
        (7, "failed", "exited 7"),
        (130, "stopped", "outside the GUI"),
    ],
)
def test_exit_codes_without_a_status_file(
    cfg: RunnerConfig,
    state: State,
    clock: Clock,
    tmp_path: Path,
    no_validation: None,
    code: int,
    status: str,
    note: str,
) -> None:
    sh = script(tmp_path, "exit.sh", f"exit {code}\n")
    job = state.create_job("gate", "astra-demo", gate_spec(cfg, ["bash", str(sh), "astra", "demo"]))
    s = make_sup(cfg, state, clock)
    s.tick()
    wait_until(lambda: s.children[job.id].poll() is not None)
    s.tick()
    j = state.get_job(job.id)
    assert j is not None and j.status == status and j.exit_code == code, j
    assert any(note in n for n in j.notes)


def test_status_file_wins_over_the_exit_code(
    cfg: RunnerConfig, state: State, clock: Clock, tmp_path: Path, no_validation: None
) -> None:
    sh = script(tmp_path, "s.sh", "exit 0\n")
    job = state.create_job("gate", "astra-demo", gate_spec(cfg, ["bash", str(sh), "astra", "demo"]))
    (cfg.gate_out / "astra-demo.status.json").write_text(
        json.dumps({"phase": "infra_failed", "stop_reason": "infra:x", "exit_code": 130})
    )
    s = make_sup(cfg, state, clock)
    s.tick()
    wait_until(lambda: s.children[job.id].poll() is not None)
    s.tick()
    j = state.get_job(job.id)
    assert j is not None and j.status == "infra_failed" and j.exit_code == 0
    assert any("run_gate.sh stopped the run: infra:x" in n for n in j.notes)


def test_stop_request_sends_sigint_once_then_escalates(
    cfg: RunnerConfig, state: State, clock: Clock, tmp_path: Path, no_validation: None
) -> None:
    # Ignores SIGINT, so the cascade has to escalate to the process group.
    sh = script(tmp_path, "stubborn.sh", "trap '' INT\nsleep 30 & wait\n")
    job = state.create_job("gate", "astra-demo", gate_spec(cfg, ["bash", str(sh), "astra", "demo"]))
    sent: list[tuple[str, int, int]] = []
    real_kill, real_killpg = os.kill, os.killpg

    def kill(pid: int, sig: int) -> None:
        sent.append(("kill", pid, sig))
        real_kill(pid, sig)

    def killpg(pgid: int, sig: int) -> None:
        sent.append(("killpg", pgid, sig))
        real_killpg(pgid, sig)

    s = make_sup(cfg, state, clock, kill=kill, killpg=killpg)
    s.tick()
    j = state.get_job(job.id)
    assert j is not None and j.status == "running" and j.pid
    state.request_stop(job.id, "operator")
    s.tick()
    j = state.get_job(job.id)
    assert j is not None and j.status == "stopping" and j.stop_signalled_at
    assert sent == [("kill", j.pid, signal.SIGINT)]
    s.tick()  # nothing new before the grace period
    assert len(sent) == 1
    clock.advance(STOP_GRACE_S + 1)
    s.tick()
    assert sent[-1] == ("killpg", j.pid, signal.SIGTERM)
    wait_until(lambda: s.children[job.id].poll() is not None)
    s.tick()
    j = state.get_job(job.id)
    assert j is not None and j.status == "stopped" and j.exit_code == -signal.SIGTERM
    assert any("stopped on request" in n for n in j.notes)
    assert any("SIGTERM sent" in n for n in j.notes)


def test_stop_request_on_a_cooperative_script(
    cfg: RunnerConfig, state: State, clock: Clock, tmp_path: Path, no_validation: None
) -> None:
    sh = script(tmp_path, "nice.sh", "trap 'exit 130' INT\nwhile true; do sleep 0.1; done\n")
    job = state.create_job("gate", "astra-demo", gate_spec(cfg, ["bash", str(sh), "astra", "demo"]))
    s = make_sup(cfg, state, clock)
    s.tick()
    state.request_stop(job.id, "operator")
    s.tick()
    wait_until(lambda: s.children[job.id].poll() is not None)
    s.tick()
    j = state.get_job(job.id)
    assert j is not None and j.status == "stopped" and j.exit_code == 130


def test_restart_adopts_a_live_job_and_finalizes_from_the_status_file(
    cfg: RunnerConfig, state: State, clock: Clock, tmp_path: Path, no_validation: None
) -> None:
    sh = script(tmp_path, "long.sh", "sleep 30\n")
    job = state.create_job("gate", "astra-demo", gate_spec(cfg, ["bash", str(sh), "astra", "demo"]))
    first = make_sup(cfg, state, clock)
    first.tick()
    child = first.children[job.id]
    # A new backend: same state file, no Popen handles.
    second = make_sup(cfg, state, clock)
    second.tick()
    j = state.get_job(job.id)
    assert j is not None and j.status == "running" and j.pid == child.pid
    assert second.children == {}
    # The run ends while no backend is watching; the script's status file tells the story.
    (cfg.gate_out / "astra-demo.status.json").write_text(
        json.dumps({"phase": "done", "exit_code": 0, "stop_reason": None})
    )
    child.terminate()
    child.wait(timeout=10)
    second.tick()
    j = state.get_job(job.id)
    assert j is not None and j.status == "finished" and j.exit_code == 0
    first.tick()  # the old instance notices too and does not resurrect it
    assert state.get_job(job.id).status == "finished"  # type: ignore[union-attr]


def test_restart_with_a_recycled_pid_marks_the_job_lost(
    cfg: RunnerConfig, state: State, clock: Clock, tmp_path: Path, no_validation: None
) -> None:
    sh = script(tmp_path, "long.sh", "sleep 30\n")
    job = state.create_job("gate", "astra-demo", gate_spec(cfg, ["bash", str(sh), "astra", "demo"]))
    first = make_sup(cfg, state, clock)
    first.tick()
    pid = first.children[job.id].pid
    state.update_job(job.id, proc_lstart="Mon Jan  1 00:00:00 2001")  # not the process we started
    second = make_sup(cfg, state, clock)
    second.tick()
    j = state.get_job(job.id)
    assert j is not None and j.status == "lost" and any("final state unknown" in n for n in j.notes)
    first.children[job.id].kill()
    first.children[job.id].wait()
    assert pid


def test_one_at_a_time_and_outside_runs_hold_the_queue(
    cfg: RunnerConfig, state: State, clock: Clock, tmp_path: Path, no_validation: None
) -> None:
    quick = script(tmp_path, "quick.sh", "exit 0\n")
    a = state.create_job(
        "gate", "astra-a", gate_spec(cfg, ["bash", str(quick), "astra", "a"], job_name="astra-a")
    )
    b = state.create_job(
        "gate", "astra-b", gate_spec(cfg, ["bash", str(quick), "astra", "b"], job_name="astra-b")
    )
    outside = [
        Proc(
            pid=99999,
            ppid=1,
            pgid=99999,
            stat="S",
            lstart="Thu Oct 29 11:00:00 2026",
            command="/x/bin/python harbor run -p t --job-name hand-made",
        )
    ]
    table: list[Proc] = list(outside)
    s = make_sup(cfg, state, clock, list_procs=lambda: table + _own_procs())
    s.tick()
    assert state.get_job(a.id).status == "queued"  # type: ignore[union-attr]
    assert s.hold_reason and "99999" in s.hold_reason
    table.clear()
    s.tick()
    assert state.get_job(a.id).status == "running"  # type: ignore[union-attr]
    assert state.get_job(b.id).status == "queued"  # type: ignore[union-attr]
    wait_until(lambda: s.children[a.id].poll() is not None)
    s.tick()
    assert state.get_job(a.id).status == "finished"  # type: ignore[union-attr]
    assert state.get_job(b.id).status in {"queued", "running"}  # type: ignore[union-attr]

    def b_done() -> bool:
        s.tick()
        return state.get_job(b.id).status == "finished"  # type: ignore[union-attr]

    wait_until(b_done)


def test_prepare_step_runs_first_then_the_gate_command(
    cfg: RunnerConfig, state: State, clock: Clock, tmp_path: Path, no_validation: None
) -> None:
    wt = cfg.run_worktrees / "bench-t"
    prep = script(tmp_path, "prep.sh", f"mkdir -p {wt}\necho prepared\n")
    main = script(tmp_path, "main.sh", "echo main ran\nexit 0\n")
    spec = gate_spec(
        cfg,
        ["bash", str(main), "astra", "demo"],
        needs_prep=True,
        prep_argv=["bash", str(prep), "bench/t"],
        prep_env={"RUN_WORKTREES": str(cfg.run_worktrees)},
        tag="bench/t",
    )
    job = state.create_job("gate", "astra-demo", spec)
    s = make_sup(cfg, state, clock)
    s.tick()
    j = state.get_job(job.id)
    assert (
        j is not None
        and j.status == "preparing"
        and j.phase == "prepare"
        and j.argv_token == "bench/t"
    )
    wait_until(lambda: s.children[job.id].poll() is not None)
    s.tick()
    j = state.get_job(job.id)
    assert (
        j is not None
        and j.status == "running"
        and j.phase == "main"
        and "worktree prepared" in j.notes
    )
    wait_until(lambda: s.children[job.id].poll() is not None)
    s.tick()
    j = state.get_job(job.id)
    assert j is not None and j.status == "finished"
    console = Path(str(j.console_log)).read_text()
    assert (
        "prepared\n" in console
        and "main ran\n" in console
        and "prepare: bash" in console
        and "main: bash" in console
    )


def test_failed_prepare_fails_the_job(
    cfg: RunnerConfig, state: State, clock: Clock, tmp_path: Path, no_validation: None
) -> None:
    prep = script(tmp_path, "prep.sh", "echo refusing >&2\nexit 1\n")
    spec = gate_spec(
        cfg,
        ["bash", "x", "astra", "demo"],
        needs_prep=True,
        prep_argv=["bash", str(prep), "bench/t"],
        prep_env={},
    )
    job = state.create_job("gate", "astra-demo", spec)
    s = make_sup(cfg, state, clock)
    s.tick()
    wait_until(lambda: s.children[job.id].poll() is not None)
    s.tick()
    j = state.get_job(job.id)
    assert (
        j is not None
        and j.status == "failed"
        and any("run_worktree.sh exited 1" in n for n in j.notes)
    )


def test_launch_revalidates_the_plan(
    cfg: RunnerConfig, state: State, clock: Clock, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(
        launch, "validate_before_launch", lambda _cfg, _spec: ["HEAD is not at bench/t"]
    )
    job = state.create_job("gate", "astra-demo", gate_spec(cfg, ["bash", "x", "astra", "demo"]))
    s = make_sup(cfg, state, clock)
    s.tick()
    j = state.get_job(job.id)
    assert (
        j is not None
        and j.status == "failed"
        and j.notes == ["refused at launch: HEAD is not at bench/t"]
    )
    assert s.children == {}


def test_sweep_removes_only_this_jobs_compose_projects(
    cfg: RunnerConfig, state: State, clock: Clock, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    job_dir = cfg.gate_jobs_dir / "astra-demo"
    (job_dir / "demo__abc1234").mkdir(parents=True)
    containers = [
        Container(
            id="c1",
            name="demo__abc1234__env-main-1",
            status="Up",
            state="running",
            image="i",
            labels={"com.docker.compose.project": "demo__abc1234__env"},
        ),
        Container(
            id="c2",
            name="other__zzz__env-main-1",
            status="Up",
            state="running",
            image="i",
            labels={"com.docker.compose.project": "other__zzz__env"},
        ),
    ]
    calls: list[list[str]] = []

    def fake_run(argv: list[str], **_: Any) -> Any:
        calls.append(argv)

        class R:
            stdout = ""

        return R()

    monkeypatch.setattr(sup.subprocess, "run", fake_run)
    job = state.create_job("gate", "astra-demo", gate_spec(cfg, ["bash", "x", "astra", "demo"]))
    s = make_sup(cfg, state, clock, containers=lambda: containers)
    s._sweep(job)
    rm = [c for c in calls if c[1:3] == ["rm", "-f"]]
    assert len(rm) == 1 and rm[0][3:] == ["c1"]
    assert any("removed 1 leftover" in n for n in state.get_job(job.id).notes)  # type: ignore[union-attr]


def test_service_holds_one_lock_per_machine(cfg: RunnerConfig) -> None:
    a = SupervisorService()
    b = SupervisorService()
    a.start(cfg, interval_s=0.05)
    try:
        assert a.status == "active" and a.describe()["status"] == "active"
        b.start(cfg)
        assert b.status == "inactive" and str(os.getpid()) in str(b.detail)
        wait_until(lambda: a.describe().get("last_tick") is not None)
    finally:
        a.stop()
        b.stop()
    assert a.status == "stopped" and a.thread is None
    c = SupervisorService()
    c.start(cfg, interval_s=0.05)
    assert c.status == "active"
    c.stop()


def test_job_dataclass_roundtrip(state: State) -> None:
    job = state.create_job("gate", "n", {"a": 1})
    assert isinstance(job, Job) and job.to_dict()["spec"] == {"a": 1}


# ------------------------------------------------------------ suite levels --


def _suite_job(
    cfg: RunnerConfig, state: State, repo_tmp: Path, *, mode: str, quota_max: int = 0
) -> Any:
    """A queued suite level whose child is the fake vulcanbench from the fixtures."""
    repo = fake_repo(repo_tmp)
    wt = fake_worktree(repo, cfg)
    set_fake_run(wt, mode)
    out_dir = cfg.harness_runs_dir / "demo-sweep" / "low"
    cli = wt / ".venv" / "bin" / "vulcanbench"
    spec: dict[str, Any] = {
        "kind": "suite",
        "job_name": "demo-sweep-low",
        "sweep_id": "demo-sweep-abc",
        "sweep_name": "demo-sweep",
        "model": "gpt-6.1-terra",
        "harness": "codex",
        "suite": "demo-suite",
        "task_ids": ["alpha-task", "beta-task"],
        "n_tasks": 2,
        "effort": "low",
        "repeat": 1,
        "only_missing": False,
        "tag": "bench/2026-10-29-test",
        "worktree": str(wt),
        "needs_prep": False,
        "argv": [
            str(cli),
            "run",
            "--suite",
            "demo-suite",
            "--model",
            "gpt-6.1-terra",
            "--effort",
            "low",
            "-o",
            str(out_dir),
        ],
        "argv_token": f"-o {out_dir}",
        "cwd": str(wt),
        "env": {},
        "out_dir": str(out_dir),
        "quota_continue": {"max": quota_max, "wait_s": sweep.QUOTA_WAIT_S, "used": 0},
    }
    return state.create_job("suite", "demo-sweep-low", spec), wt, out_dir


def _run_to_end(s: Supervisor, state: State, job_id: int, timeout: float = 15.0) -> Job:
    s.tick()
    wait_until(
        lambda: s.children.get(job_id) is None or s.children[job_id].poll() is not None, timeout
    )
    s.tick()
    job = state.get_job(job_id)
    assert job is not None
    return job


def test_suite_level_finishes_from_the_output_directory(
    cfg: RunnerConfig, state: State, clock: Clock, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(launch, "REPO_ROOT", tmp_path / "repo")
    job, _wt, out_dir = _suite_job(cfg, state, tmp_path, mode="finish")
    s = make_sup(cfg, state, clock)
    done = _run_to_end(s, state, job.id)
    assert done.status == "finished" and done.exit_code == 0, done.notes
    assert len(list(out_dir.glob("*/summary.json"))) == 2
    argv = json.loads((out_dir / "argv.json").read_text())
    assert "VULCANBENCH_ALLOW_DIRTY" not in argv["env"] and argv["argv"][0] == "run"


def test_suite_level_with_missing_tasks_is_incomplete_not_retried(
    cfg: RunnerConfig, state: State, clock: Clock, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(launch, "REPO_ROOT", tmp_path / "repo")
    job, _wt, _out = _suite_job(cfg, state, tmp_path, mode="partial")
    s = make_sup(cfg, state, clock)
    done = _run_to_end(s, state, job.id)
    assert done.status == "incomplete" and "1 of 2 runs" in done.notes[-1]
    s.tick()
    assert state.queued_jobs() == [] and state.waiting_jobs() == []


def test_quota_window_waits_then_continues_only_as_often_as_allowed(
    cfg: RunnerConfig, state: State, clock: Clock, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(launch, "REPO_ROOT", tmp_path / "repo")
    job, _wt, out_dir = _suite_job(cfg, state, tmp_path, mode="quota", quota_max=1)
    s = make_sup(cfg, state, clock)
    waiting = _run_to_end(s, state, job.id)
    assert waiting.status == "waiting" and waiting.exit_code == 2
    assert waiting.spec["quota_continue"]["used"] == 1 and "--only-missing" in waiting.spec["argv"]
    assert "continues with --only-missing" in waiting.notes[-1]
    resume = waiting.spec["resume_at"]
    s.tick()  # not due yet
    assert state.get_job(job.id).status == "waiting"  # type: ignore[union-attr]
    clock.advance(sweep.QUOTA_WAIT_S + 1)
    again = _run_to_end(s, state, job.id)
    # The second run hits the window too; no continuation is left, so the level ends incomplete.
    assert again.status == "incomplete" and "all 1 continuations used" in again.notes[-1]
    assert again.spec["resume_at"] == resume
    assert (
        len(list(out_dir.glob("*/summary.json"))) == 1
    )  # the fake refuses the last task every time


def test_auth_failure_ends_the_level(
    cfg: RunnerConfig, state: State, clock: Clock, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(launch, "REPO_ROOT", tmp_path / "repo")
    job, _wt, _out = _suite_job(cfg, state, tmp_path, mode="auth", quota_max=5)
    s = make_sup(cfg, state, clock)
    done = _run_to_end(s, state, job.id)
    assert done.status == "failed" and "login expired" in done.notes[-1]


def test_stop_kills_the_agent_session_then_waits_for_the_harness(
    cfg: RunnerConfig, state: State, clock: Clock, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(launch, "REPO_ROOT", tmp_path / "repo")
    job, _wt, out_dir = _suite_job(cfg, state, tmp_path, mode="hang")
    killed: list[tuple[int, int]] = []

    def killpg(pgid: int, sig: int) -> None:
        killed.append((pgid, sig))
        os.killpg(pgid, sig)

    s = make_sup(cfg, state, clock, killpg=killpg)
    s.tick()
    wait_until((out_dir / "agent.pid").exists)
    agent_pid = int((out_dir / "agent.pid").read_text())
    state.request_stop(job.id, "test")
    s.tick()  # SIGINT to vulcanbench
    assert state.get_job(job.id).status == "stopping"  # type: ignore[union-attr]
    assert state.get_job(job.id).spec["agent_pgids_seen"] == [agent_pid]  # type: ignore[union-attr]
    clock.advance(sup.AGENT_KILL_AFTER_S + 1)
    s.tick()  # the agent's own session is killed, whether or not the harness is still there
    wait_until(lambda: job.id not in s.children or s.children[job.id].poll() is not None)
    s.tick()
    done = state.get_job(job.id)
    assert done is not None and done.status == "stopped" and done.exit_code == 130
    assert (agent_pid, signal.SIGKILL) in killed
    assert any("agent process group" in n for n in done.notes)
    assert (out_dir / "interrupted").read_text() == "sigint"
    wait_until(lambda: not _alive(agent_pid))


def _alive(pid: int) -> bool:
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    return True


def test_prepare_only_job_finishes_after_the_worktree_step(
    cfg: RunnerConfig, state: State, clock: Clock, tmp_path: Path, no_validation: None
) -> None:
    ok = script(tmp_path, "prep.sh", f'mkdir -p "{cfg.run_worktrees / "bench-t"}"\nexit 0\n')
    spec = {
        "kind": "prepare",
        "job_name": "prepare-bench-t",
        "tag": "bench/t",
        "worktree": str(cfg.run_worktrees / "bench-t"),
        "needs_prep": True,
        "argv": None,
        "prep_argv": ["bash", str(ok), "bench/t"],
        "prep_env": {"RUN_WORKTREES": str(cfg.run_worktrees)},
    }
    job = state.create_job("prepare", "prepare-bench-t", spec)
    s = make_sup(cfg, state, clock)
    done = _run_to_end(s, state, job.id)
    assert done.status == "finished" and "worktree ready" in done.notes[-1]
    again = state.create_job("prepare", "prepare-bench-t", spec)
    s.tick()
    assert state.get_job(again.id).status == "finished"  # type: ignore[union-attr]
    assert "already present" in state.get_job(again.id).notes[-1]  # type: ignore[union-attr]


def test_suite_sweep_touches_only_unfinished_runs(
    cfg: RunnerConfig, state: State, clock: Clock, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    out_dir = cfg.harness_runs_dir / "sw" / "low"
    (out_dir / "alpha-task-aaaaaaaa").mkdir(parents=True)
    (out_dir / "alpha-task-aaaaaaaa" / "summary.json").write_text("{}")
    (out_dir / "beta-task-bbbbbbbb").mkdir()
    containers = [
        Container(
            id="c1",
            name="x",
            status="Up",
            state="running",
            image="i",
            labels={"com.docker.compose.project": "vb-beta-task-bbbbbbbb"},
        ),
        Container(
            id="c2",
            name="y",
            status="Up",
            state="running",
            image="i",
            labels={"com.docker.compose.project": "vb-alpha-task-aaaaaaaa"},
        ),
        Container(
            id="c3",
            name="z",
            status="Up",
            state="running",
            image="i",
            mounts=f"{tmp_path}/vulcanbench-beta-task-bbbbbbbb-xyz/workspace",
        ),
    ]
    calls: list[list[str]] = []

    def fake_run(argv: list[str], **_: Any) -> Any:
        calls.append(argv)

        class R:
            stdout = ""

        return R()

    monkeypatch.setattr(sup.subprocess, "run", fake_run)
    monkeypatch.setattr(sup.tempfile, "gettempdir", lambda: str(tmp_path))
    (tmp_path / "vulcanbench-beta-task-bbbbbbbb-xyz").mkdir()
    (tmp_path / "vulcanbench-alpha-task-aaaaaaaa-q").mkdir()
    job = state.create_job("suite", "sw-low", {"kind": "suite", "out_dir": str(out_dir)})
    s = make_sup(cfg, state, clock, containers=lambda: containers)
    s._sweep_suite(job)
    rm = [c for c in calls if c[1:3] == ["rm", "-f"]]
    assert len(rm) == 1 and sorted(rm[0][3:]) == ["c1", "c3"]
    assert not (tmp_path / "vulcanbench-beta-task-bbbbbbbb-xyz").exists()
    assert (tmp_path / "vulcanbench-alpha-task-aaaaaaaa-q").exists()
