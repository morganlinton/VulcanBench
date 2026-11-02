"""Runner state: launch plans and queued or running jobs, in a stdlib sqlite file.

Deliberately not SQLModel: the backend's ``SQLModel.metadata`` is global, so a
runner table defined that way would be created in the public Postgres by
``db.init_db()`` and picked up by alembic. This file lives at
``<runner home>/runner.db``, outside every checkout, and survives backend
restarts; the supervisor re-adopts the jobs recorded here on start.

One short-lived connection per call keeps it safe to use from the request
handlers and the supervisor thread at the same time.
"""

from __future__ import annotations

import json
import secrets
import sqlite3
from collections.abc import Callable
from dataclasses import dataclass, field
from datetime import UTC, datetime, timedelta
from pathlib import Path
from typing import Any

SCHEMA_VERSION = 1
PLAN_TTL_S = 600
# ``waiting``: a suite level that hit a quota window and continues on its own
# after ``spec["resume_at"]`` (only when the plan allowed it, and only so many
# times). ``incomplete``: a level that ended with tasks still missing.
ACTIVE_STATUSES = ("queued", "waiting", "preparing", "running", "stopping")
PENDING_STATUSES = ("queued", "waiting")
FINAL_STATUSES = ("finished", "incomplete", "stopped", "failed", "infra_failed", "lost")


class PlanError(Exception):
    def __init__(self, status: int, detail: str) -> None:
        super().__init__(detail)
        self.status = status
        self.detail = detail


def utcnow() -> datetime:
    return datetime.now(UTC)


def iso(dt: datetime) -> str:
    return dt.astimezone(UTC).isoformat(timespec="seconds")


def parse(value: str | None) -> datetime | None:
    if not value:
        return None
    try:
        return datetime.fromisoformat(value)
    except ValueError:
        return None


@dataclass
class Plan:
    id: str
    hash: str
    kind: str
    spec: dict[str, Any]
    created_at: str
    expires_at: str
    confirmed_at: str | None = None
    job_id: int | None = None
    # Columns kept from the gate milestone (sqlite cannot drop them); unused.
    usage_check: dict[str, Any] | None = None
    usage_checked_at: str | None = None

    def to_dict(self) -> dict[str, Any]:
        return {
            "id": self.id,
            "hash": self.hash,
            "kind": self.kind,
            "spec": self.spec,
            "created_at": self.created_at,
            "expires_at": self.expires_at,
            "confirmed_at": self.confirmed_at,
            "job_id": self.job_id,
        }


@dataclass
class Job:
    id: int
    kind: str
    name: str
    status: str
    spec: dict[str, Any]
    plan_hash: str | None
    created_at: str
    started_at: str | None = None
    finished_at: str | None = None
    phase: str | None = None  # prepare | main
    pid: int | None = None
    pgid: int | None = None
    proc_lstart: str | None = None
    boot_time: str | None = None
    argv_token: str | None = None
    exit_code: int | None = None
    stop_requested_at: str | None = None
    stop_reason: str | None = None
    stop_signalled_at: str | None = None
    stop_escalated_at: str | None = None
    console_log: str | None = None
    notes: list[str] = field(default_factory=list)

    @property
    def active(self) -> bool:
        return self.status in ACTIVE_STATUSES

    def to_dict(self) -> dict[str, Any]:
        return {
            "id": self.id,
            "kind": self.kind,
            "name": self.name,
            "status": self.status,
            "spec": self.spec,
            "plan_hash": self.plan_hash,
            "created_at": self.created_at,
            "started_at": self.started_at,
            "finished_at": self.finished_at,
            "phase": self.phase,
            "pid": self.pid,
            "pgid": self.pgid,
            "exit_code": self.exit_code,
            "stop_requested_at": self.stop_requested_at,
            "stop_reason": self.stop_reason,
            "stop_signalled_at": self.stop_signalled_at,
            "stop_escalated_at": self.stop_escalated_at,
            "console_log": self.console_log,
            "notes": list(self.notes),
        }


_JOB_COLUMNS = (
    "id, kind, name, status, spec, plan_hash, created_at, started_at, finished_at, phase, "
    "pid, pgid, proc_lstart, boot_time, argv_token, exit_code, stop_requested_at, "
    "stop_reason, stop_signalled_at, stop_escalated_at, console_log, notes"
)
_JOB_FIELDS = frozenset(c.strip() for c in _JOB_COLUMNS.split(","))

