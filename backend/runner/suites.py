"""Build your own eval suite: the backend half of the GUI's Suites pages.

Every change to a suite goes through ``vulcanbench suite ...`` from a run
worktree pinned to a bench tag (the *tooling worktree*), exactly as a
benchmark run does; this module only picks the worktree, runs the command
with ``--json`` and maps its exit code. Reads (the suite list, a suite's
tasks, a task's files) come straight from disk as JSON and text, so polling
never starts a Python interpreter.

The builder is password-gated in the engine (docs/DECISIONS.md, 2026-11-03):
locked commands exit 7 with ``builder_locked``, ``builder_not_configured`` or
``wrong_password``, which every route here turns into a 403 the pages read to
show the unlock screen. :func:`unlock` hands the password to
``vulcanbench suite unlock --password-stdin`` on the child's stdin; it is never
an argument, never in the environment, never logged, never stored.

On disk per task (all written by the engine): ``build/scaffold.json``,
``build/author.json``, ``build/validate.json``; ``build/dropped.json`` is the
one file the GUI writes. A task's stage is derived from those files and the
runner's author and validate jobs.
"""

from __future__ import annotations

import json
import re
import threading
import time
from collections import deque
from dataclasses import dataclass
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

from backend.runner import launch, sweep
from backend.runner.config import RunnerConfig
from backend.runner.discovery import read_json
from backend.runner.redact import redact
from backend.runner.state import Job
from harness.suite_lock import lock_digest

# ``harness.suite_cli.ID_RE`` and ``REPO_RE``: what ``vulcanbench suite init`` accepts.
SUITE_ID_RE = re.compile(r"^[a-z0-9][a-z0-9-]{2,63}$")
REPO_RE = re.compile(r"^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$")
TASK_ID_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$")
DATE_RE = re.compile(r"^\d{4}-\d{2}-\d{2}$")
#: A bench tag whose worktree holds this file can run the builder's commands.
BUILDER_MARKER = Path("harness") / "suite_builder" / "access.py"
BUILDER_KINDS = ("author", "validate")
MAX_LICENSE_NOTE_CHARS = 2000
#: Billing the authoring session takes (``suite task author --billing``).
AUTHOR_BILLING = ("auto", "subscription")
ACCEPTED_STATUSES = ("PASS", "WARN")
LOCKED_CODES = ("builder_locked", "builder_not_configured", "wrong_password")
#: Path segments the routes use under /suites; a suite cannot be named after one.
RESERVED_IDS = frozenset({"access", "unlock", "lock", "new"})

PRS_TIMEOUT_S = 120
PRS_CACHE_S = 600
SCAFFOLD_TIMEOUT_S = 600
FREEZE_TIMEOUT_S = 300
SHORT_TIMEOUT_S = 60

MAX_PASSWORD_CHARS = 1024
UNLOCK_FAILURES_ALLOWED = 5
UNLOCK_FAILURE_WINDOW_S = 600
UNLOCK_COOLDOWN_S = 60

TEST_FILE_CAP = 64 * 1024
TESTS_TOTAL_CAP = 512 * 1024
GOLD_CAP = 256 * 1024
ISSUE_CAP = 128 * 1024


class SuitesError(Exception):
    """A refusal for the routes: an HTTP status and a detail (text or ``{code, message}``)."""

    def __init__(self, status: int, detail: str | dict[str, Any]) -> None:
        super().__init__(detail if isinstance(detail, str) else detail.get("message", ""))
        self.status = status
        self.detail = detail


def _now() -> datetime:
    return datetime.now(UTC)


# ----------------------------------------------------- tooling worktree --


def _builder_capable(wt: Path) -> bool:
    return (wt / BUILDER_MARKER).is_file() and (wt / ".venv" / "bin" / "vulcanbench").is_file()


def tooling_worktree(cfg: RunnerConfig, tag: str | None = None) -> dict[str, Any]:
    """The run worktree the builder's commands run from.

    ``tag`` when given; otherwise the newest bench tag whose worktree exists,
    has the suite builder (``harness/suite_builder/access.py``) and passes
    the same checks as a benchmark launch (at the tag, clean, ``.venv``).
    """
    if tag is not None:
        info = launch.worktree_status(cfg, tag)
        problems = list(info["problems"])
        wt = Path(info["path"])
        if not info["exists"]:
            problems.append(f"{wt} does not exist; prepare it on the Launch page")
        elif not (wt / BUILDER_MARKER).is_file():
            problems.append(f"{tag} predates the suite builder; pick a newer bench tag")
        return _tooling(tag, info, problems)
    candidates = [t for t in launch.bench_tags() if _builder_capable(launch.worktree_dir(cfg, t))]
    first_problem: dict[str, Any] | None = None
    for t in candidates:
        info = launch.worktree_status(cfg, t)
        if not info["problems"]:
            return _tooling(t, info, [])
        if first_problem is None:
            first_problem = _tooling(t, info, list(info["problems"]))
    if first_problem is not None:
        return first_problem
    return {
        "tag": None,
        "worktree": None,
        "tag_commit": None,
        "ready": False,
        "problems": [
            "no prepared bench tag has the suite builder: cut a bench tag from main and "
            "prepare its worktree on the Launch page"
        ],
    }


def _tooling(tag: str, info: dict[str, Any], problems: list[str]) -> dict[str, Any]:
    return {
        "tag": tag,
        "worktree": info["path"],
        "tag_commit": info.get("tag_commit"),
        "ready": not problems,
        "problems": problems,
    }


def require_tooling(cfg: RunnerConfig, tag: str | None = None) -> dict[str, Any]:
    tooling = tooling_worktree(cfg, tag)
    if not tooling["ready"]:
        raise SuitesError(
            409,
            {
                "code": "tooling_not_ready",
                "message": "; ".join(tooling["problems"]),
                "tooling": tooling,
            },
        )
    return tooling


