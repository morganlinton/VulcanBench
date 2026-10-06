#!/usr/bin/env python3
"""Run committed benchmark definitions from a dedicated clone, outside authoring."""

from __future__ import annotations

import argparse
import contextlib
import fcntl
import hashlib
import json
import os
import re
import shutil
import signal
import subprocess
import sys
import time
import uuid
from collections.abc import Iterator
from datetime import UTC, datetime
from pathlib import Path
from typing import Any, TextIO

CONFIG = Path.home() / ".config/vulcanbench/runner.json"
LOCK = Path.home() / ".local/state/vulcanbench/execution.lock"
MARKER = "vulcanbench-runner.json"
PUSH_DISABLED = "disabled://benchmark-runner-read-only"


def now() -> str:
    return datetime.now(UTC).isoformat()


def write_json(path: Path, data: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(".tmp")
    temporary.write_text(json.dumps(data, indent=2) + "\n")
    temporary.replace(path)


def command(args: list[str], cwd: Path | None = None, **kwargs: Any) -> str:
    result: str = subprocess.check_output(args, cwd=cwd, text=True, **kwargs)
    return result.strip()


def git(repo: Path, *args: str) -> str:
    return command(["git", "-C", str(repo), *args])


def disjoint(*paths: Path) -> None:
    resolved = [p.resolve() for p in paths]
    for i, a in enumerate(resolved):
        for b in resolved[i + 1 :]:
            if a.is_relative_to(b) or b.is_relative_to(a):
                raise ValueError(f"Directories must be separate, not nested: {a}, {b}")


@contextlib.contextmanager
def execution_lock(path: Path = LOCK) -> Iterator[TextIO]:
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.with_suffix(".review.json").exists():
        raise ValueError(
            f"Interrupted process cleanup needs review: {path.with_suffix('.review.json')}"
        )
    with path.open("a+") as handle:
        try:
            fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as exc:
            raise ValueError(f"Another runner operation holds {path}") from exc
        handle.seek(0)
        handle.truncate()
        handle.write(json.dumps({"pid": os.getpid(), "started_at": now()}))
        handle.flush()
        try:
            yield handle
        finally:
            fcntl.flock(handle, fcntl.LOCK_UN)


def clean(repo: Path) -> None:
    if git(repo, "status", "--porcelain", "--untracked-files=all"):
        raise ValueError(f"Runner checkout has local changes: {repo}. Preserve and review them.")
    extras = git(
        repo,
        "ls-files",
        "--others",
        "--ignored",
        "--exclude-standard",
        "--",
        "tasks",
        "harness",
        "scripts",
    ).splitlines()
    if any("__pycache__" not in Path(p).parts for p in extras):
        raise ValueError("Ignored files in runner source/task directories require review")


def load_config(path: Path) -> dict[str, str]:
    config: dict[str, str] = json.loads(path.read_text())
    source, runner, results = (Path(config[k]).resolve() for k in ("source", "runner", "results"))
    disjoint(source, runner, results)
    # Linked worktrees share Git administration; execution uses an independent clone.
    if not (runner / ".git").is_dir():
        raise ValueError("Runner must be an independent clone, not an authoring worktree")
    marker = json.loads((runner / ".git" / MARKER).read_text())
    if marker != config:
        raise ValueError("Runner configuration does not match its ownership marker")
    if git(runner, "remote", "get-url", "--push", "origin") != PUSH_DISABLED:
        raise ValueError("Runner origin push URL must remain disabled")
    return config


def initialize(args: argparse.Namespace) -> None:
    source, runner, results = args.source.resolve(), args.runner.resolve(), args.results.resolve()
    disjoint(source, runner, results)
    if args.config.exists() or runner.exists():
        raise ValueError("Config or runner already exists; inspect it instead of replacing it")
    origin = git(source, "remote", "get-url", "origin")
    revision = git(source, "rev-parse", "HEAD")
    runner.parent.mkdir(parents=True, exist_ok=True)
    # Copy Git objects, never share hardlinks or alternates with the authoring repository.
    subprocess.run(
        ["git", "clone", "--no-hardlinks", "--no-checkout", str(source), str(runner)], check=True
    )
    git(runner, "remote", "set-url", "origin", origin)
    git(runner, "remote", "set-url", "--push", "origin", PUSH_DISABLED)
    git(runner, "checkout", "--detach", revision)
    results.mkdir(parents=True, exist_ok=True)
    config = {
        "source": str(source),
        "runner": str(runner),
        "results": str(results),
        "origin": origin,
    }
    write_json(runner / ".git" / MARKER, config)
    write_json(args.config, config)
    print(f"Created runner at {runner}; results at {results}")


def exact_revision(value: str) -> str:
    if not re.fullmatch(r"[0-9a-f]{40}", value):
        raise argparse.ArgumentTypeError(
            "Use the full 40-character commit SHA, not a branch or tag"
        )
    return value


def child_environment(runner: Path) -> dict[str, str]:
    env = os.environ.copy()
    for key in ("PYTHONPATH", "PYTHONHOME", "VIRTUAL_ENV", "UV_PROJECT_ENVIRONMENT"):
        env.pop(key, None)
    env["PYTHONNOUSERSITE"] = "1"
    env["PYTHONDONTWRITEBYTECODE"] = "1"
    env["PATH"] = str(runner / ".venv/bin") + os.pathsep + env.get("PATH", "")
    return env


def prepare(config: dict[str, str], revision: str) -> None:
    runner = Path(config["runner"])
    clean(runner)
    if active_benchmarks(runner):
        raise ValueError("An unmanaged benchmark is using the runner checkout")
    if subprocess.run(
        ["git", "-C", str(runner), "cat-file", "-e", revision + "^{commit}"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        check=False,
    ).returncode:
        git(runner, "fetch", "origin", revision)
    git(runner, "checkout", "--detach", revision)
    # Refuse an unresolved LFS pointer rather than benchmark the wrong fixture bytes.
    git(runner, "lfs", "pull")
    env = child_environment(runner)
    subprocess.run(
        ["uv", "sync", "--locked", "--extra", "dev", "--extra", "test", "--python", sys.executable],
        cwd=runner,
        env=env,
        check=True,
    )
    clean(runner)
    write_json(
        runner / ".git" / "PREPARED.json",
        {
            "revision": revision,
            "environment": environment_inventory(runner, env),
            "uv_lock_sha256": hashlib.sha256((runner / "uv.lock").read_bytes()).hexdigest(),
        },
    )
    print(f"Prepared {revision} in {runner}")


def environment_inventory(runner: Path, env: dict[str, str]) -> dict[str, Any]:
    code = "import importlib.metadata as m,json,sys; print(json.dumps({'python':sys.version, 'packages':sorted((d.metadata['Name'],d.version) for d in m.distributions())}))"
    result: dict[str, Any] = json.loads(
        command([str(runner / ".venv/bin/python"), "-B", "-c", code], cwd=runner, env=env)
    )
    return result


def active_benchmarks(repo: Path | None = None) -> list[int]:
    listing = command(["ps", "-axo", "pid=,ppid=,args="])
    processes = {}
    for line in listing.splitlines():
        pid, ppid, cmd = line.strip().split(None, 2)
        processes[int(pid)] = (int(ppid), cmd)
    ancestors = {os.getpid()}
    parent = os.getppid()
    while parent in processes and parent not in ancestors:
        ancestors.add(parent)
        parent = processes[parent][0]
    pattern = re.compile(
        r"(?:^|\s)(?:\S*/)?vulcanbench\s+(?:run|effort-sweep)\b|"
        r"(?:^|\s)-m\s+harness\.cli\s+(?:run|effort-sweep)\b"
    )
    return [
        pid
        for pid, (_, cmd) in processes.items()
        if pid not in ancestors and pattern.search(cmd) and (repo is None or str(repo) in cmd)
    ]


INSPECT = """
import hashlib, importlib.metadata, json, sys
from pathlib import Path
import harness
from harness.tasks import load_task, task_hash
from harness.suite import load_suite
root = Path.cwd()
assert Path(harness.__file__).resolve().is_relative_to(root), "Foreign harness import"
spec = json.loads(sys.argv[1])
if spec['suite']:
    suite = load_suite(spec['suite'])
    tasks = [load_task(t, suite.tasks_root) for t in suite.task_ids]
else:
    tasks = [load_task(spec['task'], Path(spec['tasks_root']))]
assert tasks, "No tasks selected"
for task in tasks:
    assert task.root.resolve().is_relative_to(root / 'tasks'), "Task outside committed tree"
    if task.snapshot:
        assert not task.snapshot.read_bytes().startswith(b'version https://git-lfs.github.com/spec/'), "LFS pointer"
print(json.dumps({
 'tasks': [{'id': t.task_id, 'task_hash': task_hash(t), 'metadata': t.metadata} for t in tasks],
 'python': sys.version,
 'packages': sorted((d.metadata['Name'], d.version) for d in importlib.metadata.distributions()),
 'harness_path': str(Path(harness.__file__).resolve()),
 'settings_sha256': hashlib.sha256((root / 'vulcanbench.toml').read_bytes()).hexdigest()
}))
"""


def selection(args: argparse.Namespace) -> dict[str, Any]:
    if args.suite and not re.fullmatch(r"[A-Za-z0-9_-]+", args.suite):
        raise ValueError("Suite must be a name, not a path")
    if args.task and not re.fullmatch(r"[A-Za-z0-9_-]+", args.task):
        raise ValueError("Task must be an ID, not a path")
    root = Path(args.tasks_root)
    if root.is_absolute() or ".." in root.parts or not root.parts or root.parts[0] != "tasks":
        raise ValueError("--tasks-root must be a relative path inside tasks/")
    if args.repeat < 1:
        raise ValueError("--repeat must be positive")
    return {"suite": args.suite, "task": args.task, "tasks_root": str(root)}


def run_command(args: argparse.Namespace, runner: Path, output: Path) -> list[str]:
    cmd = [str(runner / ".venv/bin/python"), "-B", "-m", "harness.cli", "run"]
    if args.suite:
        cmd += ["--suite", args.suite]
    else:
        cmd += ["--task", args.task, "--tasks-root", args.tasks_root]
    cmd += [
        "--model",
        args.model,
        "--harness",
        args.harness,
        "--billing",
        args.billing,
        "--sandbox",
        args.sandbox,
        "--repeat",
        str(args.repeat),
        "--max-concurrency",
        "1",
        "--output-dir",
        str(output),
        "--judges" if args.judges else "--no-judges",
        "--agent-container" if args.agent_container else "--no-agent-container",
    ]
    if args.effort:
        cmd += ["--effort", args.effort]
    return cmd


def tool_versions(args: argparse.Namespace, env: dict[str, str]) -> dict[str, Any]:
    tools = {"git": "--version", "ocamlc": "-version", "dune": "--version", "opam": "--version"}
    if args.harness in {"codex", "claude-code"}:
        tools["claude" if args.harness == "claude-code" else "codex"] = "--version"
    versions = {}
    for name, flag in tools.items():
        executable = shutil.which(name, path=env["PATH"])
        if executable:
            result = subprocess.run(
                [executable, flag], env=env, capture_output=True, text=True, timeout=10, check=False
            )
            versions[name] = {
                "path": executable,
                "version": result.stdout.strip(),
                "exit_code": result.returncode,
            }
    return versions


def descendants(parent: int) -> set[int]:
    rows = command(["ps", "-axo", "pid=,ppid="]).splitlines()
    pairs = [tuple(map(int, line.split())) for line in rows]
    family = {parent}
    while True:
        expanded = family | {pid for pid, ppid in pairs if ppid in family}
        if expanded == family:
            return family - {parent}
        family = expanded


def live_pids(pids: set[int]) -> list[int]:
    if not pids:
        return []
    result = subprocess.run(
        ["ps", "-p", ",".join(map(str, sorted(pids))), "-o", "pid=,stat="],
        capture_output=True,
        text=True,
        check=False,
    )
    return [
        int(row.split()[0])
        for row in result.stdout.splitlines()
        if not row.split()[1].startswith("Z")
    ]


def execute(cmd: list[str], runner: Path, output: Path, env: dict[str, str], lock: TextIO) -> int:
    with (output / "console.log").open("w") as log:
        child = subprocess.Popen(
            cmd,
            cwd=runner,
            env=env,
            stdout=log,
            stderr=subprocess.STDOUT,
            start_new_session=True,
            pass_fds=(lock.fileno(),),
        )
        old = {}
        interrupted_children: set[int] = set()

        def forward(signum: int, _frame: Any) -> None:
            if child.poll() is None:
                # Provider CLIs start separate sessions; signal their descendant PIDs too.
                interrupted_children.update(descendants(child.pid))
                for pid in interrupted_children:
                    with contextlib.suppress(ProcessLookupError):
                        os.kill(pid, signum)
                with contextlib.suppress(ProcessLookupError):
                    os.killpg(child.pid, signum)

        for sig in (signal.SIGINT, signal.SIGTERM):
            old[sig] = signal.signal(sig, forward)
        try:
            return child.wait()
        finally:
            for sig, handler in old.items():
                signal.signal(sig, handler)
            if child.poll() is None:
                child.terminate()
                child.wait()
            deadline = time.monotonic() + 5
            remaining = live_pids(interrupted_children)
            while remaining and time.monotonic() < deadline:
                time.sleep(0.1)
                remaining = live_pids(interrupted_children)
            if remaining:
                review = Path(lock.name).with_suffix(".review.json")
                write_json(review, {"pids": remaining, "experiment": str(output), "time": now()})
                raise ValueError(f"Interrupted descendants still running; review {review}")


def launch(args: argparse.Namespace, config: dict[str, str], lock: TextIO) -> int:
    runner, results = Path(config["runner"]), Path(config["results"])
    spec = selection(args)
    clean(runner)
    if git(runner, "rev-parse", "HEAD") != args.revision:
        raise ValueError("Runner revision differs; prepare the requested commit first")
    if (
        subprocess.run(
            ["git", "-C", str(runner), "symbolic-ref", "-q", "HEAD"],
            stdout=subprocess.DEVNULL,
            check=False,
        ).returncode
        == 0
    ):
        raise ValueError("Runner must have detached HEAD")
    active = active_benchmarks()
    if active and not args.check_only:
        raise ValueError(f"Existing benchmark processes require waiting: {active}")
    env = child_environment(runner)
    python = str(runner / ".venv/bin/python")
    prepared = json.loads((runner / ".git" / "PREPARED.json").read_text())
    if prepared["revision"] != args.revision or prepared["environment"] != environment_inventory(
        runner, env
    ):
        raise ValueError("Runner dependencies differ from preparation receipt; prepare again")
    if prepared["uv_lock_sha256"] != hashlib.sha256((runner / "uv.lock").read_bytes()).hexdigest():
        raise ValueError("Runner dependency lock differs from preparation receipt")
    inputs = json.loads(
        command([python, "-B", "-c", INSPECT, json.dumps(spec)], cwd=runner, env=env)
    )
    stamp = datetime.now(UTC).strftime("%Y%m%dT%H%M%SZ")
    output = results / f"{stamp}-{args.revision[:12]}-{uuid.uuid4().hex[:8]}"
    output.mkdir(parents=True, exist_ok=False)
    cmd = run_command(args, runner, output / "runs")
    manifest: dict[str, Any] = {
        "schema_version": 1,
        "started_at": now(),
        "revision": args.revision,
        "git_tree": git(runner, "rev-parse", "HEAD^{tree}"),
        "origin": config["origin"],
        "runner": str(runner),
        "output": str(output),
        "command": cmd,
        "inputs": inputs,
        "launcher_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        "mode": "preflight" if args.check_only else "benchmark",
        "active_legacy_benchmark_pids": active,
        "status": "preflight" if args.check_only else "running",
        "tools": tool_versions(args, env),
        "path_sha256": hashlib.sha256(env["PATH"].encode()).hexdigest(),
        "native_isolation": "observational; separate Git state is not OS isolation",
    }
    write_json(output / "experiment.json", manifest)
    print(f"Experiment receipt: {output / 'experiment.json'}", flush=True)
    if args.check_only:
        return 0
    try:
        code = execute(cmd, runner, output, env, lock)
        manifest["exit_code"] = code
        manifest["status"] = "completed" if code == 0 else "failed"
        clean(runner)
        if git(runner, "rev-parse", "HEAD") != args.revision:
            raise ValueError("Runner revision changed during execution")
    except BaseException as exc:
        manifest["status"] = "failed"
        manifest["error"] = f"{type(exc).__name__}: {exc}"
        raise
    finally:
        manifest["finished_at"] = now()
        write_json(output / "experiment.json", manifest)
    return code


def parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--config", type=Path, default=CONFIG)
    sub = p.add_subparsers(dest="action", required=True)
    init = sub.add_parser("init", help="Create a separate clone and external results directory")
    init.add_argument("--source", type=Path, required=True)
    init.add_argument("--runner", type=Path, required=True)
    init.add_argument("--results", type=Path, required=True)
    prep = sub.add_parser("prepare", help="Check out a commit and sync locked Python dependencies")
    prep.add_argument("--revision", type=exact_revision, required=True)
    run = sub.add_parser("run", help="Record inputs and execute from the prepared commit")
    run.add_argument("--revision", type=exact_revision, required=True)
    group = run.add_mutually_exclusive_group(required=True)
    group.add_argument("--suite")
    group.add_argument("--task")
    run.add_argument("--tasks-root", default="tasks/v1")
    run.add_argument("--model", required=True)
    run.add_argument("--harness", default="vulcan")
    run.add_argument("--billing", choices=["auto", "api", "subscription"], default="auto")
    run.add_argument("--sandbox", choices=["local", "docker"], required=True)
    run.add_argument("--effort")
    run.add_argument("--repeat", type=int, default=1)
    run.add_argument("--judges", action=argparse.BooleanOptionalAction, default=False)
    run.add_argument("--agent-container", action=argparse.BooleanOptionalAction, default=False)
    run.add_argument(
        "--check-only", action="store_true", help="Record preflight only; no model calls"
    )
    return p


def main() -> int:
    args = parser().parse_args()
    try:
        with execution_lock() as lock:
            if args.action == "init":
                initialize(args)
            else:
                config = load_config(args.config)
                if args.action == "prepare":
                    prepare(config, args.revision)
                else:
                    return launch(args, config, lock)
        return 0
    except (ValueError, OSError, subprocess.CalledProcessError) as exc:
        print(f"Runner stopped: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
