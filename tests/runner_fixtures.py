"""Builders for on-disk shapes the runner reads: harbor gate jobs and harness runs.

Kept as a plain module (imported by the runner tests) so each test can build
exactly the tree it needs under ``tmp_path``.
"""

from __future__ import annotations

import json
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