# ------------------------------------------------------------ CLI calls --


@dataclass(frozen=True)
class CliResult:
    code: int | None
    doc: dict[str, Any] | None
    stderr: str

    @property
    def error(self) -> dict[str, Any]:
        err = (self.doc or {}).get("error")
        return err if isinstance(err, dict) else {}


def run_suite_cli(
    tooling: dict[str, Any], args: list[str], *, timeout: float, input_text: str = ""
) -> CliResult:
    """``<worktree>/.venv/bin/vulcanbench suite <args> --json``; stdin is ``input_text``, then EOF.

    Stdin is always a closed pipe so ``gh`` or ``git`` can never stop on a
    prompt. Arguments never carry a secret.
    """
    code, out, err = sweep._run_cli(
        Path(str(tooling["worktree"])),
        ["suite", *args, "--json"],
        timeout=timeout,
        input_text=input_text,
    )
    doc = sweep._last_json(out)
    return CliResult(code, doc if isinstance(doc, dict) else None, err)


def _with_home(cfg: RunnerConfig, args: list[str]) -> list[str]:
    return [*args, "--suites-home", str(cfg.suites_home)]


def cli_error(res: CliResult, statuses: dict[int, int], *, what: str) -> SuitesError:
    """Turn a failed suite command into a refusal; exit 7 is always the builder lock (403)."""
    err = res.error
    last = (res.stderr.strip().splitlines() or [""])[-1]
    message = str(err.get("message") or last or f"{what} failed")
    if res.code is None:
        status = 504 if "timed out" in res.stderr else 502
        return SuitesError(status, {"code": "cli_unavailable", "message": redact(res.stderr)[:500]})
    status = 403 if res.code == 7 else statuses.get(res.code, 502)
    detail: dict[str, Any] = {
        "code": str(err.get("code") or f"exit_{res.code}"),
        "message": redact(message)[:1000],
        "exit_code": res.code,
    }
    for key in ("problems", "validation_status", "path"):
        if key in err:
            detail[key] = err[key]
    if detail["code"] in {"gh_auth", "gh_missing"}:
        detail["hint"] = "run: gh auth login (in a terminal on this Mac)"
    return SuitesError(status, detail)


# ---------------------------------------------------------------- access --

_unlock_lock = threading.Lock()
_unlock_failures: deque[float] = deque()


def access_status(cfg: RunnerConfig, tag: str | None = None) -> dict[str, Any]:
    """``suite access --json`` from the tooling worktree, with the worktree it came from."""
    tooling = tooling_worktree(cfg, tag)
    if not tooling["ready"]:
        return {"tooling": tooling, "access": None}
    res = run_suite_cli(tooling, ["access"], timeout=SHORT_TIMEOUT_S)
    if res.code != 0 or res.doc is None:
        raise cli_error(res, {}, what="suite access")
    return {"tooling": tooling, "access": _access_doc(res.doc)}


def _access_doc(doc: dict[str, Any]) -> dict[str, Any]:
    return {
        "configured": bool(doc.get("configured")),
        "unlocked": bool(doc.get("unlocked")),
        "unlocked_at": doc.get("unlocked_at"),
    }


def check_password_shape(password: Any) -> str:
    """A password the engine can read as one stdin line; the refusal never quotes it."""
    if not isinstance(password, str) or not password:
        raise SuitesError(422, {"code": "bad_request", "message": 'send {"password": "..."}'})
    if len(password) > MAX_PASSWORD_CHARS:
        raise SuitesError(422, {"code": "bad_request", "message": "that password is too long"})
    if any(c in password for c in "\r\n\x00"):
        raise SuitesError(
            422, {"code": "bad_request", "message": "the password cannot contain line breaks"}
        )
    return password


def _cooldown_left(now: float) -> float:
    while _unlock_failures and now - _unlock_failures[0] > UNLOCK_FAILURE_WINDOW_S:
        _unlock_failures.popleft()
    if len(_unlock_failures) < UNLOCK_FAILURES_ALLOWED:
        return 0.0
    return max(0.0, UNLOCK_COOLDOWN_S - (now - _unlock_failures[-1]))


def unlock(cfg: RunnerConfig, password: str, *, tag: str | None = None) -> dict[str, Any]:
    """Pipe ``password`` to ``vulcanbench suite unlock --password-stdin`` and report access.

    One unlock at a time; after five wrong passwords in ten minutes the next
    try waits a minute. Nothing the child prints reaches the browser before
    any copy of the password in it is masked.
    """
    tooling = require_tooling(cfg, tag)
    with _unlock_lock:
        wait = _cooldown_left(time.monotonic())
        if wait > 0:
            raise SuitesError(
                429,
                {
                    "code": "too_many_attempts",
                    "message": f"too many wrong passwords; try again in {int(wait) + 1} s",
                    "retry_after_s": int(wait) + 1,
                },
            )
        res = run_suite_cli(
            tooling,
            ["unlock", "--password-stdin"],
            timeout=SHORT_TIMEOUT_S,
            input_text=password + "\n",
        )
        res = CliResult(
            res.code,
            _mask(res.doc, password),
            res.stderr.replace(password, "[REDACTED]") if password in res.stderr else res.stderr,
        )
        if res.code == 0 and res.doc is not None:
            _unlock_failures.clear()
            return {"tooling": tooling, "access": _access_doc(res.doc)}
        if res.error.get("code") == "wrong_password":
            _unlock_failures.append(time.monotonic())
        raise cli_error(res, {2: 409}, what="suite unlock")


def _mask(doc: dict[str, Any] | None, secret: str) -> dict[str, Any] | None:
    if doc is None:
        return None
    text = json.dumps(doc)
    if secret not in text and json.dumps(secret)[1:-1] not in text:
        return doc
    masked = text.replace(json.dumps(secret)[1:-1], "[REDACTED]").replace(secret, "[REDACTED]")
    try:
        out = json.loads(masked)
    except json.JSONDecodeError:
        return {
            "error": {"code": "unreadable", "message": "the unlock command's output was withheld"}
        }
    return out if isinstance(out, dict) else None


