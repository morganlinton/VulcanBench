"""Builders for on-disk shapes the runner reads: harbor gate jobs and harness runs.

Kept as a plain module (imported by the runner tests) so each test can build
exactly the tree it needs under ``tmp_path``.
"""

from __future__ import annotations

import json
import shutil
import subprocess
from datetime import UTC, datetime, timedelta
from pathlib import Path
from typing import Any

from backend.runner.config import RunnerConfig
from backend.runner.docker_ps import Container
from backend.runner.procs import Proc

T0 = datetime(2026, 10, 29, 12, 0, 0, tzinfo=UTC)


def iso_z(dt: datetime) -> str:
    return dt.astimezone(UTC).strftime("%Y-%m-%dT%H:%M:%S.%fZ")


def make_config(tmp_path: Path, **overrides: Any) -> RunnerConfig:
    cfg = RunnerConfig(
        enabled=True,
        home=tmp_path / "runner",
        gate_out=tmp_path / "gate",
        run_worktrees=tmp_path / "worktrees",
        extra_run_roots=(),
        first_check_after_s=180,
    )
    for k, v in overrides.items():
        object.__setattr__(cfg, k, v)
    (cfg.gate_out / "jobs").mkdir(parents=True, exist_ok=True)
    (cfg.home / "runs").mkdir(parents=True, exist_ok=True)
    return cfg


def claude_stream(
    model: str = "claude-opus-5-5",
    *,
    turns: int = 3,
    tool_calls: int = 2,
    wrong_model_turns: int = 0,
    infra_line: str | None = None,
    rate_limit_status: str = "allowed",
    finished: bool = False,
) -> str:
    lines = [
        json.dumps(
            {"type": "system", "subtype": "init", "model": model, "claude_code_version": "2.1.302"}
        )
    ]
    remaining_tools = tool_calls
    for _ in range(turns):
        content: list[dict[str, Any]] = [{"type": "text", "text": "working"}]
        if remaining_tools > 0:
            content.append({"type": "tool_use", "name": "Bash", "input": {"command": "ls"}})
            remaining_tools -= 1
        lines.append(
            json.dumps({"type": "assistant", "message": {"model": model, "content": content}})
        )
    for _ in range(wrong_model_turns):
        lines.append(
            json.dumps(
                {"type": "assistant", "message": {"model": "claude-opus-4-8", "content": []}}
            )
        )
    lines.append(
        json.dumps(
            {
                "type": "rate_limit_event",
                "rate_limit_info": {"status": rate_limit_status, "unifiedWindows": {}},
            }
        )
    )
    if infra_line:
        lines.append(infra_line)
    if finished:
        lines.append(json.dumps({"type": "result", "subtype": "success", "num_turns": turns}))
    return "\n".join(lines) + "\n"


def codex_stream(*, commands: int = 3, finished: bool = True) -> str:
    lines = ["WARNING: proceeding", json.dumps({"type": "thread.started", "thread_id": "t1"})]
    for i in range(commands):
        lines.append(
            json.dumps({"type": "item.started", "item": {"type": "command_execution", "id": i}})
        )
        lines.append(
            json.dumps({"type": "item.completed", "item": {"type": "command_execution", "id": i}})
        )
    lines.append(
        json.dumps({"type": "item.completed", "item": {"type": "agent_message", "text": "done"}})
    )
    if finished:
        lines.append(json.dumps({"type": "turn.completed", "usage": {}}))
    return "\n".join(lines) + "\n"