_SCHEMA = """
CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS plan (
    id TEXT PRIMARY KEY,
    hash TEXT NOT NULL,
    kind TEXT NOT NULL,
    spec TEXT NOT NULL,
    created_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    confirmed_at TEXT,
    job_id INTEGER,
    usage_check TEXT,
    usage_checked_at TEXT
);
CREATE TABLE IF NOT EXISTS job (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    kind TEXT NOT NULL,
    name TEXT NOT NULL,
    status TEXT NOT NULL,
    spec TEXT NOT NULL,
    plan_hash TEXT,
    created_at TEXT NOT NULL,
    started_at TEXT,
    finished_at TEXT,
    phase TEXT,
    pid INTEGER,
    pgid INTEGER,
    proc_lstart TEXT,
    boot_time TEXT,
    argv_token TEXT,
    exit_code INTEGER,
    stop_requested_at TEXT,
    stop_reason TEXT,
    stop_signalled_at TEXT,
    stop_escalated_at TEXT,
    console_log TEXT,
    notes TEXT NOT NULL DEFAULT '[]'
);
CREATE INDEX IF NOT EXISTS job_status ON job (status);
"""


def _row_job(row: sqlite3.Row) -> Job:
    d = dict(row)
    d["spec"] = json.loads(d["spec"])
    d["notes"] = json.loads(d["notes"] or "[]")
    return Job(**d)


def _row_plan(row: sqlite3.Row) -> Plan:
    d = dict(row)
    d["spec"] = json.loads(d["spec"])
    d["usage_check"] = json.loads(d["usage_check"]) if d["usage_check"] else None
    return Plan(**d)