def lock(cfg: RunnerConfig, *, tag: str | None = None) -> dict[str, Any]:
    tooling = require_tooling(cfg, tag)
    res = run_suite_cli(tooling, ["lock"], timeout=SHORT_TIMEOUT_S)
    if res.code != 0 or res.doc is None:
        raise cli_error(res, {}, what="suite lock")
    return {"tooling": tooling, "access": _access_doc(res.doc)}


def require_unlocked(tooling: dict[str, Any]) -> None:
    """For the one GUI-side write (drop) and for plans: the same check the engine makes."""
    res = run_suite_cli(tooling, ["access"], timeout=SHORT_TIMEOUT_S)
    if res.code != 0 or res.doc is None:
        raise cli_error(res, {}, what="suite access")
    doc = _access_doc(res.doc)
    if not doc["configured"]:
        raise SuitesError(
            403,
            {"code": "builder_not_configured", "message": "no suite builder password is set"},
        )
    if not doc["unlocked"]:
        raise SuitesError(
            403, {"code": "builder_locked", "message": "the suite builder is locked on this Mac"}
        )


# ------------------------------------------------------------ disk reads --


def check_suite_id(suite_id: str) -> str:
    if not SUITE_ID_RE.match(suite_id):
        raise SuitesError(404, f"suite {suite_id!r} not found")
    return suite_id


def check_task_id(task: str) -> str:
    if not TASK_ID_RE.match(task) or task.startswith("."):
        raise SuitesError(404, f"task {task!r} not found")
    return task


def _inside(base: Path, target: Path) -> bool:
    try:
        return target.resolve().is_relative_to(base.resolve())
    except OSError:
        return False


def suite_dir(cfg: RunnerConfig, suite_id: str) -> Path:
    d = cfg.suites_home / check_suite_id(suite_id)
    if not (d / "suite.json").is_file() or not _inside(cfg.suites_home, d):
        raise SuitesError(404, f"suite {suite_id!r} not found in {cfg.suites_home}")
    return d


def task_dir(cfg: RunnerConfig, suite_id: str, task: str) -> Path:
    base = suite_dir(cfg, suite_id)
    d = base / check_task_id(task)
    if not (d / "metadata.json").is_file() or not _inside(base, d):
        raise SuitesError(404, f"task {task!r} not found in suite {suite_id}")
    return d


def _manifest(d: Path) -> dict[str, Any]:
    return read_json(d / "suite.json") or {}


def _task_names(d: Path, manifest: dict[str, Any]) -> list[str]:
    listed = {str(t) for t in manifest.get("tasks") or []}
    on_disk = {
        p.name
        for p in d.iterdir()
        if p.is_dir() and TASK_ID_RE.match(p.name) and (p / "metadata.json").is_file()
    }
    return sorted(listed | on_disk)


def _jobs_for(jobs: list[Job], suite_id: str, task: str | None = None) -> list[Job]:
    return [
        j
        for j in jobs
        if j.kind in BUILDER_KINDS
        and j.spec.get("suite") == suite_id
        and (task is None or j.spec.get("task") == task)
    ]


def _when(value: Any) -> datetime | None:
    if not isinstance(value, str) or not value:
        return None
    try:
        dt = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError:
        return None
    return dt if dt.tzinfo else dt.replace(tzinfo=UTC)


def task_row(d: Path, task: str, manifest: dict[str, Any], jobs: list[Job]) -> dict[str, Any]:
    """One task as the Suites pages show it, with its stage derived from disk and jobs."""
    tdir = d / task
    build = tdir / "build"
    meta = read_json(tdir / "metadata.json") or {}
    scaffold = read_json(build / "scaffold.json")
    author = read_json(build / "author.json")
    validate = read_json(build / "validate.json")
    dropped = read_json(build / "dropped.json")
    own = _jobs_for(jobs, d.name, task)
    active = next((j for j in own if j.active), None)
    # A validation older than the latest authoring session says nothing about the task now.
    stale = False
    if validate is not None and author is not None:
        checked, authored = _when(validate.get("checked_at")), _when(author.get("finished_at"))
        if checked and authored and checked < authored:
            validate, stale = None, True
    if not (tdir / "metadata.json").is_file():
        stage = "missing"
    elif active is not None:
        stage = "authoring" if active.kind == "author" else "validating"
    elif validate is not None:
        stage = (
            "validated" if str(validate.get("status")) in ACCEPTED_STATUSES else "validation_failed"
        )
    elif author is not None:
        stage = "authored" if author.get("status") == "ok" else "author_failed"
    else:
        stage = "scaffolded"
    builder = meta.get("builder") if isinstance(meta.get("builder"), dict) else {}
    cost = (author or {}).get("cost") if isinstance((author or {}).get("cost"), dict) else {}
    return {
        "id": task,
        "pr": builder.get("pr") if builder else None,
        "title": _pr_title(build),
        "stage": stage,
        "accepted": task in [str(t) for t in manifest.get("tasks") or []],
        "accepted_unvalidated": task
        in [str(t) for t in manifest.get("admitted_unvalidated") or []],
        "dropped": dropped is not None,
        "dropped_reason": (dropped or {}).get("reason"),
        "scaffold": None
        if scaffold is None
        else {
            "base_commit": scaffold.get("base_commit"),
            "languages": scaffold.get("languages"),
            "image": scaffold.get("image"),
            "gold_applies": scaffold.get("gold_applies"),
            "warnings": scaffold.get("warnings") or [],
            "scaffolded_at": scaffold.get("scaffolded_at"),
        },
        "author": None
        if author is None
        else {
            "status": author.get("status"),
            "harness": author.get("harness"),
            "model": author.get("model"),
            "effort": author.get("effort"),
            "finished_at": author.get("finished_at"),
            "duration_s": author.get("duration_s"),
            "cost_usd": cost.get("cli_reported_usd") if cost else None,
            "warnings": author.get("warnings") or [],
            "accepted": len(((author.get("changed") or {}).get("accepted")) or []),
            "rejected": len(((author.get("changed") or {}).get("rejected")) or []),
        },
        "validate": None
        if validate is None
        else {
            "status": validate.get("status"),
            "reasons": validate.get("reasons") or [],
            "checks": validate.get("checks") or {},
            "sandbox": validate.get("sandbox"),
            "finished_at": validate.get("checked_at"),
        },
        "validate_stale": stale,
        "active_job": _job_ref(active) if active else None,
        "last_job_id": own[0].id if own else None,
    }


