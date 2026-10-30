"""Find runs on disk and describe them as RunView objects.

Two sources:
- Frontier v5 gate jobs: ``<gate_out>/jobs/<job>/`` written by ``harbor run``
  through ``scripts/frontier-v5/run_gate.sh``.
- Harness runs: ``<root>/<run_id>/`` written by ``vulcanbench run`` (one run
  dir per task attempt, ``trace.jsonl`` while running, ``summary.json`` at
  the end).

Nothing here starts, stops or writes anything. Liveness for a run the queue
does not own comes from ``ps``; a null ``finished_at`` in harbor's
``result.json`` means "not finished", never "running".
"""

from __future__ import annotations

import json
import os
import subprocess
import time
from collections.abc import Callable
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

from backend.runner import firstcheck
from backend.runner.config import RunnerConfig
from backend.runner.docker_ps import Container, list_containers
from backend.runner.models import ContainerView, RunView, TrialView
from backend.runner.procs import Proc, find_harbor_job, find_vulcanbench_runs, list_processes
from backend.runner.state import Job

GATE_PREFIX = "gate:"
HARNESS_PREFIX = "harness:"
JOB_PREFIX = "job:"
CONSOLE_LOG = "console.log"

# Files the log endpoint may serve, relative to a job, trial or run dir.
# Nothing under agent/sessions or agent/setup is ever listed or served.
GATE_JOB_LOGS = ("job.log",)
GATE_TRIAL_LOGS = (
    "trial.log",
    "agent/claude-code.txt",
    "agent/codex.txt",
    "verifier/test-stdout.txt",
    "verifier/check.err",
    "verifier/compile.log",
    "exception.txt",
)
HARNESS_RUN_LOGS = ("trace.jsonl", "cli-agent-stream.jsonl", "final.patch")
HARNESS_STALE_S = 15 * 60
SOURCE_CACHE_TTL_S = 60.0

SourceInfo = Callable[[Path], tuple[str | None, str | None]]


def read_json(path: Path) -> dict[str, Any] | None:
    """A JSON object from ``path``, or None when missing, partial or not an object."""
    try:
        data: Any = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return None
    return data if isinstance(data, dict) else None


def parse_dt(value: Any) -> datetime | None:
    """ISO timestamp to an aware UTC datetime. Naive values are local time (harbor's job clock)."""
    if not isinstance(value, str) or not value:
        return None
    try:
        dt = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError:
        return None
    if dt.tzinfo is None:
        dt = dt.astimezone()  # interpret as local time
    return dt.astimezone(UTC)


def iso(dt: datetime | None) -> str | None:
    return dt.astimezone(UTC).isoformat(timespec="seconds") if dt else None


def _mtime(path: Path) -> datetime | None:
    try:
        return datetime.fromtimestamp(path.stat().st_mtime, UTC)
    except OSError:
        return None


def _birth(path: Path) -> datetime | None:
    try:
        st = path.stat()
    except OSError:
        return None
    born = getattr(st, "st_birthtime", None)
    return datetime.fromtimestamp(float(born) if born else st.st_mtime, UTC)


_source_cache: dict[str, tuple[float, tuple[str | None, str | None]]] = {}


def git_source_info(worktree: Path) -> tuple[str | None, str | None]:
    """(exact bench tag or None, short commit) for a checkout, cached briefly."""
    key = str(worktree)
    hit = _source_cache.get(key)
    now = time.monotonic()
    if hit and now - hit[0] < SOURCE_CACHE_TTL_S:
        return hit[1]

    def git(*args: str) -> str | None:
        try:
            proc = subprocess.run(
                ["git", "-C", str(worktree), *args],
                capture_output=True,
                text=True,
                timeout=5,
                check=False,
            )
        except (OSError, subprocess.TimeoutExpired):
            return None
        return proc.stdout.strip() if proc.returncode == 0 else None

    commit = git("rev-parse", "--short", "HEAD")
    tag = git("describe", "--tags", "--exact-match", "HEAD") if commit else None
    info = (tag, commit)
    _source_cache[key] = (now, info)
    return info