def gate_job(
    cfg: RunnerConfig,
    job: str,
    *,
    agent: str = "claude-code",
    model: str = "claude-opus-5-5",
    task: str = "javaparser-member-resolution-scope",
    attempts: int = 3,
    trials: list[dict[str, Any]],
    finished: bool = True,
    started: datetime = T0,
    task_root: Path | None = None,
) -> Path:
    """Build ``<gate_out>/jobs/<job>`` the way harbor lays it out.

    Each trial dict: ``id``, ``reward`` (dict or None), optional ``exception``,
    ``running`` (no result.json), ``stream`` (text for the agent file).
    """
    job_dir = cfg.gate_out / "jobs" / job
    job_dir.mkdir(parents=True)
    worktree = task_root or cfg.run_worktrees / "wt"
    task_path = worktree / "tasks" / "frontier-v5" / task
    task_path.mkdir(parents=True, exist_ok=True)
    (worktree / ".git").mkdir(exist_ok=True)  # a checkout marker, so source lookup runs
    (job_dir / "config.json").write_text(
        json.dumps(
            {
                "job_name": job,
                "n_attempts": attempts,
                "n_concurrent_trials": 1,
                "agents": [
                    {"name": agent, "model_name": model, "kwargs": {"reasoning_effort": "medium"}}
                ],
                "tasks": [{"path": str(task_path)}],
            }
        )
    )
    n_done = sum(1 for t in trials if not t.get("running"))
    result: dict[str, Any] = {
        "started_at": started.astimezone().replace(tzinfo=None).isoformat(),
        "finished_at": (started + timedelta(hours=1)).astimezone().replace(tzinfo=None).isoformat()
        if finished
        else None,
        "n_total_trials": attempts,
        "stats": {
            "n_completed_trials": n_done,
            "n_running_trials": len(trials) - n_done,
            "cost_usd": 4.5,
        },
    }
    (job_dir / "result.json").write_text(json.dumps(result))
    (job_dir / "job.log").write_text("harbor job log\n")
    for i, t in enumerate(trials):
        tdir = job_dir / f"{task[:32]}__{t['id']}"
        (tdir / "agent").mkdir(parents=True)
        (tdir / "verifier").mkdir()
        (tdir / "config.json").write_text("{}")
        t_start = started + timedelta(minutes=20 * i)
        stream_name = "claude-code.txt" if agent == "claude-code" else "codex.txt"
        (tdir / "agent" / stream_name).write_text(t.get("stream", claude_stream(model)))
        (tdir / "trial.log").write_text(
            f"Running command: codex exec --model {model} --json -c x=y -- 'task'\n"
            if agent == "codex"
            else "claude --model x\n"
        )
        if t.get("running"):
            continue
        if t.get("reward") is not None:
            (tdir / "verifier" / "reward.json").write_text(json.dumps(t["reward"]))
        exc = t.get("exception")
        (tdir / "result.json").write_text(
            json.dumps(
                {
                    "trial_name": tdir.name,
                    "agent_info": {"name": agent, "model_info": {"name": model}},
                    "agent_result": {"cost_usd": 1.5},
                    "verifier_result": {"rewards": t["reward"]} if t.get("reward") else None,
                    "exception_info": {"exception_type": exc, "exception_message": "boom"}
                    if exc
                    else None,
                    "started_at": iso_z(t_start),
                    "finished_at": iso_z(t_start + timedelta(minutes=15)),
                    "agent_execution": {
                        "started_at": iso_z(t_start + timedelta(seconds=30)),
                        "finished_at": iso_z(t_start + timedelta(minutes=12)),
                    },
                }
            )
        )
        if exc:
            (tdir / "exception.txt").write_text(f"{exc}: boom\n")
    return job_dir


def harbor_proc(job: str, pid: int = 4242) -> Proc:
    return Proc(
        pid=pid,
        ppid=1,
        pgid=pid,
        stat="S",
        lstart="Thu Oct 29 08:00:00 2026",
        command=(
            f"/Users/x/.local/bin/python harbor run -p /wt/tasks/frontier-v5/t -a claude-code "
            f"-m claude-opus-5-5 -k 3 -n 1 -o /gate/jobs --job-name {job} -y -q"
        ),
    )


