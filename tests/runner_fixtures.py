"""Builders for on-disk shapes the runner reads: harbor gate jobs and harness runs.

Kept as a plain module (imported by the runner tests) so each test can build
exactly the tree it needs under ``tmp_path``.
"""

from __future__ import annotations

import json
import shutil
import subprocess
from dataclasses import dataclass
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
        suites_home=tmp_path / "suites",
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


def fake_repo(tmp_path: Path, *, tag: str = "bench/2026-10-29-test") -> Path:
    """A throwaway repository with run_worktree.sh, cut_bench_tag.sh and a frozen suite at ``tag``."""
    repo = tmp_path / "repo"
    (repo / "scripts").mkdir(parents=True)
    shutil.copy(WORKTREE_SCRIPT_SRC, repo / "scripts" / "run_worktree.sh")
    shutil.copy(TAG_SCRIPT_SRC, repo / "scripts" / "cut_bench_tag.sh")
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
    # Scripts run through child_env() see no GIT_* variables and, on CI, no
    # global identity either; annotated tags need one in the repo config.
    git(repo, "config", "user.name", "t")
    git(repo, "config", "user.email", "t@example.com")
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
    (bin_dir / "fake_suite.py").write_text(FAKE_SUITE_CLI)
    (wt / ".fake-suite-password").write_text(FAKE_PASSWORD)
    cli = bin_dir / "vulcanbench"
    cli.write_text('#!/bin/bash\nexec python3 "$(dirname "$0")/vulcanbench.py" "$@"\n')
    cli.chmod(0o755)
    return wt


def set_fake_run(worktree: Path, mode: str) -> None:
    """How the fake ``vulcanbench run`` behaves: finish | partial | quota | auth | hang | crash."""
    (worktree / ".fake-run-mode").write_text(mode)


SUITE_ID = "demo-suite"
SUITE_TASKS = ("alpha-task", "beta-task")
CUSTOM_SUITE_ID = "acme-slice"
CUSTOM_TASKS = ("acme-fix-1", "acme-fix-2", "acme-fix-3")
SUITES_HOME_TAG = "bench/2026-11-03-suites-home"


def fake_custom_suite(
    cfg: RunnerConfig,
    suite_id: str = CUSTOM_SUITE_ID,
    tasks: tuple[str, ...] = CUSTOM_TASKS,
    *,
    frozen: bool = True,
) -> Path:
    """A suite in the suites home the way ``vulcanbench suite freeze`` leaves it (JSON only)."""
    d = cfg.suites_home / suite_id
    d.mkdir(parents=True, exist_ok=True)
    (d / "suite.json").write_text(
        json.dumps(
            {
                "id": suite_id,
                "display_name": "Acme slice",
                "version": "1.0.0",
                "repo": {"host": "github.com", "owner": "acme", "name": "api"},
                "tasks": list(tasks),
            }
        )
    )
    if frozen:
        (d / "suite.lock.json").write_text(
            json.dumps(
                {
                    "suite": suite_id,
                    "version": "1.0.0",
                    "frozen_at": "2026-11-03",
                    "tasks": {t: f"hash-{t}" for t in tasks},
                }
            )
        )
    return d


def tag_with_suites_home(repo: Path, tag: str = SUITES_HOME_TAG) -> str:
    """A newer bench tag whose harness CLI knows ``--suites-home``."""
    cli = repo / "harness" / "cli.py"
    cli.write_text(cli.read_text() + '# "--suites-home"\n')
    git(repo, "commit", "-q", "-am", "suites home")
    git(repo, "tag", tag)
    return tag


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


if args[:1] == ["suite"]:
    import runpy
    runpy.run_path(str(Path(__file__).with_name("fake_suite.py")), run_name="__main__")
    sys.exit(0)

tasks = ["alpha-task", "beta-task"]
lock = here / "tasks" / str(opt("--suite", "")) / "suite.lock.json"
if opt("--suites-home"):
    lock = Path(opt("--suites-home")) / str(opt("--suite", "")) / "suite.lock.json"
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


# ----------------------------------------------------------- suite builder --

BUILDER_TAG = "bench/2026-11-04-builder"
FAKE_PASSWORD = "correct horse battery staple"