def _pr_title(build: Path) -> str | None:
    """The pull request's title from ``build/pr.json`` (what ``gh pr view`` returned)."""
    title = (read_json(build / "pr.json") or {}).get("title")
    return str(title)[:300] if title else None


def _job_ref(job: Job) -> dict[str, Any]:
    return {"id": job.id, "kind": job.kind, "status": job.status, "run_id": f"job:{job.id}"}


def _suite_row(cfg: RunnerConfig, d: Path, jobs: list[Job]) -> dict[str, Any]:
    manifest = _manifest(d)
    lock = read_json(d / "suite.lock.json")
    rows = [task_row(d, t, manifest, jobs) for t in _task_names(d, manifest)]
    stages: dict[str, int] = {}
    for r in rows:
        if not r["dropped"]:
            stages[r["stage"]] = stages.get(r["stage"], 0) + 1
    active = next((j for j in _jobs_for(jobs, d.name) if j.active), None)
    repo = manifest.get("repo") if isinstance(manifest.get("repo"), dict) else None
    return {
        "id": d.name,
        "name": str(manifest.get("display_name") or d.name),
        "repo": repo,
        "status": "frozen" if lock is not None else "draft",
        "path": str(d),
        "version": (lock or manifest).get("version"),
        "frozen_at": (lock or {}).get("frozen_at"),
        "lock_digest": _digest(lock),
        "n_accepted": sum(1 for r in rows if r["accepted"]),
        "n_tasks": sum(1 for r in rows if not r["dropped"]),
        "n_dropped": sum(1 for r in rows if r["dropped"]),
        "stages": stages,
        "active_job": _job_ref(active) if active else None,
        "updated_at": manifest.get("updated_at"),
    }


def _digest(lock: dict[str, Any] | None) -> str | None:
    return lock_digest(lock) if lock is not None else None


def list_suites(cfg: RunnerConfig, jobs: list[Job]) -> list[dict[str, Any]]:
    home = cfg.suites_home
    if not home.is_dir():
        return []
    return [
        _suite_row(cfg, d, jobs)
        for d in sorted(home.iterdir())
        if d.is_dir() and SUITE_ID_RE.match(d.name) and (d / "suite.json").is_file()
    ]


def freeze_ready(rows: list[dict[str, Any]], *, frozen: bool, active: bool) -> dict[str, Any]:
    problems: list[str] = []
    accepted = [r for r in rows if r["accepted"]]
    if frozen:
        problems.append("the suite is frozen")
    if not accepted:
        problems.append("accept at least one validated task")
    for r in accepted:
        if r["stage"] != "validated":
            problems.append(f"{r['id']} is accepted but its latest validation is not PASS or WARN")
        if r["dropped"]:
            problems.append(f"{r['id']} is accepted and dropped")
    if active:
        problems.append("an authoring or validation job is still active")
    return {"ok": not problems, "problems": problems}


def suite_detail(cfg: RunnerConfig, suite_id: str, jobs: list[Job]) -> dict[str, Any]:
    d = suite_dir(cfg, suite_id)
    manifest = _manifest(d)
    row = _suite_row(cfg, d, jobs)
    tasks = [task_row(d, t, manifest, jobs) for t in _task_names(d, manifest)]
    return {
        "suite": row,
        "tasks": tasks,
        "freeze_ready": freeze_ready(
            tasks, frozen=row["status"] == "frozen", active=row["active_job"] is not None
        ),
        "options": _build_options(),
    }


def _build_options() -> dict[str, list[str]]:
    """What the author form offers: CLI harnesses only, never a blocked effort."""
    return {
        "harnesses": list(sweep.CLI_HARNESSES),
        "efforts": sweep.efforts_offered(),
        "billing": list(AUTHOR_BILLING),
    }


def _read_text(path: Path, cap: int) -> dict[str, Any] | None:
    try:
        raw = path.read_bytes()
    except OSError:
        return None
    if b"\x00" in raw[:8192]:
        return {"text": None, "binary": True, "size": len(raw), "truncated": False}
    return {
        "text": redact(raw[:cap].decode("utf-8", errors="replace")),
        "binary": False,
        "size": len(raw),
        "truncated": len(raw) > cap,
    }


