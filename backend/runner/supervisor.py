"""The one process that starts and stops GUI-launched runs.

A ``Supervisor`` has a synchronous ``tick()`` the backend's lifespan runs on
a daemon thread every couple of seconds, guarded by an ``flock`` on
``<runner home>/supervisor.lock`` so a second backend (or ``--reload``) can
never double-run the queue. Tests drive ``tick()`` directly with a fake
launcher.

What a tick does, in order:

1. Reconcile every job recorded as preparing, running or stopping with the
   process table (after a restart the child is not ours any more, so identity
   is pid + start time + boot time + an argv token). A dead child gets its
   final status from its exit code and, for gate runs, the script's status
   file; a finished worktree preparation moves the job on to its real
   command.
2. Act on persisted stop requests: SIGINT to the script (its trap stops
   harbor and sweeps the sandboxes), SIGTERM then SIGKILL to the job's
   process group if it is still there after the grace period, then a
   container sweep scoped to the job's own compose projects.
3. Start the oldest queued job when nothing owned is active and no outside
   ``harbor run`` or ``vulcanbench run`` is visible (one task at a time,
   docs/DECISIONS.md), after re-validating its plan.

Nothing here retries a failed or stopped job. The child's environment is the
allowlist from ``launch.child_env``; its output goes to a console log the
parent opens, never through the app logger.
"""

from __future__ import annotations

import contextlib
import fcntl
import os
import shutil
import signal
import subprocess
import threading
import time
from collections.abc import Callable
from datetime import UTC, datetime
from pathlib import Path
from typing import IO, Any

from backend.runner import launch, procs
from backend.runner.config import REPO_ROOT, RunnerConfig
from backend.runner.discovery import read_json
from backend.runner.docker_ps import Container, list_containers
from backend.runner.state import Job, State, iso

STOP_GRACE_S = 150.0  # the script's own 90 s + 15 s escalation, with margin
KILL_GRACE_S = 15.0
LSTART_RETRIES = 10

Launcher = Callable[..., "subprocess.Popen[bytes]"]