def tag_with_builder(repo: Path, tag: str = BUILDER_TAG) -> str:
    """A bench tag whose harness has the suite builder (``harness/suite_builder/access.py``)."""
    cli = repo / "harness" / "cli.py"
    if "--suites-home" not in cli.read_text():
        cli.write_text(cli.read_text() + '# "--suites-home"\n')
    sb = repo / "harness" / "suite_builder"
    sb.mkdir(exist_ok=True)
    (sb / "__init__.py").write_text("")
    (sb / "access.py").write_text("# fake builder marker\n")
    git(repo, "add", "-A")
    git(repo, "commit", "-q", "-m", "suite builder")
    git(repo, "tag", tag)
    return tag


@dataclass
class BuilderEnv:
    cfg: RunnerConfig
    repo: Path
    wt: Path
    tmp: Path


def builder_env(tmp_path: Path, monkeypatch: Any) -> BuilderEnv:
    """A fake repo with a suite-builder bench tag, its prepared worktree and runner env vars."""
    from backend.runner import docker_ps, launch, procs, suites, sweep  # noqa: PLC0415

    repo = fake_repo(tmp_path)
    monkeypatch.setattr(launch, "REPO_ROOT", repo)
    monkeypatch.setattr(sweep, "REPO_ROOT", repo)
    cfg = make_config(tmp_path)
    tag_with_builder(repo)
    wt = fake_worktree(repo, cfg, BUILDER_TAG)
    monkeypatch.setenv("VULCANBENCH_RUNNER", "1")
    monkeypatch.setenv("VULCANBENCH_RUNNER_HOME", str(cfg.home))
    monkeypatch.setenv("VULCANBENCH_RUNNER_GATE_OUT", str(cfg.gate_out))
    monkeypatch.setenv("VULCANBENCH_RUNNER_RUN_WORKTREES", str(cfg.run_worktrees))
    monkeypatch.setenv("VULCANBENCH_RUNNER_SUITES_HOME", str(cfg.suites_home))
    monkeypatch.setattr(procs, "list_processes", lambda **_: [])
    monkeypatch.setattr(docker_ps, "list_containers", lambda **_: [])
    suites._unlock_failures.clear()
    suites._prs_cache.clear()
    return BuilderEnv(cfg, repo, wt, tmp_path)


def set_fake_suite(worktree: Path, name: str, value: str = "1") -> None:
    """Steer the fake ``vulcanbench suite``: prs-mode, scaffold-mode, not-configured, unlocked."""
    (worktree / f".fake-suite-{name}").write_text(value)


def fake_suite_calls(worktree: Path) -> list[dict[str, Any]]:
    path = worktree / ".fake-suite-calls.jsonl"
    if not path.is_file():
        return []
    return [json.loads(line) for line in path.read_text().splitlines() if line.strip()]