def find_worktree(path: Path) -> Path | None:
    """The git checkout containing ``path`` (a ``.git`` dir or file), if any."""
    for parent in (path, *path.parents):
        if (parent / ".git").exists():
            return parent
    return None


def _trial_id(dirname: str) -> str:
    return dirname.rsplit("__", 1)[1] if "__" in dirname else dirname


def _containers_for(project: str, containers: list[Container]) -> list[ContainerView]:
    out: list[ContainerView] = []
    for c in containers:
        if c.compose_project == project or c.name.startswith(project + "-"):
            out.append(ContainerView(name=c.name, status=c.status, state=c.state, image=c.image))
    return out


def _existing(base: Path, names: tuple[str, ...]) -> list[str]:
    return [n for n in names if (base / n).is_file()]


class Discovery:
    """Scans the configured roots. Process and container listings are injectable for tests."""

    def __init__(
        self,
        cfg: RunnerConfig,
        *,
        procs: list[Proc] | None = None,
        containers: list[Container] | None = None,
        now: datetime | None = None,
        source_info: SourceInfo = git_source_info,
        jobs: list[Job] | None = None,
    ) -> None:
        self.cfg = cfg
        self.procs = procs if procs is not None else list_processes()
        self.containers = containers if containers is not None else list_containers()
        self.now = now or datetime.now(UTC)
        self.source_info = source_info
        # The queue's records, newest last so the latest job for a name wins.
        self.jobs = list(jobs or [])
        self.jobs_by_name: dict[str, Job] = {}
        for j in sorted(self.jobs, key=lambda j: j.id):
            self.jobs_by_name[j.name] = j
        # Suite levels the GUI launched, by output directory: a run dir under one
        # of these belongs to that job.
        self.suite_jobs: list[tuple[Path, Job]] = [
            (Path(str(j.spec["out_dir"])), j)
            for j in sorted(self.jobs, key=lambda j: j.id)
            if j.kind == "suite" and j.spec.get("out_dir")
        ]

    def _owning_job(self, run_dir: Path) -> Job | None:
        for out_dir, job in reversed(self.suite_jobs):
            if _safe_child(out_dir, run_dir) and run_dir != out_dir:
                return job
        return None

    # ----------------------------------------------------------------- gate --

    def gate_runs(self) -> list[RunView]:
        jobs_dir = self.cfg.gate_jobs_dir
        if not jobs_dir.is_dir():
            return []
        out: list[RunView] = []
        for job_dir in sorted(jobs_dir.iterdir()):
            if not job_dir.is_dir():
                continue
            view = self.gate_job(job_dir)
            if view is not None:
                out.append(view)
        return out

    def gate_job(self, job_dir: Path) -> RunView | None:  # noqa: PLR0912, PLR0915
        config = read_json(job_dir / "config.json")
        if config is None:
            return None
        job_name = str(config.get("job_name") or job_dir.name)
        agents = config.get("agents") or []
        agent = agents[0] if isinstance(agents, list) and agents else {}
        harness = agent.get("name") if isinstance(agent, dict) else None
        model = agent.get("model_name") if isinstance(agent, dict) else None
        kwargs = agent.get("kwargs") if isinstance(agent, dict) else None
        effort = kwargs.get("reasoning_effort") if isinstance(kwargs, dict) else None
        tasks = config.get("tasks") or []
        task_path: Path | None = None
        if isinstance(tasks, list) and tasks and isinstance(tasks[0], dict):
            raw = tasks[0].get("path")
            task_path = Path(str(raw)) if raw else None
        task = task_path.name if task_path else None
        attempts_total = config.get("n_attempts")
        result = read_json(job_dir / "result.json")
        status_file = read_json(self.cfg.gate_out / f"{job_name}.status.json")
        proc = find_harbor_job(job_name, self.procs)
        alive = proc is not None

        trials = [
            self.gate_trial(d, harness=harness, model=model, job_alive=alive)
            for d in sorted(job_dir.iterdir())
            if d.is_dir() and "__" in d.name
        ]
        solves = sum(1 for t in trials if t.reward and t.reward.get("reward") == 1)
        if solves >= 2:
            for t in trials:
                if t.status == "error" and t.reward is None:
                    t.status = "cancelled"
        attempts_done = sum(1 for t in trials if t.status in {"finished", "error", "cancelled"})

        started = parse_dt(result.get("started_at")) if result else None
        if started is None:
            started = _mtime(job_dir / "config.json")
        finished = parse_dt(result.get("finished_at")) if result else None
        notes: list[str] = []
        status = self._gate_status(job_dir, status_file, alive, finished, trials, notes)
        if status not in {"external_running", "running"} and finished is None:
            # Harbor never stamps finished_at on a cancelled job; use the last trial end.
            ends = [parse_dt(t.finished_at) for t in trials if t.finished_at]
            finished = max((e for e in ends if e), default=None)
        if status in {"external_running", "running"}:
            finished = None
        elapsed = None
        if started is not None:
            elapsed = ((finished or self.now) - started).total_seconds()

        source_tag = source_commit = None
        source_dir = None
        worktree = find_worktree(task_path) if task_path else None
        if worktree is not None:
            source_dir = str(worktree)
            source_tag, source_commit = self.source_info(worktree)
        if status_file and status_file.get("repo") and not source_commit:
            source_commit = str(status_file["repo"])

        stats = result.get("stats") if result else None
        cost = stats.get("cost_usd") if isinstance(stats, dict) else None
        logs = _existing(job_dir, GATE_JOB_LOGS)
        for name in (f"{job_name}.log", f"{job_name}.console.log", f"{job_name}.status.json"):
            if (self.cfg.gate_out / name).is_file():
                logs.append(f"../../{name}")

        running_trial = next((t for t in trials if t.status == "running"), None)
        first_check = running_trial.first_check if running_trial else None
        if first_check is None and trials:
            first_check = trials[-1].first_check

        job = self.jobs_by_name.get(job_name)
        if job is not None:
            status = _owned_status(status, job, alive)
            notes.extend(n for n in job.notes if n not in notes)
            if job.console_log and Path(job.console_log).is_file():
                logs.append(CONSOLE_LOG)
            if not source_tag and job.spec.get("tag"):
                source_tag = str(job.spec["tag"])
            if status in {"running", "stopping", "preparing"}:
                finished = None
                if started is not None:
                    elapsed = (self.now - started).total_seconds()

        return RunView(
            id=GATE_PREFIX + job_name,
            kind="gate",
            name=job_name,
            status=status,
            owned=job is not None,
            suite_or_task=task,
            model=str(model) if model else None,
            harness=str(harness) if harness else None,
            effort=str(effort) if effort else None,
            source_tag=source_tag,
            source_commit=source_commit,
            source_dir=source_dir,
            attempts_done=attempts_done,
            attempts_total=int(attempts_total) if isinstance(attempts_total, int) else None,
            solves=solves,
            started_at=iso(started),
            finished_at=iso(finished),
            elapsed_s=elapsed,
            cost_usd=float(cost) if isinstance(cost, (int, float)) else None,
            pid=proc.pid if proc else None,
            path=str(job_dir),
            trials=trials,
            log_sources=logs,
            first_check=first_check,
            notes=notes,
            job=job_summary(job) if job is not None else None,
        )

    def owned_without_dir(self) -> list[RunView]:
        """Queue entries shown as their own row: gate jobs with no job directory yet,
        every suite level (its runs are rows of their own) and worktree preparations."""
        out: list[RunView] = []
        for job in self.jobs:
            if job.kind == "gate":
                job_dir = Path(str(job.spec.get("job_dir") or self.cfg.gate_jobs_dir / job.name))
                if job_dir.is_dir():
                    continue
            out.append(self.job_view(job))
        return out

    def job_view(self, job: Job) -> RunView:
        spec = job.spec
        started = parse_dt(job.started_at) or parse_dt(job.created_at)
        finished = parse_dt(job.finished_at)
        elapsed = None
        if started and job.status in {"preparing", "running", "stopping"}:
            elapsed = (self.now - started).total_seconds()
        elif started and finished:
            elapsed = (finished - started).total_seconds()
        logs = [CONSOLE_LOG] if job.console_log and Path(job.console_log).is_file() else []
        attempts_done = 0
        attempts_total = int(spec["attempts"]) if isinstance(spec.get("attempts"), int) else None
        solves: int | None = None
        cost: float | None = None
        path: str | None = None
        if job.kind == "suite":
            out_dir = Path(str(spec.get("out_dir") or ""))
            path = str(out_dir)
            n_tasks = int(spec.get("n_tasks") or len(spec.get("task_ids") or []))
            attempts_total = n_tasks * int(spec.get("repeat") or 1) or None
            attempts_done, solves, cost = _count_results(out_dir)
        return RunView(
            id=JOB_PREFIX + str(job.id),
            kind=str(spec.get("kind") or job.kind),
            name=job.name,
            status=job.status,
            owned=True,
            suite_or_task=str(spec.get("task") or spec.get("suite") or "") or None,
            model=str(spec["model"]) if spec.get("model") else None,
            harness=str(spec.get("agent") or spec.get("harness") or "") or None,
            effort=str(spec["effort"]) if spec.get("effort") else None,
            source_tag=str(spec["tag"]) if spec.get("tag") else None,
            source_commit=str(spec["tag_commit"]) if spec.get("tag_commit") else None,
            source_dir=str(spec["worktree"]) if spec.get("worktree") else None,
            attempts_done=attempts_done,
            attempts_total=attempts_total,
            solves=solves,
            started_at=iso(started) if job.started_at else None,
            finished_at=iso(finished),
            elapsed_s=elapsed,
            cost_usd=cost,
            pid=job.pid,
            path=path,
            trials=[],
            log_sources=logs,
            notes=list(job.notes),
            job=job_summary(job),
        )

    def _gate_status(  # noqa: PLR0911
        self,
        job_dir: Path,
        status_file: dict[str, Any] | None,
        alive: bool,
        finished: datetime | None,
        trials: list[TrialView],
        notes: list[str],
    ) -> str:
        if alive:
            return "external_running"
        phase = str(status_file.get("phase")) if status_file else None
        reason = str(status_file.get("stop_reason") or "") if status_file else ""
        if phase == "done":
            if reason == "early-reject":
                notes.append("early exit after two solves")
            return "finished"
        if phase == "infra_failed":
            notes.append(f"stopped by run_gate.sh: {reason or 'infra'}")
            return "infra_failed"
        if phase == "stopped":
            notes.append(f"stopped: {reason}".rstrip(": "))
            return "stopped"
        if phase in {"preflight", "running", "stopping"}:
            notes.append(f"run_gate.sh reported phase {phase} but no harbor process is alive")
            return "lost"
        name = job_dir.name
        if name.startswith("INFRA-"):
            return "infra_failed"
        if name.startswith("STOPPED-"):
            return "stopped"
        if finished is not None:
            return "finished"
        if not trials:
            return "unknown"
        if any(t.status == "interrupted" for t in trials):
            notes.append("a trial has no result and no harbor process is alive")
            return "stopped"
        notes.append("harbor never stamped finished_at (cancelled or killed)")
        return "stopped"

    def gate_trial(
        self, trial_dir: Path, *, harness: Any, model: Any, job_alive: bool
    ) -> TrialView:
        res = read_json(trial_dir / "result.json")
        reward = read_json(trial_dir / "verifier" / "reward.json")
        if reward is None and res is not None:
            vr = res.get("verifier_result")
            if isinstance(vr, dict) and isinstance(vr.get("rewards"), dict):
                reward = dict(vr["rewards"])
        exception: str | None = None
        if res is not None:
            info = res.get("exception_info")
            if isinstance(info, dict):
                exception = str(info.get("exception_type") or info.get("type") or "error")
                msg = info.get("exception_message") or info.get("message")
                if isinstance(msg, str) and msg:
                    exception += ": " + msg.splitlines()[0][:200]
            status = "error" if info else "finished"
        else:
            status = "running" if job_alive else "interrupted"
        started = parse_dt(res.get("started_at")) if res else _birth(trial_dir / "config.json")
        finished = parse_dt(res.get("finished_at")) if res else None
        agent_started: datetime | None = None
        if res is not None and isinstance(res.get("agent_execution"), dict):
            agent_started = parse_dt(res["agent_execution"].get("started_at"))
        if agent_started is None:
            for name in ("claude-code.txt", "codex.txt"):
                stream = trial_dir / "agent" / name
                if stream.exists():
                    agent_started = _birth(stream)
                    break
        elapsed = None
        if started is not None:
            end = finished if finished else (self.now if status == "running" else None)
            if end is None:
                end = _mtime(trial_dir) or self.now
            elapsed = (end - started).total_seconds()
        agent_result = res.get("agent_result") if res else None
        cost = agent_result.get("cost_usd") if isinstance(agent_result, dict) else None
        containers = _containers_for(trial_dir.name.lower() + "__env", self.containers)
        check = firstcheck.check_agent_dir(
            trial_dir,
            harness=str(harness) if harness else None,
            requested_model=str(model) if model else None,
            agent_started_at=agent_started,
            now=self.now,
            window_s=self.cfg.first_check_after_s,
            trial_finished=status != "running",
        )
        return TrialView(
            id=_trial_id(trial_dir.name),
            name=trial_dir.name,
            status=status,
            reward=reward,
            started_at=iso(started),
            finished_at=iso(finished),
            elapsed_s=elapsed,
            cost_usd=float(cost) if isinstance(cost, (int, float)) else None,
            containers=containers,
            first_check=check,
            log_sources=_existing(trial_dir, GATE_TRIAL_LOGS),
            exception=exception,
        )

    # -------------------------------------------------------------- harness --

    def harness_runs(self) -> list[RunView]:
        out: list[RunView] = []
        live = find_vulcanbench_runs(self.procs)
        for idx, root in enumerate(self.cfg.run_roots):
            if not root.is_dir():
                continue
            for run_dir in self._walk_run_dirs(root):
                view = self.harness_run(root, idx, run_dir, live)
                if view is not None:
                    out.append(view)
        return out

    @staticmethod
    def _walk_run_dirs(root: Path, max_depth: int = 3) -> list[Path]:
        found: list[Path] = []
        base_depth = len(root.parts)
        for dirpath, dirnames, filenames in os.walk(root):
            depth = len(Path(dirpath).parts) - base_depth
            if "summary.json" in filenames or "trace.jsonl" in filenames:
                found.append(Path(dirpath))
                dirnames[:] = []  # a run dir has no nested runs
                continue
            if depth >= max_depth:
                dirnames[:] = []
            dirnames[:] = [d for d in dirnames if d not in {"workspace", "node_modules", ".git"}]
        return sorted(found)

    def harness_run(  # noqa: PLR0912, PLR0915
        self, root: Path, root_idx: int, run_dir: Path, live: list[Proc]
    ) -> RunView | None:
        summary = read_json(run_dir / "summary.json")
        trace_path = run_dir / "trace.jsonl"
        first: dict[str, Any] | None = None
        last_ts: datetime | None = None
        if summary is None:
            first, last_ts = _trace_bounds(trace_path)
            if first is None:
                return None
        meta = summary or first or {}
        model_spec = str(meta.get("model") or "")
        harness, _, bare_model = model_spec.rpartition(":")
        if summary and isinstance(summary.get("cli_agent"), dict):
            harness = str(summary["cli_agent"].get("harness") or harness)
            bare_model = str(summary["cli_agent"].get("requested_model") or bare_model)
        effort_obj = summary.get("effort") if summary else None
        effort = effort_obj.get("requested") if isinstance(effort_obj, dict) else None
        rel = run_dir.relative_to(root).as_posix()
        run_id = str(meta.get("run_id") or run_dir.name)
        task_id = meta.get("task_id")

        started = (
            parse_dt(summary.get("started_at")) if summary else parse_dt(first and first.get("ts"))
        )
        if started is None:
            started = _birth(trace_path) or _mtime(run_dir)
        finished = parse_dt(summary.get("finished_at")) if summary else None
        notes: list[str] = []
        reward: dict[str, Any] | None = None
        if summary is not None:
            status = "finished"
            scores = summary.get("scores")
            if isinstance(scores, dict):
                reward = {
                    "functional": scores.get("functional"),
                    "total": scores.get("total"),
                    "reward": 1 if scores.get("functional") == 1.0 else 0,
                }
            if summary.get("finished") is False:
                status = "failed"
                notes.append("summary says the run did not finish")
        else:
            root_token = str(root)
            alive = any(root_token in p.command for p in live)
            fresh = last_ts is not None and (self.now - last_ts).total_seconds() < HARNESS_STALE_S
            if alive or fresh:
                status = "external_running" if alive else "running"
                if not alive:
                    notes.append("trace is still growing but no vulcanbench process was found")
            else:
                status = "stopped"
                notes.append("trace stopped growing with no summary.json")
        owner = self._owning_job(run_dir)
        if owner is not None and summary is None:
            # The queue knows whether the level is still running; the process table does not need to.
            if owner.status in {"running", "stopping"}:
                status = "running" if owner.status == "running" else "stopping"
                notes = [n for n in notes if "no vulcanbench process" not in n]
            elif status in {"running", "external_running"}:
                status = "stopped"
                notes.append(f"the level's job is {owner.status}")
        elapsed = None
        if started is not None:
            end = finished or (self.now if status in {"running", "external_running"} else last_ts)
            if end is not None:
                elapsed = (end - started).total_seconds()
        source = summary.get("source") if summary else None
        tag = commit = None
        if isinstance(source, dict):
            tag = _first_str(source, ("tag", "bench_tag", "describe", "git_describe"))
            commit = _first_str(source, ("commit", "sha", "git_commit", "head"))
            if tag and not tag.startswith("bench/"):
                tag = tag if "bench/" in tag else None
        suite = summary.get("suite") if summary else None
        check = firstcheck.check_harness_run(
            run_dir,
            harness=harness or None,
            requested_model=bare_model or None,
            agent_started_at=started,
            now=self.now,
            window_s=self.cfg.first_check_after_s,
            run_finished=status not in {"running", "external_running"},
        )
        trial = TrialView(
            id=run_id,
            name=run_id,
            status="running" if status in {"running", "external_running"} else status,
            reward=reward,
            started_at=iso(started),
            finished_at=iso(finished),
            elapsed_s=elapsed,
            cost_usd=_num(summary.get("cost_usd")) if summary else None,
            containers=self._harness_containers(run_id, run_dir),
            first_check=check,
            log_sources=_existing(run_dir, HARNESS_RUN_LOGS),
        )
        return RunView(
            id=f"{HARNESS_PREFIX}{root_idx}:{rel}",
            kind="suite" if suite else "task",
            name=run_id,
            status=status,
            owned=owner is not None,
            job=job_summary(owner) if owner is not None else None,
            suite_or_task=str(suite or task_id or ""),
            model=bare_model or model_spec or None,
            harness=harness or "vulcan",
            effort=str(effort) if effort else None,
            source_tag=tag,
            source_commit=commit,
            attempts_done=1 if status not in {"running", "external_running"} else 0,
            attempts_total=1,
            solves=reward.get("reward") if reward else None,
            started_at=iso(started),
            finished_at=iso(finished),
            elapsed_s=elapsed,
            cost_usd=trial.cost_usd,
            path=str(run_dir),
            trials=[trial],
            log_sources=list(trial.log_sources),
            first_check=check,
            infra_retries=None,
            notes=notes,
        )

    def _harness_containers(self, run_id: str, run_dir: Path) -> list[ContainerView]:
        project = "vb-" + run_id.lower().replace("_", "-")[-40:]
        needles = (str(run_dir), f"vulcanbench-{run_id}-")
        out: list[ContainerView] = []
        for c in self.containers:
            if c.compose_project == project or any(n in c.mounts for n in needles):
                out.append(
                    ContainerView(name=c.name, status=c.status, state=c.state, image=c.image)
                )
        return out

    # ------------------------------------------------------------------ all --

    def all_runs(self) -> list[RunView]:
        runs = self.gate_runs() + self.harness_runs() + self.owned_without_dir()
        active = {"external_running", "running", "preparing", "stopping", "queued", "waiting"}
        runs.sort(
            key=lambda r: (0 if r.status in active else 1, r.started_at or ""),
        )
        # Active first (oldest active first), then the rest newest first.
        head = [r for r in runs if r.status in active]
        tail = sorted(
            (r for r in runs if r.status not in active),
            key=lambda r: r.started_at or "",
            reverse=True,
        )
        return head + tail

    def get(self, run_id: str) -> RunView | None:  # noqa: PLR0911
        if run_id.startswith(JOB_PREFIX):
            rest = run_id[len(JOB_PREFIX) :]
            if not rest.isdigit():
                return None
            job = next((j for j in self.jobs if j.id == int(rest)), None)
            if job is None:
                return None
            if job.kind != "gate":
                return self.job_view(job)
            job_dir = Path(str(job.spec.get("job_dir") or self.cfg.gate_jobs_dir / job.name))
            if job_dir.is_dir() and _safe_child(self.cfg.gate_jobs_dir, job_dir):
                return self.gate_job(job_dir)
            return self.job_view(job)
        if run_id.startswith(GATE_PREFIX):
            job_name = run_id[len(GATE_PREFIX) :]
            job_dir = self.cfg.gate_jobs_dir / job_name
            if not _safe_child(self.cfg.gate_jobs_dir, job_dir) or not job_dir.is_dir():
                return None
            return self.gate_job(job_dir)
        if run_id.startswith(HARNESS_PREFIX):
            rest = run_id[len(HARNESS_PREFIX) :]
            idx_s, _, rel = rest.partition(":")
            if not idx_s.isdigit():
                return None
            idx = int(idx_s)
            if idx >= len(self.cfg.run_roots):
                return None
            root = self.cfg.run_roots[idx]
            run_dir = root / rel
            if not _safe_child(root, run_dir) or not run_dir.is_dir():
                return None
            return self.harness_run(root, idx, run_dir, find_vulcanbench_runs(self.procs))
        return None

    # ------------------------------------------------------------------ logs --

    def resolve_log(  # noqa: PLR0911
        self, run: RunView, source: str, trial: str | None
    ) -> Path | None:
        """Path for a log the page may show, or None if it is not on the allowlist."""
        if source == CONSOLE_LOG and trial is None and run.job:
            console = run.job.get("console_log")
            target = Path(str(console)) if console else None
            if target is None or not _safe_child(self.cfg.home / "jobs", target):
                return None
            return target if target.is_file() else None
        if run.path is None:
            return None
        base = Path(run.path)
        if run.kind == "gate":
            if trial:
                t = next((t for t in run.trials if trial in (t.id, t.name)), None)
                if t is None or source not in GATE_TRIAL_LOGS:
                    return None
                target = base / t.name / source
                return target if _safe_child(base, target) and target.is_file() else None
            if source in GATE_JOB_LOGS:
                target = base / source
            elif source.startswith("../../"):
                name = source[len("../../") :]
                if "/" in name or not name.startswith(run.name + "."):
                    return None
                target = self.cfg.gate_out / name
            else:
                return None
            return target if target.is_file() else None
        if source not in HARNESS_RUN_LOGS:
            return None
        target = base / source
        return target if _safe_child(base, target) and target.is_file() else None