def task_detail(cfg: RunnerConfig, suite_id: str, task: str, jobs: list[Job]) -> dict[str, Any]:
    tdir = task_dir(cfg, suite_id, task)
    d = tdir.parent
    manifest = _manifest(d)
    meta = read_json(tdir / "metadata.json") or {}
    tests: list[dict[str, Any]] = []
    total = 0
    tests_root = tdir / "tests"
    if tests_root.is_dir():
        for f in sorted(tests_root.rglob("*")):
            if not f.is_file() or not _inside(tdir, f):
                continue
            rel = f.relative_to(tdir).as_posix()
            if total >= TESTS_TOTAL_CAP:
                tests.append({"path": rel, "text": None, "omitted": True})
                continue
            body = _read_text(f, min(TEST_FILE_CAP, TESTS_TOTAL_CAP - total)) or {}
            total += len(body.get("text") or "")
            tests.append({"path": rel, **body})
    own = _jobs_for(jobs, suite_id, task)
    return {
        "suite": {
            "id": suite_id,
            "name": manifest.get("display_name") or suite_id,
            "repo": manifest.get("repo") if isinstance(manifest.get("repo"), dict) else None,
        },
        "frozen": (d / "suite.lock.json").is_file(),
        "task": task_row(d, task, manifest, jobs),
        "issue_md": _read_text(tdir / "issue.md", ISSUE_CAP),
        "metadata": {
            k: meta.get(k)
            for k in (
                "tests",
                "image",
                "test_timeout_s",
                "setup",
                "upstream",
                "languages",
                "agent_hints",
                "category",
                "difficulty",
            )
            if k in meta
        },
        "tests": tests,
        "gold_patch": _read_text(tdir / "gold_patch.diff", GOLD_CAP),
        "author_notes": _read_text(tdir / "build" / "author_notes.md", ISSUE_CAP),
        "scaffold": read_json(tdir / "build" / "scaffold.json"),
        "author": read_json(tdir / "build" / "author.json"),
        "validate": read_json(tdir / "build" / "validate.json"),
        "jobs": [
            {
                "id": j.id,
                "kind": j.kind,
                "status": j.status,
                "created_at": j.created_at,
                "finished_at": j.finished_at,
                "notes": j.notes,
                "run_id": f"job:{j.id}",
            }
            for j in own
        ],
        "log_run_id": f"job:{own[0].id}" if own else None,
        "options": _build_options(),
    }


def scaffolded_prs(d: Path) -> dict[int, str]:
    out: dict[int, str] = {}
    for meta_path in d.glob("*/metadata.json"):
        meta = read_json(meta_path) or {}
        builder = meta.get("builder")
        pr = builder.get("pr") if isinstance(builder, dict) else None
        if isinstance(pr, int):
            out[pr] = meta_path.parent.name
    return out


# --------------------------------------------------------------- writes --

_suite_locks: dict[str, threading.Lock] = {}
_suite_locks_guard = threading.Lock()
_prs_cache: dict[tuple[str, str, int], tuple[float, dict[str, Any]]] = {}


def _suite_lock(suite_id: str) -> threading.Lock:
    with _suite_locks_guard:
        return _suite_locks.setdefault(suite_id, threading.Lock())


def _refuse_if_frozen(d: Path) -> None:
    if (d / "suite.lock.json").is_file():
        raise SuitesError(
            409, {"code": "suite_frozen", "message": f"{d.name} is frozen; it is read-only here"}
        )


def create_suite(
    cfg: RunnerConfig, *, suite_id: str, display_name: str, repo: str, tag: str | None = None
) -> dict[str, Any]:
    if not SUITE_ID_RE.match(suite_id) or suite_id in RESERVED_IDS:
        raise SuitesError(
            422,
            {
                "code": "invalid_id",
                "message": "id: lower-case letters, digits and hyphens, 3 to 64 characters",
            },
        )
    if not REPO_RE.match(repo):
        raise SuitesError(422, {"code": "invalid_repo", "message": "repository must be owner/name"})
    name = display_name.strip()
    if not name or len(name) > 120 or any(ord(c) < 32 for c in name):
        raise SuitesError(422, {"code": "invalid_name", "message": "name: 1 to 120 characters"})
    tooling = require_tooling(cfg, tag)
    res = run_suite_cli(
        tooling,
        _with_home(cfg, ["init", suite_id, "--display-name", name, "--repo", repo]),
        timeout=SHORT_TIMEOUT_S,
    )
    if res.code != 0:
        status = 409 if res.error.get("code") in {"id_collides", "exists"} else 422
        raise cli_error(res, {2: status, 8: 409}, what="suite init")
    return _suite_row(cfg, cfg.suites_home / suite_id, [])


def list_prs(
    cfg: RunnerConfig,
    suite_id: str,
    *,
    since: str | None,
    limit: int,
    refresh: bool,
    tag: str | None = None,
) -> dict[str, Any]:
    d = suite_dir(cfg, suite_id)
    if since is not None and not DATE_RE.match(since):
        raise SuitesError(422, {"code": "invalid_option", "message": "since must be YYYY-MM-DD"})
    key = (suite_id, since or "", limit)
    cached = _prs_cache.get(key)
    now = time.monotonic()
    if cached is not None and not refresh and now - cached[0] < PRS_CACHE_S:
        doc, fetched_age = cached[1], now - cached[0]
    else:
        tooling = require_tooling(cfg, tag)
        args = ["prs", "list", "--suite", suite_id, "--limit", str(limit)]
        if since:
            args += ["--since", since]
        res = run_suite_cli(tooling, _with_home(cfg, args), timeout=PRS_TIMEOUT_S)
        if res.code != 0 or res.doc is None:
            raise cli_error(res, {2: 422, 3: 409, 4: 502, 5: 502}, what="suite prs list")
        doc, fetched_age = res.doc, 0.0
        _prs_cache[key] = (now, doc)
    done = scaffolded_prs(d)
    prs = [
        {**p, "task_id": done.get(int(p.get("number") or 0))}
        for p in doc.get("prs") or []
        if isinstance(p, dict)
    ]
    return {
        "repo": doc.get("repo"),
        "since": since,
        "limit": limit,
        "cached_s": round(fetched_age),
        "prs": prs,
    }