def trial_containers(trial_dirname: str) -> list[Container]:
    project = trial_dirname.lower() + "__env"
    return [
        Container(
            id="abc",
            name=f"{project}-main-1",
            status="Up 4 minutes",
            state="running",
            image=project,
            labels={"com.docker.compose.project": project, "com.docker.compose.service": "main"},
        ),
        Container(
            id="def",
            name="unrelated-1",
            status="Up 2 days",
            state="running",
            image="postgres",
            labels={"com.docker.compose.project": "other"},
        ),
    ]


def harness_run(
    root: Path,
    run_id: str,
    *,
    task_id: str = "hello-world",
    model: str = "claude-code:claude-opus-5-5",
    functional: float | None = 1.0,
    finished: bool = True,
    started: datetime = T0,
    suite: str | None = None,
) -> Path:
    d = root / run_id
    d.mkdir(parents=True)
    events = [
        {
            "ts": iso_z(started + timedelta(seconds=i)),
            "step": i + 1,
            "run_id": run_id,
            "task_id": task_id,
            "model": model,
            "type": "llm_response" if i % 2 else "tool_observation",
            "data": {},
        }
        for i in range(4)
    ]
    (d / "trace.jsonl").write_text("\n".join(json.dumps(e) for e in events) + "\n")
    (d / "cli-agent-stream.jsonl").write_text(claude_stream("claude-opus-5-5", finished=finished))
    if finished:
        (d / "summary.json").write_text(
            json.dumps(
                {
                    "run_id": run_id,
                    "task_id": task_id,
                    "model": model,
                    "steps": 4,
                    "scores": {"functional": functional, "total": functional},
                    "finished": True,
                    "started_at": iso_z(started),
                    "finished_at": iso_z(started + timedelta(minutes=9)),
                    "duration_s": 540,
                    "cost_usd": 0.42,
                    "suite": suite,
                    "effort": {"requested": "high"},
                    "cli_agent": {"harness": "claude-code", "requested_model": "claude-opus-5-5"},
                    "source": {"tag": "bench/2026-10-05-x", "commit": "abc1234"},
                }
            )
        )
    return d


# --------------------------------------------------------------- launches --

REPO = Path(__file__).resolve().parents[1]
GATE_SCRIPT_SRC = REPO / "scripts" / "frontier-v5" / "run_gate.sh"
WORKTREE_SCRIPT_SRC = REPO / "scripts" / "run_worktree.sh"
TAG_SCRIPT_SRC = REPO / "scripts" / "cut_bench_tag.sh"


def git(cwd: Path, *args: str) -> str:
    env = {
        "PATH": "/usr/bin:/bin:/usr/local/bin",
        "HOME": str(cwd),
        "GIT_AUTHOR_NAME": "t",
        "GIT_AUTHOR_EMAIL": "t@example.com",
        "GIT_COMMITTER_NAME": "t",
        "GIT_COMMITTER_EMAIL": "t@example.com",
    }
    out = subprocess.run(
        ["git", "-C", str(cwd), *args], capture_output=True, text=True, check=True, env=env
    )
    return out.stdout.strip()