def fake_builder_task(
    cfg: RunnerConfig,
    suite_id: str,
    task: str,
    *,
    pr: int = 7,
    author: str | None = None,
    validate: str | None = None,
    accepted: bool = False,
) -> Path:
    """A task in a draft suite the way scaffold (and optionally author, validate) left it."""
    d = cfg.suites_home / suite_id
    if not (d / "suite.json").is_file():
        d.mkdir(parents=True, exist_ok=True)
        (d / "suite.json").write_text(
            json.dumps(
                {
                    "id": suite_id,
                    "display_name": "Acme draft",
                    "version": "0.1.0",
                    "repo": {"host": "github", "owner": "acme", "name": "api"},
                    "tasks": [],
                }
            )
        )
    t = d / task
    (t / "build").mkdir(parents=True)
    (t / "repo").mkdir()
    (t / "repo" / "main.py").write_text("print('base')\n")
    (t / "tests").mkdir()
    (t / "tests" / "vb_test_fix.py").write_text("# canary\ndef test_fix():\n    assert True\n")
    (t / "issue.md").write_text("DRAFT\n\nThe fix is needed.\n")
    (t / "gold_patch.diff").write_text("--- a/main.py\n+++ b/main.py\n@@ -1 +1 @@\n-x\n+y\n")
    (t / "metadata.json").write_text(
        json.dumps(
            {
                "id": task,
                "tests": {"fail_to_pass": ["tests/vb_test_fix.py"], "pass_to_pass": []},
                "image": "base",
                "builder": {"repo": "acme/api", "pr": pr, "status": "scaffolded"},
            }
        )
    )
    (t / "build" / "pr.json").write_text(json.dumps({"number": pr, "title": f"Fix bug {pr}"}))
    (t / "build" / "scaffold.json").write_text(
        json.dumps(
            {
                "task_id": task,
                "pr": pr,
                "base_commit": "a" * 40,
                "languages": ["python"],
                "image": "base",
                "gold_applies": True,
                "warnings": [],
                "scaffolded_at": "2026-11-04T10:00:00+00:00",
            }
        )
    )
    if author is not None:
        (t / "build" / "author.json").write_text(
            json.dumps(
                {
                    "task_id": task,
                    "status": author,
                    "harness": "codex",
                    "model": "gpt-6.1-terra",
                    "effort": "medium",
                    "finished_at": "2026-11-04T11:00:00+00:00",
                    "duration_s": 600,
                    "cost": {"cli_reported_usd": 1.25},
                    "changed": {"accepted": ["issue.md"], "rejected": []},
                    "warnings": [],
                }
            )
        )
    if validate is not None:
        (t / "build" / "validate.json").write_text(
            json.dumps(
                {
                    "task_id": task,
                    "status": validate,
                    "reasons": [] if validate == "PASS" else ["gold does not solve"],
                    "checks": {"gold_solves": validate == "PASS", "no_answer_in_repo": True},
                    "sandbox": "docker",
                    "checked_at": "2026-11-04T12:00:00+00:00",
                }
            )
        )
    if accepted:
        manifest = json.loads((d / "suite.json").read_text())
        manifest["tasks"] = [*manifest["tasks"], task]
        (d / "suite.json").write_text(json.dumps(manifest))
    return t