def scaffold_task(
    cfg: RunnerConfig,
    suite_id: str,
    *,
    pr: int,
    tag: str | None = None,
    license_note: str = "",
) -> dict[str, Any]:
    d = suite_dir(cfg, suite_id)
    _refuse_if_frozen(d)
    if pr < 1:
        raise SuitesError(422, {"code": "invalid_option", "message": "pr must be a PR number"})
    note = license_note.strip()
    if len(note) > MAX_LICENSE_NOTE_CHARS or any(ord(c) < 32 and c not in "\n\t" for c in note):
        raise SuitesError(
            422,
            {
                "code": "invalid_option",
                "message": f"license note: plain text, at most {MAX_LICENSE_NOTE_CHARS} characters",
            },
        )
    existing = scaffolded_prs(d).get(pr)
    if existing:
        raise SuitesError(
            409, {"code": "exists", "message": f"PR #{pr} already has task {existing}"}
        )
    lock = _suite_lock(suite_id)
    if not lock.acquire(blocking=False):
        raise SuitesError(
            409, {"code": "busy", "message": f"another scaffold is running in {suite_id}"}
        )
    try:
        tooling = require_tooling(cfg, tag)
        res = run_suite_cli(
            tooling,
            _with_home(
                cfg,
                ["task", "scaffold", suite_id, "--pr", str(pr)]
                + (["--license-note", note] if note else []),
            ),
            timeout=SCAFFOLD_TIMEOUT_S,
        )
    finally:
        lock.release()
    if res.code != 0 or res.doc is None:
        raise cli_error(res, {2: 422, 3: 409, 4: 502, 5: 502, 8: 409}, what="suite task scaffold")
    task = str(res.doc.get("task_id") or "")
    if not TASK_ID_RE.match(task):
        raise SuitesError(502, {"code": "bad_output", "message": "scaffold named no task"})
    row = task_row(d, task, _manifest(d), [])
    warnings = res.doc.get("warnings")
    row["scaffold_warnings"] = (
        [str(w)[:300] for w in warnings][:20] if isinstance(warnings, list) else []
    )
    return row


def _active_job_problem(jobs: list[Job], suite_id: str, task: str | None) -> str | None:
    active = next((j for j in _jobs_for(jobs, suite_id, task) if j.active), None)
    if active is None:
        return None
    return f"job {active.id} ({active.kind}) is {active.status}"


def accept_task(
    cfg: RunnerConfig, suite_id: str, task: str, jobs: list[Job], *, tag: str | None = None
) -> dict[str, Any]:
    tdir = task_dir(cfg, suite_id, task)
    d = tdir.parent
    _refuse_if_frozen(d)
    busy = _active_job_problem(jobs, suite_id, task)
    if busy:
        raise SuitesError(409, {"code": "busy", "message": busy})
    row = task_row(d, task, _manifest(d), jobs)
    if row["dropped"]:
        raise SuitesError(409, {"code": "dropped", "message": f"{task} is dropped"})
    if row["stage"] != "validated":
        raise SuitesError(
            409,
            {
                "code": "not_validated",
                "message": f"{task} needs a PASS or WARN validation newer than its authoring",
            },
        )
    tooling = require_tooling(cfg, tag)
    res = run_suite_cli(
        tooling, _with_home(cfg, ["add-task", suite_id, "--task", task]), timeout=SHORT_TIMEOUT_S
    )
    if res.code != 0:
        raise cli_error(res, {1: 409, 2: 409, 5: 404}, what="suite add-task")
    return task_row(d, task, _manifest(d), jobs)


def drop_task(
    cfg: RunnerConfig,
    suite_id: str,
    task: str,
    jobs: list[Job],
    *,
    reason: str = "",
    tag: str | None = None,
) -> dict[str, Any]:
    """Take a task out of the suite's list (``suite remove-task``) and mark it dropped.

    The directory stays; ``build/dropped.json`` is the GUI's own marker.
    """
    tdir = task_dir(cfg, suite_id, task)
    d = tdir.parent
    _refuse_if_frozen(d)
    busy = _active_job_problem(jobs, suite_id, task)
    if busy:
        raise SuitesError(409, {"code": "busy", "message": busy})
    tooling = require_tooling(cfg, tag)
    require_unlocked(tooling)
    if task in [str(t) for t in _manifest(d).get("tasks") or []]:
        res = run_suite_cli(
            tooling,
            _with_home(cfg, ["remove-task", suite_id, "--task", task]),
            timeout=SHORT_TIMEOUT_S,
        )
        if res.code != 0:
            raise cli_error(res, {5: 404}, what="suite remove-task")
    build = tdir / "build"
    build.mkdir(exist_ok=True)
    record = {
        "schema": "vulcanbench.gui.dropped.v1",
        "reason": reason.strip()[:300],
        "dropped_at": _now().isoformat(timespec="seconds"),
    }
    tmp = build / "dropped.json.tmp"
    tmp.write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")
    tmp.replace(build / "dropped.json")
    return task_row(d, task, _manifest(d), jobs)


def freeze_suite(
    cfg: RunnerConfig, suite_id: str, jobs: list[Job], *, tag: str | None = None
) -> dict[str, Any]:
    detail = suite_detail(cfg, suite_id, jobs)
    ready = detail["freeze_ready"]
    if not ready["ok"]:
        raise SuitesError(
            409,
            {
                "code": "not_ready",
                "message": "; ".join(ready["problems"]),
                "problems": ready["problems"],
            },
        )
    tooling = require_tooling(cfg, tag)
    res = run_suite_cli(tooling, _with_home(cfg, ["freeze", suite_id]), timeout=FREEZE_TIMEOUT_S)
    if res.code != 0 or res.doc is None:
        raise cli_error(res, {1: 409, 2: 409, 5: 404}, what="suite freeze")
    return {
        "id": suite_id,
        "path": res.doc.get("path"),
        "version": res.doc.get("version"),
        "lock_digest": res.doc.get("lock_digest"),
        "n_tasks": res.doc.get("n_tasks"),
        "tooling": tooling,
    }


# ------------------------------------------------- queued build jobs --
#
# Authoring (one model session per task, spends quota) and validation (Docker,
# minutes per task) are queued jobs: built here as a plan, shown with the exact
# commands, queued only by the confirm click with the plan's hash, re-checked
# by ``validate_build_launch`` at confirm and again right before launch.