def _owned_status(disk_status: str, job: Job, harbor_alive: bool) -> str:
    """What the Runs page shows for a job the queue owns."""
    if job.status in {"queued", "preparing", "stopping"}:
        return job.status
    if job.status == "running":
        return "running"
    if disk_status in {"external_running"} and harbor_alive:
        # The queue says it ended but harbor is still there: show it, the supervisor sweeps.
        return "stopping" if job.status == "stopped" else "external_running"
    if disk_status in {"lost", "unknown", "stopped"} and job.status in {
        "failed",
        "infra_failed",
        "lost",
        "stopped",
    }:
        return job.status
    return disk_status


def _count_results(out_dir: Path) -> tuple[int, int, float | None]:
    """(runs with a summary, runs that passed, summed cost) directly under a level's output dir."""
    if not out_dir.is_dir():
        return 0, 0, None
    done = passed = 0
    cost = 0.0
    known = False
    for d in out_dir.iterdir():
        if not d.is_dir():
            continue
        summary = read_json(d / "summary.json")
        if summary is None:
            continue
        done += 1
        scores = summary.get("scores")
        if isinstance(scores, dict) and scores.get("functional") == 1.0:
            passed += 1
        c = summary.get("cost_usd")
        if isinstance(c, (int, float)):
            cost += float(c)
            known = True
    return done, passed, (round(cost, 4) if known else None)