class Supervisor:
    def __init__(
        self,
        cfg: RunnerConfig,
        state: State,
        *,
        launcher: Launcher = subprocess.Popen,
        list_procs: Callable[[], list[procs.Proc]] = procs.list_processes,
        containers: Callable[[], list[Container]] = list_containers,
        kill: Callable[[int, int], None] = os.kill,
        killpg: Callable[[int, int], None] = os.killpg,
        now: Callable[[], datetime] = lambda: datetime.now(UTC),
    ) -> None:
        self.cfg = cfg
        self.state = state
        self.launcher = launcher
        self.list_procs = list_procs
        self.containers = containers
        self.kill = kill
        self.killpg = killpg
        self.now = now
        self.children: dict[int, subprocess.Popen[bytes]] = {}
        self.logs: dict[int, IO[bytes]] = {}
        self.hold_reason: str | None = None
        self.last_tick: datetime | None = None
        self.last_error: str | None = None

    # ------------------------------------------------------------------ tick --

    def tick(self) -> None:
        try:
            self._reconcile()
            self._handle_stops()
            self._maybe_launch()
            self.last_error = None
        except Exception as exc:
            self.last_error = f"{type(exc).__name__}: {exc}"
        self.last_tick = self.now()

    def describe(self) -> dict[str, Any]:
        return {
            "last_tick": iso(self.last_tick) if self.last_tick else None,
            "last_error": self.last_error,
            "hold_reason": self.hold_reason,
            "children": sorted(self.children),
        }

    # ------------------------------------------------------------- liveness --

    def _procs(self) -> list[procs.Proc]:
        procs.reset_cache()
        return self.list_procs()

    def _child_alive(self, job: Job, table: list[procs.Proc]) -> bool:
        child = self.children.get(job.id)
        if child is not None:
            return child.poll() is None
        if job.pid is None:
            return False
        if job.boot_time and procs.boot_time() not in (None, job.boot_time):
            return False
        return procs.same_process(job.pid, job.proc_lstart, job.argv_token, table)

    def _exit_code(self, job: Job) -> int | None:
        child = self.children.get(job.id)
        return child.returncode if child is not None else None

    def _release(self, job_id: int) -> None:
        self.children.pop(job_id, None)
        log = self.logs.pop(job_id, None)
        if log is not None:
            with contextlib.suppress(OSError):
                log.close()

    # ------------------------------------------------------------ reconcile --

    def _reconcile(self) -> None:
        table = self._procs()
        for job in self.state.active_jobs():
            if self._child_alive(job, table):
                continue
            code = self._exit_code(job)
            self._release(job.id)
            if job.phase == "prepare":
                self._after_prepare(job, code)
            else:
                self._finalize(job, code)

    def _after_prepare(self, job: Job, code: int | None) -> None:
        if code == 0 or (
            code is None and launch.worktree_dir(self.cfg, str(job.spec["tag"])).is_dir()
        ):
            if job.stop_requested_at:
                self._final(
                    job, "stopped", code, "stop requested while the worktree was being prepared"
                )
                return
            self.state.add_note(job.id, "worktree prepared")
            self._start_main(self.state.update_job(job.id, phase=None, pid=None))
            return
        self._final(job, "failed", code, f"run_worktree.sh exited {code}; see the console log")

    def _finalize(self, job: Job, code: int | None) -> None:
        status, code, note = self._verdict(job, code)
        self._final(job, status, code, note)

    def _verdict(self, job: Job, code: int | None) -> tuple[str, int | None, str | None]:  # noqa: PLR0911
        status_file = launch_status_file(self.cfg, job)
        phase = str(status_file.get("phase")) if status_file else None
        reason = str(status_file.get("stop_reason") or "") if status_file else ""
        if code is None and status_file and isinstance(status_file.get("exit_code"), int):
            code = int(status_file["exit_code"])
        if phase == "done":
            return "finished", code, "early exit after two solves" if reason else None
        if phase == "infra_failed":
            return "infra_failed", code, f"run_gate.sh stopped the run: {reason}"
        if phase == "stopped":
            return "stopped", code, f"run_gate.sh stopped on {reason}"
        if code == 0:
            return "finished", code, None
        if code == 2:
            return "failed", code, "refused before harbor started; see the console log"
        if code in (130, 143) or (code is not None and code < 0):
            if job.stop_requested_at:
                return "stopped", code, "stopped on request"
            return "stopped", code, "stopped by a signal from outside the GUI"
        if code == 1:
            return "infra_failed", code, "run_gate.sh exited 1 (infrastructure failure)"
        if code is None:
            return (
                "lost",
                None,
                "process gone after a backend restart and no status file; final state unknown",
            )
        return "failed", code, f"exited {code}"

    def _final(self, job: Job, status: str, code: int | None, note: str | None) -> None:
        notes = [*job.notes, note] if note else job.notes
        self.state.update_job(
            job.id,
            status=status,
            exit_code=code,
            finished_at=iso(self.now()),
            notes=notes,
            phase=None,
        )
        if job.kind == "gate" and status != "finished":
            self._sweep(job)

    # ---------------------------------------------------------------- stops --

    def _handle_stops(self) -> None:
        now = self.now()
        table: list[procs.Proc] | None = None
        for job in self.state.active_jobs():
            if not job.stop_requested_at or job.pid is None:
                continue
            if table is None:
                table = self._procs()
            if not job.stop_signalled_at:
                self._signal(job.pid, signal.SIGINT, pg=False)
                if job.kind == "gate" and not job.spec.get("script_features", {}).get(
                    "status_file", True
                ):
                    # Old run_gate.sh: its backgrounded harbor ignores SIGINT, so hit harbor directly.
                    harbor = procs.find_harbor_job(job.name, table)
                    if harbor is not None:
                        self._signal(harbor.pid, signal.SIGINT, pg=False)
                self.state.update_job(job.id, status="stopping", stop_signalled_at=iso(now))
                continue
            signalled = _dt(job.stop_signalled_at)
            escalated = _dt(job.stop_escalated_at)
            if (
                escalated is None
                and signalled
                and (now - signalled).total_seconds() >= STOP_GRACE_S
            ):
                self._signal(job.pgid or job.pid, signal.SIGTERM, pg=True)
                self.state.update_job(job.id, stop_escalated_at=iso(now))
                self.state.add_note(
                    job.id, "still alive after the grace period; SIGTERM sent to the process group"
                )
            elif escalated and (now - escalated).total_seconds() >= KILL_GRACE_S:
                self._signal(job.pgid or job.pid, signal.SIGKILL, pg=True)
                self.state.add_note(job.id, "SIGKILL sent to the process group")

    def _signal(self, pid: int, sig: int, *, pg: bool) -> None:
        try:
            if pg:
                self.killpg(pid, sig)
            else:
                self.kill(pid, sig)
        except ProcessLookupError:
            pass
        except PermissionError:
            pass

    def _sweep(self, job: Job) -> None:
        """Remove containers of this job's own compose projects only; never by image or broad prefix."""
        job_dir = Path(str(job.spec.get("job_dir") or ""))
        if not job_dir.is_dir():
            return
        projects = {
            d.name.lower() + "__env" for d in job_dir.iterdir() if d.is_dir() and "__" in d.name
        }
        if not projects:
            return
        docker = shutil.which("docker") or str(Path("~/.docker/bin/docker").expanduser())
        ids = [c.id for c in self.containers() if c.compose_project in projects]
        if ids:
            try:
                subprocess.run(
                    [docker, "rm", "-f", *ids], capture_output=True, timeout=60, check=False
                )
                self.state.add_note(job.id, f"removed {len(ids)} leftover sandbox container(s)")
            except (OSError, subprocess.TimeoutExpired) as exc:
                self.state.add_note(job.id, f"container sweep failed: {exc}")
        for project in sorted(projects):
            try:
                ls = subprocess.run(
                    [
                        docker,
                        "network",
                        "ls",
                        "-q",
                        "--filter",
                        f"label=com.docker.compose.project={project}",
                    ],
                    capture_output=True,
                    text=True,
                    timeout=30,
                    check=False,
                )
                nets = ls.stdout.split()
                if nets:
                    subprocess.run(
                        [docker, "network", "rm", *nets],
                        capture_output=True,
                        timeout=30,
                        check=False,
                    )
            except (OSError, subprocess.TimeoutExpired):
                pass

    # --------------------------------------------------------------- launch --

    def _maybe_launch(self) -> None:
        if self.state.active_jobs():
            self.hold_reason = None
            return
        queued = self.state.queued_jobs()
        if not queued:
            self.hold_reason = None
            return
        table = self._procs()
        outside = [p for p in table if "harbor run" in p.command and not p.zombie]
        outside += procs.find_vulcanbench_runs(table)
        if outside:
            self.hold_reason = (
                f"waiting: a run launched outside the GUI is active (pid {outside[0].pid}); "
                "one task at a time"
            )
            return
        self.hold_reason = None
        job = queued[0]
        problems = (
            [] if job.spec.get("needs_prep") else launch.validate_before_launch(self.cfg, job.spec)
        )
        if (
            job.spec.get("needs_prep")
            and launch.worktree_dir(self.cfg, str(job.spec["tag"])).is_dir()
        ):
            # Prepared since the plan was built (by hand or by an earlier job): validate like any other.
            problems = launch.validate_before_launch(self.cfg, job.spec)
            if not problems:
                job = self.state.update_job(job.id, spec={**job.spec, "needs_prep": False})
        if problems:
            self._final(job, "failed", None, "refused at launch: " + "; ".join(problems))
            return
        if job.spec.get("needs_prep"):
            self._start_prepare(job)
        else:
            self._start_main(job)

    def _console_log(self, job: Job) -> Path:
        path = self.cfg.home / "jobs" / str(job.id) / "console.log"
        path.parent.mkdir(parents=True, exist_ok=True)
        return path

    def _start_prepare(self, job: Job) -> None:
        argv = [str(a) for a in job.spec["prep_argv"]]
        env = launch.child_env(dict(job.spec.get("prep_env") or {}))
        self._spawn(job, argv, str(REPO_ROOT), env, token=str(job.spec["tag"]), phase="prepare")

    def _start_main(self, job: Job) -> None:
        problems = launch.validate_before_launch(self.cfg, job.spec)
        if problems:
            self._final(job, "failed", None, "refused at launch: " + "; ".join(problems))
            return
        argv = [str(a) for a in job.spec["argv"]]
        env = launch.child_env(dict(job.spec.get("env") or {}))
        token = f"{Path(argv[1]).name} {argv[2]} {argv[3]}"
        self._spawn(job, argv, str(job.spec["cwd"]), env, token=token, phase="main")

    def _spawn(
        self, job: Job, argv: list[str], cwd: str, env: dict[str, str], *, token: str, phase: str
    ) -> None:
        log_path = self._console_log(job)
        log = open(log_path, "ab")  # noqa: SIM115 - held open for the child's lifetime
        stamp = iso(self.now())
        log.write(f"=== {stamp} {phase}: {' '.join(argv)}\n=== cwd {cwd}\n".encode())
        log.flush()
        try:
            child = self.launcher(
                argv,
                cwd=cwd,
                env=env,
                stdin=subprocess.DEVNULL,
                stdout=log,
                stderr=subprocess.STDOUT,
                start_new_session=True,
            )
        except OSError as exc:
            log.close()
            self._final(job, "failed", None, f"could not start {argv[0]}: {exc}")
            return
        self.children[job.id] = child
        self.logs[job.id] = log
        lstart = self._lstart(child.pid)
        self.state.update_job(
            job.id,
            status="preparing" if phase == "prepare" else "running",
            phase=phase,
            pid=child.pid,
            pgid=child.pid,
            proc_lstart=lstart,
            boot_time=procs.boot_time(),
            argv_token=token,
            started_at=job.started_at or stamp,
            console_log=str(log_path),
        )

    def _lstart(self, pid: int) -> str | None:
        for _ in range(LSTART_RETRIES):
            p = procs.find_pid(pid, self._procs())
            if p is not None:
                return p.lstart
            time.sleep(0.05)
        return None