AUTHOR_TIMEOUT_S = 3600
AUTHOR_MAX_TURNS = 200
MAX_TASKS_PER_PLAN = 20
_MODEL_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._:/-]*$")


@dataclass(frozen=True)
class BuildForm:
    kind: str  # author | validate
    suite: str
    tasks: tuple[str, ...]
    tag: str | None = None
    harness: str = ""
    model: str = ""
    billing: str = "auto"
    effort: str | None = None
    cli_bin_dir: str = ""


def _check_build_form(form: BuildForm) -> None:  # noqa: PLR0912
    if form.kind not in BUILDER_KINDS:
        raise SuitesError(422, "kind must be author or validate")
    if not 1 <= len(form.tasks) <= MAX_TASKS_PER_PLAN:
        raise SuitesError(422, f"pick 1 to {MAX_TASKS_PER_PLAN} tasks")
    if len(set(form.tasks)) != len(form.tasks):
        raise SuitesError(422, "a task is listed twice")
    for t in form.tasks:
        check_task_id(t)
    if form.kind == "validate":
        return
    if form.harness not in sweep.CLI_HARNESSES:
        raise SuitesError(
            422, f"authoring runs through a CLI harness: one of {', '.join(sweep.CLI_HARNESSES)}"
        )
    if not _MODEL_RE.match(form.model) or len(form.model) > 120 or ":" in form.model:
        raise SuitesError(422, "give a bare model id for that harness")
    if form.billing not in AUTHOR_BILLING:
        raise SuitesError(422, f"billing must be one of {', '.join(AUTHOR_BILLING)}")
    if form.effort is not None:
        from harness.settings import blocked_efforts  # noqa: PLC0415

        if form.effort in blocked_efforts():
            raise SuitesError(
                422, f"effort {form.effort!r} is blocked by vulcanbench.toml and never runs"
            )
        if form.effort not in sweep.efforts_offered():
            raise SuitesError(422, f"unknown effort level {form.effort!r}")
    if form.cli_bin_dir:
        p = Path(form.cli_bin_dir).expanduser()
        if not p.is_absolute() or not p.is_dir():
            raise SuitesError(422, "CLI bin dir must be an existing absolute directory")


def _cli(tooling: dict[str, Any]) -> str:
    return str(Path(str(tooling["worktree"])) / ".venv" / "bin" / "vulcanbench")


def author_argv(
    cfg: RunnerConfig, tooling: dict[str, Any], form: BuildForm, task: str
) -> list[str]:
    argv = [
        _cli(tooling),
        "suite",
        "task",
        "author",
        form.suite,
        "--task",
        task,
        "--harness",
        form.harness,
        "--model",
        form.model,
        "--billing",
        form.billing,
    ]
    if form.effort:
        argv += ["--effort", form.effort]
    argv += [
        "--timeout",
        str(AUTHOR_TIMEOUT_S),
        "--max-turns",
        str(AUTHOR_MAX_TURNS),
        "--suites-home",
        str(cfg.suites_home),
        "--json",
    ]
    return argv


def validate_argv(
    cfg: RunnerConfig, tooling: dict[str, Any], form: BuildForm, task: str
) -> list[str]:
    return [
        _cli(tooling),
        "suite",
        "validate",
        form.suite,
        "--task",
        task,
        "--sandbox",
        "docker",
        "--suites-home",
        str(cfg.suites_home),
        "--json",
    ]