def job_summary(job: Job) -> dict[str, Any]:
    d = job.to_dict()
    d.pop("spec", None)
    spec = job.spec
    d["spec"] = {
        k: spec.get(k)
        for k in (
            "argv",
            "cwd",
            "env",
            "path_prepend",
            "prep_argv",
            "needs_prep",
            "tag",
            "worktree",
            "estimate",
            "warnings",
            "sweep_id",
            "sweep_name",
            "model",
            "harness",
            "suite",
            "suite_name",
            "effort",
            "out_dir",
            "resume_at",
            "quota_continue",
        )
    }
    return d


def _safe_child(base: Path, target: Path) -> bool:
    try:
        return target.resolve().is_relative_to(base.resolve())
    except OSError:
        return False


def _first_str(d: dict[str, Any], keys: tuple[str, ...]) -> str | None:
    for k in keys:
        v = d.get(k)
        if isinstance(v, str) and v:
            return v
    return None


def _num(v: Any) -> float | None:
    return float(v) if isinstance(v, (int, float)) and not isinstance(v, bool) else None


def _trace_bounds(path: Path) -> tuple[dict[str, Any] | None, datetime | None]:
    """First event object and the timestamp of the last complete event in a trace."""
    if not path.is_file():
        return None, None
    first: dict[str, Any] | None = None
    last_ts: datetime | None = None
    try:
        with path.open("r", encoding="utf-8", errors="replace") as fh:
            for raw in fh:
                line = raw.strip()
                if not line:
                    continue
                try:
                    obj: Any = json.loads(line)
                except json.JSONDecodeError:
                    continue
                if not isinstance(obj, dict):
                    continue
                if first is None:
                    first = obj
                ts = parse_dt(obj.get("ts"))
                if ts is not None:
                    last_ts = ts
    except OSError:
        return None, None
    return first, last_ts


def tail_text(path: Path, *, max_bytes: int = 64_000, max_lines: int = 400) -> tuple[str, bool]:
    """The last ``max_lines`` lines (within ``max_bytes``) of a file, and whether it was cut."""
    try:
        size = path.stat().st_size
        with path.open("rb") as fh:
            if size > max_bytes:
                fh.seek(size - max_bytes)
                data = fh.read()
                truncated = True
            else:
                data = fh.read()
                truncated = False
    except OSError:
        return "", False
    text = data.decode("utf-8", errors="replace")
    lines = text.splitlines()
    if truncated and lines:
        lines = lines[1:]  # drop the partial first line
    if len(lines) > max_lines:
        lines = lines[-max_lines:]
        truncated = True
    return "\n".join(lines), truncated