def launch_status_file(cfg: RunnerConfig, job: Job) -> dict[str, Any] | None:
    path = job.spec.get("status_file")
    if path:
        return read_json(Path(str(path)))
    return read_json(cfg.gate_out / f"{job.name}.status.json")


def _dt(value: str | None) -> datetime | None:
    if not value:
        return None
    try:
        return datetime.fromisoformat(value)
    except ValueError:
        return None


# ------------------------------------------------------------- service --


class SupervisorService:
    """Owns the lock, the thread and the single Supervisor for this backend process."""

    def __init__(self) -> None:
        self.supervisor: Supervisor | None = None
        self.thread: threading.Thread | None = None
        self.stop_event = threading.Event()
        self.lock_file: IO[str] | None = None
        self.status = "disabled"
        self.detail: str | None = None

    def start(self, cfg: RunnerConfig, *, interval_s: float = 2.0) -> None:
        if self.thread is not None:
            return
        cfg.home.mkdir(parents=True, exist_ok=True)
        lock_path = cfg.home / "supervisor.lock"
        fh = open(lock_path, "a+", encoding="utf-8")  # noqa: SIM115 - held for the process lifetime
        try:
            fcntl.flock(fh.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
        except OSError:
            fh.seek(0)
            holder = fh.read().strip() or "unknown pid"
            fh.close()
            self.status = "inactive"
            self.detail = f"another backend holds {lock_path} ({holder})"
            return
        fh.seek(0)
        fh.truncate()
        fh.write(str(os.getpid()))
        fh.flush()
        self.lock_file = fh
        self.supervisor = Supervisor(cfg, State(cfg.home / "runner.db"))
        self.stop_event.clear()
        self.thread = threading.Thread(
            target=self._run, args=(interval_s,), name="vulcanbench-runner", daemon=True
        )
        self.thread.start()
        self.status = "active"
        self.detail = None

    def _run(self, interval_s: float) -> None:
        assert self.supervisor is not None
        while not self.stop_event.is_set():
            self.supervisor.tick()
            self.stop_event.wait(interval_s)

    def stop(self) -> None:
        """Stop ticking. Running jobs are left alone; the next backend adopts them."""
        self.stop_event.set()
        if self.thread is not None:
            self.thread.join(timeout=10)
            self.thread = None
        if self.lock_file is not None:
            try:
                fcntl.flock(self.lock_file.fileno(), fcntl.LOCK_UN)
                self.lock_file.close()
            except OSError:
                pass
            self.lock_file = None
        if self.status == "active":
            self.status = "stopped"

    def describe(self) -> dict[str, Any]:
        out: dict[str, Any] = {"status": self.status, "detail": self.detail}
        if self.supervisor is not None:
            out.update(self.supervisor.describe())
        return out


SERVICE = SupervisorService()