def fake_repo(
    tmp_path: Path, *, tag: str = "bench/2026-10-29-test", tasks: tuple[str, ...] = ("demo-task",)
) -> Path:
    """A throwaway repository with run_gate.sh, run_worktree.sh and Frontier v5 tasks at ``tag``."""
    repo = tmp_path / "repo"
    (repo / "scripts" / "frontier-v5").mkdir(parents=True)
    shutil.copy(GATE_SCRIPT_SRC, repo / "scripts" / "frontier-v5" / "run_gate.sh")
    shutil.copy(WORKTREE_SCRIPT_SRC, repo / "scripts" / "run_worktree.sh")
    shutil.copy(TAG_SCRIPT_SRC, repo / "scripts" / "cut_bench_tag.sh")
    for name in tasks:
        d = repo / "tasks" / "frontier-v5" / name
        d.mkdir(parents=True)
        (d / "task.toml").write_text('[metadata]\nvulcanbench_language = "python"\n')
    suite = repo / "tasks" / SUITE_ID
    suite.mkdir(parents=True)
    (suite / "suite.lock.json").write_text(
        json.dumps(
            {
                "suite": SUITE_ID,
                "version": "1.0.0",
                "frozen_at": "2026-10-29",
                "tasks": {t: {"task_hash": "x"} for t in SUITE_TASKS},
            }
        )
    )
    (suite / "suite.json").write_text(json.dumps({"display_name": "VulcanBench Demo v1 (demo)"}))
    (repo / "harness").mkdir()
    (repo / "harness" / "__init__.py").write_text("")
    (repo / "harness" / "cli.py").write_text(
        '# fake\ndef harness_doctor(): ...\n# "--max-infra-retries"\n'
    )
    (repo / "vulcanbench.toml").write_text('[effort]\nblocked = ["ultra"]\n')
    git(repo, "init", "-q", "-b", "main")
    git(repo, "add", "-A")
    git(repo, "commit", "-q", "-m", "init")
    git(repo, "tag", tag)
    return repo


def fake_worktree(repo: Path, cfg: RunnerConfig, tag: str = "bench/2026-10-29-test") -> Path:
    """What run_worktree.sh would leave behind: a detached checkout at the tag with a .venv
    whose ``vulcanbench`` is a fake that answers doctor, estimate and run (see FAKE_CLI)."""
    wt = cfg.run_worktrees / tag.replace("/", "-")
    wt.parent.mkdir(parents=True, exist_ok=True)
    git(repo, "worktree", "add", "--detach", str(wt), tag)
    bin_dir = wt / ".venv" / "bin"
    bin_dir.mkdir(parents=True)
    (bin_dir / "python").write_text("")
    (bin_dir / "vulcanbench.py").write_text(FAKE_CLI)
    cli = bin_dir / "vulcanbench"
    cli.write_text('#!/bin/bash\nexec python3 "$(dirname "$0")/vulcanbench.py" "$@"\n')
    cli.chmod(0o755)
    return wt


def set_fake_run(worktree: Path, mode: str) -> None:
    """How the fake ``vulcanbench run`` behaves: finish | partial | quota | auth | hang | crash."""
    (worktree / ".fake-run-mode").write_text(mode)


SUITE_ID = "demo-suite"
SUITE_TASKS = ("alpha-task", "beta-task")