# The fake ``vulcanbench suite``. Steered by <worktree>/.fake-suite-* files (the
# child env is an allowlist). Every call is recorded with its argv and the
# names of its environment variables, never stdin; unlock records only
# whether stdin carried the expected password.
FAKE_SUITE_CLI = r"""
import json, os, re, sys, time
from pathlib import Path

args = sys.argv[1:]
assert args[0] == "suite"
args = args[1:]
here = Path(__file__).resolve().parents[2]
with (here / ".fake-suite-calls.jsonl").open("a") as fh:
    fh.write(json.dumps({"argv": sys.argv[1:], "env": sorted(os.environ)}) + "\n")


def flag(name):
    p = here / f".fake-suite-{name}"
    return p.read_text().strip() if p.exists() else None


def opt(name, default=None):
    return args[args.index(name) + 1] if name in args else default


def out(doc, code=0):
    print(json.dumps(doc))
    sys.exit(code)


def fail(code, message, exit_code, **extra):
    print(json.dumps({"schema": "vulcanbench.suite.error.v1",
                      "error": {"code": code, "message": message, **extra}}))
    print(f"error {message}", file=sys.stderr)
    sys.exit(exit_code)


def access():
    configured = flag("not-configured") is None
    unlocked = configured and flag("unlocked") is not None
    return {"schema": "vulcanbench.suite.access.v1", "configured": configured,
            "unlocked": unlocked, "unlocked_at": "2026-11-04T09:00:00+00:00" if unlocked else None,
            "unlock_file": "/home/x/.config/vulcanbench/suite-builder-unlock.json"}


def gate():
    st = access()
    if not st["configured"]:
        fail("builder_not_configured", "no suite builder password is set", 7)
    if not st["unlocked"]:
        fail("builder_locked", "the suite builder is for subscribers; unlock it", 7)


home = Path(opt("--suites-home", str(here / "suites")))
cmd = args[0]

if cmd == "access":
    out(access())
if cmd == "unlock":
    assert "--password-stdin" in args
    line = sys.stdin.readline()
    password = line[:-1] if line.endswith("\n") else line
    expected = flag("password")  # set by the test; never a literal here
    (here / ".fake-suite-stdin-ok").write_text("yes" if password == expected else "no")
    if flag("not-configured") is not None:
        fail("builder_not_configured", "no suite builder password is set", 7)
    if flag("echo") is not None:  # a misbehaving CLI: the secret in its output, no JSON
        print("debug: got " + password, file=sys.stderr)
        print("password was " + password)
        sys.exit(1)
    if password != expected:
        fail("wrong_password", "that is not the suite builder password", 7)
    (here / ".fake-suite-unlocked").write_text("1")
    out(access())
if cmd == "lock":
    (here / ".fake-suite-unlocked").unlink(missing_ok=True)
    out(access())

gate()

if cmd == "init":
    sid = args[1]
    if not re.match(r"^[a-z0-9][a-z0-9-]{2,63}$", sid):
        fail("invalid_id", "bad id", 2)
    if sid == "demo-suite":
        fail("id_collides", "suite id is a suite in the checkout", 2)
    d = home / sid
    if d.exists():
        fail("exists", f"{d} already exists", 8, path=str(d))
    d.mkdir(parents=True)
    owner, name = opt("--repo").split("/", 1)
    manifest = {"id": sid, "display_name": opt("--display-name"), "version": "0.1.0",
                "repo": {"host": "github", "owner": owner, "name": name}, "tasks": [],
                "updated_at": "2026-11-04T09:00:00+00:00"}
    (d / "suite.json").write_text(json.dumps(manifest))
    out({"schema": "vulcanbench.suite.init.v1", "id": sid, "path": str(d), "manifest": manifest})

if cmd == "prs":
    mode = flag("prs-mode") or "ok"
    if mode == "gh_auth":
        fail("gh_auth", "gh is not logged in", 3)
    if mode == "network":
        fail("gh_failed", "gh pr list failed: could not resolve host", 4)
    if mode == "hang":
        time.sleep(30)
    n = int(opt("--limit", 50))
    prs = [{"number": 40 + i, "title": f"Fix thing {i}", "url": f"https://github.com/acme/api/pull/{40 + i}",
            "merged_at": "2026-10-01", "additions": 30, "deletions": 5, "changed_files": 3,
            "test_files": ["tests/t.py"], "src_files": ["a.py"], "languages": ["python"],
            "suitability": {"score": 90 - i, "reasons": ["has tests"], "flags": []}, "task_id": None}
           for i in range(min(n, 3))]
    out({"schema": "vulcanbench.suite.prs.v1", "repo": "acme/api", "since": opt("--since"), "prs": prs})

if args[:2] == ["task", "scaffold"]:
    sid = args[2]
    pr = int(opt("--pr"))
    if (flag("scaffold-mode") or "ok") == "network":
        fail("gh_failed", "git clone failed", 4)
    d = home / sid
    tid = f"pr-{pr}-fix-thing"
    t = d / tid
    if t.exists():
        fail("exists", f"{t} exists", 8)
    (t / "build").mkdir(parents=True)
    (t / "repo").mkdir()
    (t / "repo" / "a.py").write_text("x = 1\n")
    note = opt("--license-note")
    if note:
        (t / "repo" / "NOTICE").write_text(note + "\n")
    no_license = flag("no-license") is not None and not note
    (t / "issue.md").write_text("<!-- DRAFT -->\nFix thing\n")
    (t / "gold_patch.diff").write_text("--- a/a.py\n+++ b/a.py\n")
    (t / "metadata.json").write_text(json.dumps({"id": tid, "tests": {"fail_to_pass": [], "pass_to_pass": []},
                                                 "builder": {"repo": "acme/api", "pr": pr, "status": "scaffolded"}}))
    (t / "build" / "pr.json").write_text(json.dumps({"number": pr, "title": "Fix thing"}))
    report = {"schema": "vulcanbench.suite.scaffold.v1", "task_dir": str(t), "task_id": tid, "pr": pr,
              "base_commit": "b" * 40, "languages": ["python"], "image": "base", "gold_applies": True,
              "warnings": ["no_license: repo/ has no LICENSE or NOTICE; pass --license-note"] if no_license else [],
              "scaffolded_at": "2026-11-04T10:00:00+00:00"}
    (t / "build" / "scaffold.json").write_text(json.dumps(report))
    out(report)

if cmd in ("add-task", "remove-task"):
    sid = args[1]
    task = opt("--task")
    d = home / sid
    manifest = json.loads((d / "suite.json").read_text())
    tasks = list(manifest.get("tasks") or [])
    if cmd == "add-task":
        v = d / task / "build" / "validate.json"
        status = json.loads(v.read_text())["status"] if v.exists() else None
        if status not in ("PASS", "WARN"):
            fail("not_validated", f"task {task} is not validated", 1, validation_status=status)
        if task not in tasks:
            tasks.append(task)
    else:
        if task not in tasks:
            fail("not_found", f"task {task} is not in suite", 5)
        tasks = [t for t in tasks if t != task]
    manifest["tasks"] = tasks
    (d / "suite.json").write_text(json.dumps(manifest))
    out({"schema": f"vulcanbench.suite.{cmd}.v1", "id": sid, "task_id": task, "tasks": tasks})

if cmd == "freeze":
    sid = args[1]
    d = home / sid
    manifest = json.loads((d / "suite.json").read_text())
    tasks = list(manifest.get("tasks") or [])
    if not tasks:
        fail("empty_suite", "no admitted tasks", 2, problems=["tasks list is empty"])
    lock = {"suite": sid, "version": manifest.get("version"), "frozen_at": "2026-11-04",
            "tasks": {t: {"task_hash": "h-" + t} for t in tasks}}
    (d / "suite.lock.json").write_text(json.dumps(lock))
    out({"schema": "vulcanbench.suite.freeze.v1", "id": sid, "path": str(d / "suite.lock.json"),
         "version": lock["version"], "lock_digest": "d" * 16, "n_tasks": len(tasks)})

def now():
    from datetime import datetime, timezone  # the fake may run on an older system python3
    return datetime.now(timezone.utc).isoformat(timespec="seconds")


if args[:2] == ["task", "author"]:
    sid, task = args[2], opt("--task")
    t = home / sid / task
    mode = flag("author-mode") or "finish"
    print(f"authoring {task} with {opt('--harness')}:{opt('--model')}", file=sys.stderr, flush=True)
    if mode == "refuse":
        fail("harness_not_ready", "codex is not ready: not logged in", 3)
    if mode == "crash":
        sys.exit(9)
    if mode == "hang":
        import signal, subprocess
        agent = subprocess.Popen(["sleep", "300"], start_new_session=True)
        (here / ".fake-suite-agent.pid").write_text(str(agent.pid))

        def on_int(*_):
            sys.exit(130)

        signal.signal(signal.SIGINT, on_int)
        for _ in range(3000):
            time.sleep(0.1)
        sys.exit(0)
    status = "ok" if mode == "finish" else "failed"
    doc = {"schema": "vulcanbench.suite.author.v1", "task_id": task, "harness": opt("--harness"),
           "model": opt("--model"), "effort": opt("--effort"), "status": status,
           "started_at": now(), "finished_at": now(), "duration_s": 1.0,
           "changed": {"accepted": ["issue.md", "tests/vb_test.py"] if status == "ok" else [], "rejected": ["repo/a.py"]},
           "cost": {"cli_reported_usd": 0.5}, "warnings": []}
    (t / "build" / "author.json").write_text(json.dumps(doc))
    if status == "ok":  # what a real session leaves: a final issue and one hidden test
        (t / "issue.md").write_text("Fix thing\n\nThe handler crashes on an empty header.\n")
        (t / "tests").mkdir(exist_ok=True)
        (t / "tests" / "vb_test.py").write_text("def test_empty_header():\n    assert True\n")
    if mode == "quota":
        print("provider error: You have hit your usage limit. limit reached", file=sys.stderr)
        out(doc, 1)
    out(doc, 0 if status == "ok" else 6)

if cmd == "validate":
    sid, task = args[1], opt("--task")
    assert opt("--sandbox") == "docker"
    mode = flag("validate-mode") or "pass"
    if mode == "infra":
        fail("docker_unavailable", "--sandbox docker requires a running Docker daemon", 3)
    status = {"pass": "PASS", "fail": "FAIL", "skip": "SKIP"}[mode]
    rec = {"schema": "vulcanbench.suite.validate.v1", "task_id": task, "status": status,
           "reasons": [] if status == "PASS" else ["base does not fail"],
           "checks": {"no_answer_in_repo": True, "gold_applies": True}, "sandbox": "docker",
           "checked_at": now()}
    (home / sid / task / "build" / "validate.json").write_text(json.dumps(rec))
    out({"schema": "vulcanbench.suite.validate-report.v1", "id": sid, "results": [rec]},
        {"PASS": 0, "FAIL": 1, "SKIP": 4}[status])

print("fake vulcanbench suite: unknown command", args, file=sys.stderr)
sys.exit(9)
"""
