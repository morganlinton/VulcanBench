"""Optional SQLModel persistence for the API.

When ``DATABASE_URL`` is set, runs and feedback are stored in a database
(Postgres in production, SQLite in tests). When it is unset, the API falls back
to scanning the ``./runs/`` filesystem — so nothing here is required for local
use.

Run rows mirror the dict shape ``harness.leaderboard.scan_leaderboard`` returns,
so the same ``aggregate_by_model`` logic works against either source. The full
run summary is stored verbatim in a JSON column.
"""

from __future__ import annotations

import os
from datetime import UTC, datetime
from typing import Any

from sqlalchemy import JSON as SA_JSON
from sqlalchemy import Column
from sqlalchemy.engine import Engine
from sqlmodel import Field, Session, SQLModel, create_engine, select

DATABASE_URL = os.environ.get("DATABASE_URL")

_ROW_FIELDS = (
    "task_id",
    "model",
    "suite",
    "total",
    "functional",
    "quality",
    "security",
    "efficiency",
    "human_like",
    "steps",
    "total_tokens",
    "cost_usd",
    "duration_s",
    "finished_at",
)


class Run(SQLModel, table=True):
    run_id: str = Field(primary_key=True)
    task_id: str | None = None
    model: str | None = None
    suite: str | None = None
    total: float | None = None
    functional: float | None = None
    quality: float | None = None
    security: float | None = None
    efficiency: float | None = None
    human_like: float | None = None
    steps: int | None = None
    total_tokens: int | None = None
    cost_usd: float | None = None
    duration_s: float | None = None
    finished_at: str | None = None
    summary: dict[str, Any] = Field(default_factory=dict, sa_column=Column(SA_JSON))


class Feedback(SQLModel, table=True):
    id: int | None = Field(default=None, primary_key=True)
    task_id: str | None = None
    run_id: str | None = None
    rating: int | None = None
    comment: str | None = None
    created_at: str = Field(default_factory=lambda: datetime.now(UTC).isoformat())


def _make_engine(url: str | None) -> Engine | None:
    return create_engine(url, echo=False) if url else None


_engine: Engine | None = _make_engine(DATABASE_URL)


def configure(url: str | None) -> None:
    """Point the store at ``url`` (or disable it with ``None``). Used by tests."""
    global DATABASE_URL, _engine  # noqa: PLW0603 — module-level store handle
    if _engine is not None:
        _engine.dispose()
    DATABASE_URL = url
    _engine = _make_engine(url)


def close_db() -> None:
    """Dispose the configured engine, releasing pooled DB connections."""
    if _engine is not None:
        _engine.dispose()


def enabled() -> bool:
    """True when a database is configured (``DATABASE_URL`` set)."""
    return _engine is not None


def init_db() -> None:
    if _engine is not None:
        SQLModel.metadata.create_all(_engine)


def _row_from_summary(summary: dict[str, Any]) -> Run:
    scores = summary.get("scores", {})
    return Run(
        run_id=summary.get("run_id", ""),
        task_id=summary.get("task_id"),
        model=summary.get("model"),
        suite=summary.get("suite"),
        total=scores.get("total"),
        functional=scores.get("functional"),
        quality=scores.get("quality"),
        security=scores.get("security"),
        efficiency=scores.get("efficiency"),
        human_like=scores.get("human_like"),
        steps=summary.get("steps"),
        total_tokens=summary.get("total_tokens"),
        cost_usd=summary.get("cost_usd"),
        duration_s=summary.get("duration_s"),
        finished_at=summary.get("finished_at"),
        summary=summary,
    )


def upsert_run(summary: dict[str, Any]) -> None:
    """Insert or replace a run from its summary dict."""
    if _engine is None:
        return
    row = _row_from_summary(summary)
    with Session(_engine) as session:
        session.merge(row)
        session.commit()


def leaderboard_rows() -> list[dict[str, Any]]:
    """Per-run rows in the same shape as ``scan_leaderboard``."""
    if _engine is None:
        return []
    with Session(_engine) as session:
        runs = session.exec(select(Run)).all()
    return [
        {
            "run_id": r.run_id,
            "task_hash": (r.summary or {}).get("task_hash"),
            **{f: getattr(r, f) for f in _ROW_FIELDS},
        }
        for r in runs
    ]


def get_summary(run_id: str) -> dict[str, Any] | None:
    if _engine is None:
        return None
    with Session(_engine) as session:
        row = session.get(Run, run_id)
    return row.summary if row is not None else None


def add_feedback(
    task_id: str | None, run_id: str | None, rating: int | None, comment: str | None
) -> dict[str, Any]:
    if _engine is None:
        raise RuntimeError("database not configured")
    fb = Feedback(task_id=task_id, run_id=run_id, rating=rating, comment=comment)
    with Session(_engine) as session:
        session.add(fb)
        session.commit()
        session.refresh(fb)
    return {"id": fb.id, "created_at": fb.created_at}