def build_job_plan(
    cfg: RunnerConfig, form: BuildForm, jobs: list[Job], *, docker_ok: bool
) -> dict[str, Any]:
    """An author or validate plan: one queued job per task, nothing started."""
    _check_build_form(form)
    d = suite_dir(cfg, form.suite)
    _refuse_if_frozen(d)
    manifest = _manifest(d)
    rows: dict[str, dict[str, Any]] = {}
    for t in form.tasks:
        task_dir(cfg, form.suite, t)  # 404 for a task that is not there
        row = task_row(d, t, manifest, jobs)
        if row["dropped"]:
            raise SuitesError(409, f"{t} is dropped")
        if row["active_job"] is not None:
            raise SuitesError(
                409, f"{t} has job {row['active_job']['id']} {row['active_job']['status']}"
            )
        if form.kind == "validate" and (row["author"] or {}).get("status") != "ok":
            raise SuitesError(409, f"{t} has no successful authoring session yet; author it first")
        rows[t] = row
    if form.kind == "validate" and not docker_ok:
        raise SuitesError(409, "validation runs in Docker and Docker is not running")
    tooling = require_tooling(cfg, form.tag)
    require_unlocked(tooling)
    wt = Path(str(tooling["worktree"]))
    path_prepend = str(Path(form.cli_bin_dir).expanduser()) if form.cli_bin_dir else ""
    common = {
        "suite": form.suite,
        "suite_name": manifest.get("display_name") or form.suite,
        "tag": tooling["tag"],
        "tag_commit": tooling["tag_commit"],
        "worktree": str(wt),
        "needs_prep": False,
        "cwd": str(wt),
        "env": {},
        "path_prepend": path_prepend,
        "suites_home": str(cfg.suites_home),
    }
    batch_id = f"{form.kind}-{form.suite}-{_now().strftime('%Y%m%d%H%M%S')}"
    warnings: list[str] = []
    entries: list[dict[str, Any]] = []
    for t in form.tasks:
        row = rows[t]
        argv = (
            author_argv(cfg, tooling, form, t)
            if form.kind == "author"
            else validate_argv(cfg, tooling, form, t)
        )
        entries.append(
            {
                "kind": form.kind,
                "job_name": f"{form.kind}-{form.suite}-{t}"[:120],
                "batch_id": batch_id,
                **common,
                "task": t,
                "task_dir": str(d / t),
                "pr": row["pr"],
                "harness": form.harness or None,
                "agent": form.harness or None,
                "model": form.model or None,
                "billing": form.billing if form.kind == "author" else None,
                "effort": form.effort,
                "argv": argv,
                "argv_token": f"{form.suite} --task {t}",
            }
        )
        if form.kind == "author" and row["author"] is not None:
            warnings.append(
                f"{t} was authored before: a new session starts from the current files and its "
                "accepted changes replace them"
            )
        if form.kind == "validate" and row["validate"] is not None:
            warnings.append(
                f"{t} was validated before: the new verdict replaces build/validate.json"
            )
    n = len(entries)
    if form.kind == "author":
        doctor = sweep.run_doctor(wt, form.harness, path_prepend=path_prepend)
        if doctor["status"] == "not_ready":
            warnings.append(
                f"harness doctor says {form.harness} is not ready: {doctor.get('detail')}"
            )
        elif doctor["status"] == "error":
            warnings.append(f"harness doctor failed: {doctor.get('note')}")
        warnings.append(
            "each session runs on this Mac as you, through the harness CLI, in a scratch copy of the "
            "task with network off; only the issue, hidden tests, gold patch and allowed metadata "
            "keys are copied back"
        )
        estimate = {
            "sessions": n,
            "per_session_cap_s": AUTHOR_TIMEOUT_S,
            "max_turns": AUTHOR_MAX_TURNS,
            "max_wall_s": n * AUTHOR_TIMEOUT_S,
            "billing": (
                f"{form.harness} subscription login (no metered API)"
                if form.billing == "subscription"
                else "auto: the subscription login when the harness has one, else the API key"
            ),
            "cost": (
                "open-ended: no price table covers authoring; budget about one Frontier v4 task "
                f"run per pull request, {n} in this plan"
            ),
            "quota": "spends model quota once per task; nothing retries, a refused session stays failed",
            "infra_retries": "none",
        }
    else:
        doctor = {"status": "not_applicable", "note": "validation makes no model call"}
        estimate = {
            "sessions": 0,
            "tasks": n,
            "billing": "none: Docker runs the hidden tests at the base and after the gold patch, three times",
            "cost": "no model calls",
            "quota": "none",
            "infra_retries": "none",
            "max_wall_s": None,
        }
    spec: dict[str, Any] = {
        "kind": form.kind,
        "job_name": batch_id,
        "batch_id": batch_id,
        **common,
        "tasks": list(form.tasks),
        "harness": form.harness or None,
        "model": form.model or None,
        "billing": form.billing if form.kind == "author" else None,
        "effort": form.effort,
        "jobs": entries,
        "doctor": doctor,
        "estimate": estimate,
        "tooling": tooling,
        "warnings": warnings,
        "usage_check_required": False,
    }
    spec["hash"] = launch.plan_hash(spec)
    return spec


def validate_build_launch(cfg: RunnerConfig, spec: dict[str, Any]) -> list[str]:  # noqa: PLR0912
    """Re-check one author or validate job right before confirm and before its child starts."""
    problems: list[str] = []
    kind = spec.get("kind")
    tag = str(spec.get("tag") or "")
    problems.extend(launch.validate_worktree(cfg, tag))
    wt = Path(str(spec.get("worktree")))
    if wt != launch.worktree_dir(cfg, tag):
        problems.append("worktree path does not match the tag")
    if not (wt / BUILDER_MARKER).is_file():
        problems.append(f"{tag} has no suite builder")
    argv = [str(a) for a in (spec.get("argv") or [])]
    head = ["suite", "task", "author"] if kind == "author" else ["suite", "validate"]
    if (
        not argv
        or Path(argv[0]) != wt / ".venv" / "bin" / "vulcanbench"
        or argv[1 : 1 + len(head)] != head
    ):
        problems.append(f"argv is not the worktree's vulcanbench {' '.join(head)}")
        return problems
    suite_id = str(spec.get("suite") or "")
    task = str(spec.get("task") or "")
    if argv[1 + len(head)] != suite_id or not SUITE_ID_RE.match(suite_id):
        problems.append("argv names a different suite")
    if "--task" not in argv or argv[argv.index("--task") + 1] != task or not TASK_ID_RE.match(task):
        problems.append("argv names a different task")
    if (
        "--suites-home" not in argv
        or Path(argv[argv.index("--suites-home") + 1]) != cfg.suites_home
    ):
        problems.append("argv's --suites-home is not the configured suites home")
    d = cfg.suites_home / suite_id
    if not (d / task / "metadata.json").is_file() or Path(str(spec.get("task_dir"))) != d / task:
        problems.append(f"task {task} is not in {d}")
    if (d / "suite.lock.json").is_file():
        problems.append(f"{suite_id} is frozen")
    if "--allow-dirty" in argv:
        problems.append("argv contains --allow-dirty")
    if kind == "author":
        from harness.settings import blocked_efforts  # noqa: PLC0415

        if "--effort" in argv and argv[argv.index("--effort") + 1] in blocked_efforts():
            problems.append("effort is blocked by vulcanbench.toml")
        if "--harness" not in argv or argv[argv.index("--harness") + 1] not in sweep.CLI_HARNESSES:
            problems.append("argv's harness is not a CLI harness")
    elif "--sandbox" not in argv or argv[argv.index("--sandbox") + 1] != "docker":
        problems.append("validation must run with --sandbox docker")
    for name in spec.get("env") or {}:
        problems.append(f"unexpected env override {name}")
    prepend = str(spec.get("path_prepend") or "")
    if prepend and not Path(prepend).is_dir():
        problems.append(f"CLI bin dir {prepend} is gone")
    return problems


def read_build_record(spec: dict[str, Any], name: str) -> dict[str, Any] | None:
    """``build/author.json`` or ``build/validate.json`` of the job's task (for the supervisor)."""
    return read_json(Path(str(spec.get("task_dir") or "")) / "build" / name)