class State:
    def __init__(self, path: Path, *, now: Callable[[], datetime] = utcnow) -> None:
        self.path = path
        self.now = now
        path.parent.mkdir(parents=True, exist_ok=True)
        with self._connect() as con:
            con.executescript(_SCHEMA)
            row = con.execute("SELECT version FROM schema_version").fetchone()
            if row is None:
                con.execute("INSERT INTO schema_version (version) VALUES (?)", (SCHEMA_VERSION,))
            elif int(row[0]) > SCHEMA_VERSION:
                raise RuntimeError(
                    f"{path}: schema version {row[0]} is newer than this backend ({SCHEMA_VERSION})"
                )

    def _connect(self) -> sqlite3.Connection:
        con = sqlite3.connect(self.path, timeout=5.0, isolation_level=None)
        con.row_factory = sqlite3.Row
        con.execute("PRAGMA journal_mode=WAL")
        con.execute("PRAGMA busy_timeout=5000")
        return con

    # ---------------------------------------------------------------- plans --

    def create_plan(self, kind: str, spec: dict[str, Any], plan_hash: str) -> Plan:
        now = self.now()
        plan = Plan(
            id=secrets.token_urlsafe(12),
            hash=plan_hash,
            kind=kind,
            spec=spec,
            created_at=iso(now),
            expires_at=iso(now + timedelta(seconds=PLAN_TTL_S)),
        )
        with self._connect() as con:
            con.execute(
                "INSERT INTO plan (id, hash, kind, spec, created_at, expires_at) "
                "VALUES (?, ?, ?, ?, ?, ?)",
                (plan.id, plan.hash, plan.kind, json.dumps(spec), plan.created_at, plan.expires_at),
            )
        return plan

    def get_plan(self, plan_id: str) -> Plan | None:
        with self._connect() as con:
            row = con.execute("SELECT * FROM plan WHERE id = ?", (plan_id,)).fetchone()
        return _row_plan(row) if row else None

    def plan_is_live(self, plan: Plan) -> str | None:
        """None if the plan can still be confirmed, else the reason it cannot."""
        if plan.confirmed_at:
            return "plan already confirmed"
        expires = parse(plan.expires_at)
        if expires is None or self.now() >= expires:
            return "plan expired; build it again"
        return None

    def confirm_plan(self, plan_id: str, plan_hash: str) -> list[Job]:
        """Turn a live plan into queued jobs (one per ``spec["jobs"]`` entry, else one).

        The hash must match what the page showed.
        """
        plan = self.get_plan(plan_id)
        if plan is None:
            raise PlanError(404, "plan not found")
        if plan.hash != plan_hash:
            raise PlanError(
                409, "plan hash does not match; the plan shown is not the one confirmed"
            )
        reason = self.plan_is_live(plan)
        if reason:
            raise PlanError(409, reason)
        now = iso(self.now())
        entries = [j for j in (plan.spec.get("jobs") or []) if isinstance(j, dict)] or [plan.spec]
        ids: list[int] = []
        with self._connect() as con:
            con.execute("BEGIN IMMEDIATE")
            row = con.execute("SELECT confirmed_at FROM plan WHERE id = ?", (plan_id,)).fetchone()
            if row is None or row["confirmed_at"]:
                con.execute("ROLLBACK")
                raise PlanError(409, "plan already confirmed")
            for entry in entries:
                cur = con.execute(
                    "INSERT INTO job (kind, name, status, spec, plan_hash, created_at, notes) "
                    "VALUES (?, ?, 'queued', ?, ?, ?, '[]')",
                    (
                        str(entry.get("kind") or plan.kind),
                        str(entry.get("job_name") or plan.kind),
                        json.dumps({**entry, "plan_id": plan_id}),
                        plan.hash,
                        now,
                    ),
                )
                ids.append(int(cur.lastrowid or 0))
            con.execute(
                "UPDATE plan SET confirmed_at = ?, job_id = ? WHERE id = ?", (now, ids[0], plan_id)
            )
            con.execute("COMMIT")
        jobs = [self.get_job(i) for i in ids]
        assert all(j is not None for j in jobs)
        return [j for j in jobs if j is not None]

    # ----------------------------------------------------------------- jobs --

    def create_job(
        self, kind: str, name: str, spec: dict[str, Any], plan_hash: str | None = None
    ) -> Job:
        """A queued job without a plan (tests and internal callers); the GUI goes through confirm_plan."""
        with self._connect() as con:
            cur = con.execute(
                "INSERT INTO job (kind, name, status, spec, plan_hash, created_at, notes) "
                "VALUES (?, ?, 'queued', ?, ?, ?, '[]')",
                (kind, name, json.dumps(spec), plan_hash, iso(self.now())),
            )
            job_id = int(cur.lastrowid or 0)
        job = self.get_job(job_id)
        assert job is not None
        return job

    def get_job(self, job_id: int) -> Job | None:
        with self._connect() as con:
            row = con.execute(f"SELECT {_JOB_COLUMNS} FROM job WHERE id = ?", (job_id,)).fetchone()
        return _row_job(row) if row else None

    def job_by_name(self, name: str) -> Job | None:
        """The most recent job with this name (gate job names are unique on disk anyway)."""
        with self._connect() as con:
            row = con.execute(
                f"SELECT {_JOB_COLUMNS} FROM job WHERE name = ? ORDER BY id DESC LIMIT 1", (name,)
            ).fetchone()
        return _row_job(row) if row else None

    def list_jobs(self, *, limit: int = 100) -> list[Job]:
        with self._connect() as con:
            rows = con.execute(
                f"SELECT {_JOB_COLUMNS} FROM job ORDER BY id DESC LIMIT ?", (limit,)
            ).fetchall()
        return [_row_job(r) for r in rows]

    def jobs_with_status(self, *statuses: str) -> list[Job]:
        marks = ",".join("?" for _ in statuses)
        with self._connect() as con:
            rows = con.execute(
                f"SELECT {_JOB_COLUMNS} FROM job WHERE status IN ({marks}) ORDER BY id",
                statuses,
            ).fetchall()
        return [_row_job(r) for r in rows]

    def active_jobs(self) -> list[Job]:
        return self.jobs_with_status("preparing", "running", "stopping")

    def queued_jobs(self) -> list[Job]:
        return self.jobs_with_status("queued")

    def waiting_jobs(self) -> list[Job]:
        return self.jobs_with_status("waiting")

    def due_waiting(self, now: datetime | None = None) -> list[Job]:
        """Waiting jobs whose quota pause is over, oldest first."""
        now = now or self.now()
        out: list[Job] = []
        for job in self.waiting_jobs():
            resume = (
                parse(str(job.spec.get("resume_at") or "")) if job.spec.get("resume_at") else None
            )
            if resume is None or now >= resume:
                out.append(job)
        return out

    def update_job(self, job_id: int, **fields: Any) -> Job:
        bad = set(fields) - _JOB_FIELDS - {"notes"}
        if bad or "id" in fields:
            raise ValueError(f"unknown job fields: {sorted(bad)}")
        if not fields:
            job = self.get_job(job_id)
            if job is None:
                raise KeyError(job_id)
            return job
        sets = ", ".join(f"{k} = ?" for k in fields)
        values = [json.dumps(v) if k in {"spec", "notes"} else v for k, v in fields.items()]
        with self._connect() as con:
            con.execute(f"UPDATE job SET {sets} WHERE id = ?", (*values, job_id))
        job = self.get_job(job_id)
        if job is None:
            raise KeyError(job_id)
        return job

    def add_note(self, job_id: int, note: str) -> None:
        job = self.get_job(job_id)
        if job is None:
            return
        self.update_job(job_id, notes=[*job.notes, note])

    def request_stop(self, job_id: int, reason: str) -> Job | None:
        """Persist a stop request; the supervisor acts on it (and keeps acting after a restart)."""
        job = self.get_job(job_id)
        if job is None or not job.active:
            return None
        if job.status in PENDING_STATUSES:
            note = (
                "removed from the queue before it started"
                if job.status == "queued"
                else "stopped while waiting for the quota window; nothing continues"
            )
            return self.update_job(
                job_id,
                status="stopped",
                stop_reason=reason,
                finished_at=iso(self.now()),
                notes=[*job.notes, note],
            )
        if job.stop_requested_at:
            return job
        return self.update_job(job_id, stop_requested_at=iso(self.now()), stop_reason=reason)

    def delete_queued(self, job_id: int) -> bool:
        with self._connect() as con:
            cur = con.execute(
                "DELETE FROM job WHERE id = ? AND status IN ('queued', 'waiting')", (job_id,)
            )
        return bool(cur.rowcount)