# The fake CLI. ``run`` reads <worktree>/.fake-run-mode (the child env is an
# allowlist, so a file is the only channel) and writes the harness's on-disk
# shapes into -o: <task>-<8hex>/summary.json for a finished task, a growing
# trace.jsonl for one that is running.
FAKE_CLI = r"""
import json, os, signal, sys, time, uuid
from pathlib import Path

args = sys.argv[1:]
here = Path(__file__).resolve().parents[2]  # the worktree
mode = (here / ".fake-run-mode").read_text().strip() if (here / ".fake-run-mode").exists() else "finish"


def opt(name, default=None):
    return args[args.index(name) + 1] if name in args else default


tasks = ["alpha-task", "beta-task"]
lock = here / "tasks" / str(opt("--suite", "")) / "suite.lock.json"
if lock.is_file():
    tasks = sorted(json.loads(lock.read_text())["tasks"])


if args[:2] == ["harness", "doctor"]:
    ready = not (here / ".fake-doctor-not-ready").exists()
    print(json.dumps([{"harness": args[2], "version": "9.9.9", "auth_mode": "subscription",
                       "plan_name": "max", "ready": ready, "detail": "" if ready else "not logged in"}]))
    sys.exit(0 if ready else 2)
if args[:1] == ["estimate"]:
    model = opt("--model", "")
    if "nova" in model:
        print(f"error no built-in pricing for model '{model}'; cannot estimate cost")
        sys.exit(0)
    print(json.dumps({"task_ids": tasks, "repeat": int(opt("--repeat", 1)), "judges": False,
                      "low_usd": 1.0, "mid_usd": 2.0, "high_usd": 3.0, "recommended_usd": 4.5,
                      "models": [{"model": model, "provider": model.split(":")[0], "env_var": "X",
                                  "n_runs": len(tasks), "low_usd": 1.0, "mid_usd": 2.0, "high_usd": 3.0,
                                  "recommended_usd": 4.5, "confidence": "low", "notes": ["defaults"],
                                  "per_task": []}]}))
    sys.exit(0)
if args[:1] != ["run"]:
    print("fake vulcanbench: unknown command", args, file=sys.stderr)
    sys.exit(1)

out = Path(opt("-o"))
out.mkdir(parents=True, exist_ok=True)
model = opt("--model")
effort = opt("--effort")
(out / "argv.json").write_text(json.dumps({"argv": args, "env": {k: v for k, v in os.environ.items()}}))
print(f"running suite {opt('--suite')} with {model} ...", flush=True)
done = {json.loads(p.read_text())["task_id"] for p in out.glob("*/summary.json")}
missing = [t for t in tasks if t not in done] if "--only-missing" in args else list(tasks)


def finish(task, functional=1.0):
    d = out / f"{task}-{uuid.uuid4().hex[:8]}"
    d.mkdir()
    (d / "trace.jsonl").write_text(json.dumps({"ts": "2026-10-29T12:00:00Z", "task_id": task, "model": model}) + "\n")
    (d / "summary.json").write_text(json.dumps({"run_id": d.name, "task_id": task, "model": model,
        "scores": {"functional": functional}, "finished": True, "cost_usd": 0.5, "duration_s": 60,
        "effort": {"requested": effort}, "suite": opt("--suite"),
        "started_at": "2026-10-29T12:00:00Z", "finished_at": "2026-10-29T12:01:00Z"}))


if mode == "finish":
    for t in missing:
        finish(t, 1.0 if t == tasks[0] else 0.0)
    print("pass@1=0.5")
    sys.exit(0)
if mode == "partial":
    if missing:
        finish(missing[0])
    print("1 run(s) errored")
    sys.exit(0)
if mode == "quota":
    # One task finishes, the next one is refused by the subscription window.
    if len(missing) > 1:
        finish(missing[0])
    if missing:
        d = out / f"{missing[-1]}-{uuid.uuid4().hex[:8]}"
        d.mkdir()
        (d / "trace.jsonl").write_text(json.dumps({"ts": "2026-10-29T12:00:00Z", "task_id": missing[-1]}) + "\n")
        (d / "cli-agent-stream.jsonl").write_text('{"type":"error","message":"You have hit your usage limit. limit reached"}\n')
    print("provider error usage limit reached")
    sys.exit(2)
if mode == "auth":
    print("provider error: refresh token has expired, please log out and sign in again")
    sys.exit(2)
if mode == "crash":
    sys.exit(7)
if mode == "hang":
    d = out / f"{missing[0]}-{uuid.uuid4().hex[:8]}"
    d.mkdir()
    (d / "trace.jsonl").write_text(json.dumps({"ts": "2026-10-29T12:00:00Z", "task_id": missing[0]}) + "\n")
    # An "agent" in its own session, the way the harness starts CLI agents.
    agent = __import__("subprocess").Popen(["sleep", "300"], start_new_session=True)
    (out / "agent.pid").write_text(str(agent.pid))

    def on_int(*_):
        (out / "interrupted").write_text("sigint")
        time.sleep(0.3)
        sys.exit(130)

    signal.signal(signal.SIGINT, on_int)
    for _ in range(3000):
        time.sleep(0.1)
        with (d / "trace.jsonl").open("a") as fh:
            fh.write(json.dumps({"ts": "2026-10-29T12:00:01Z", "step": 1}) + "\n")
    sys.exit(0)
print("unknown mode", mode, file=sys.stderr)
sys.exit(9)
"""
