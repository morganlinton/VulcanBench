"""The runner's sqlite state: plans, confirm, jobs, stop requests."""

from __future__ import annotations

import sqlite3
from datetime import UTC, datetime, timedelta
from pathlib import Path

import pytest

from backend.runner.state import PLAN_TTL_S, PlanError, State


class Clock:
    def __init__(self) -> None:
        self.t = datetime(2026, 10, 29, 12, 0, tzinfo=UTC)

    def __call__(self) -> datetime:
        return self.t

    def advance(self, s: float) -> None:
        self.t += timedelta(seconds=s)


@pytest.fixture
def clock() -> Clock:
    return Clock()


@pytest.fixture
def state(tmp_path: Path, clock: Clock) -> State:
    return State(tmp_path / "runner" / "runner.db", now=clock)


def test_plan_confirm_creates_one_queued_job(state: State) -> None:
    plan = state.create_plan("gate", {"job_name": "opus-demo", "argv": ["bash"]}, "abc123def456")
    assert state.get_plan(plan.id) is not None and state.plan_is_live(plan) is None
    with pytest.raises(PlanError) as bad:
        state.confirm_plan(plan.id, "0000000000000000")
    assert bad.value.status == 409
    (job,) = state.confirm_plan(plan.id, "abc123def456")
    assert job.status == "queued" and job.name == "opus-demo" and job.plan_hash == "abc123def456"
    assert state.queued_jobs() == [job]
    again = state.get_plan(plan.id)
    assert again is not None and again.confirmed_at and again.job_id == job.id
    with pytest.raises(PlanError, match="already confirmed"):
        state.confirm_plan(plan.id, "abc123def456")
    assert len(state.list_jobs()) == 1


def test_plan_expires(state: State, clock: Clock) -> None:
    plan = state.create_plan("gate", {"job_name": "x"}, "h" * 16)
    clock.advance(PLAN_TTL_S + 1)
    with pytest.raises(PlanError, match="expired"):
        state.confirm_plan(plan.id, "h" * 16)
    assert state.get_plan("missing") is None
    with pytest.raises(PlanError) as nf:
        state.confirm_plan("missing", "h" * 16)
    assert nf.value.status == 404


def test_usage_check_is_stored_on_the_plan(state: State) -> None:
    plan = state.create_plan("gate", {"job_name": "x"}, "h" * 16)
    plan = state.set_usage_check(plan.id, {"status": "allowed", "would_start": True})
    assert plan.usage_check == {"status": "allowed", "would_start": True}
    assert plan.usage_checked_at == "2026-10-29T12:00:00+00:00"


def test_job_updates_notes_and_stop_requests(state: State, clock: Clock) -> None:
    job = state.create_job("gate", "astra-demo", {"argv": ["bash"]})
    job = state.update_job(
        job.id, status="running", pid=4242, proc_lstart="Thu Oct 29 12:00:00 2026"
    )
    assert job.active and state.active_jobs() == [job] and state.queued_jobs() == []
    state.add_note(job.id, "hello")
    assert state.get_job(job.id).notes == ["hello"]  # type: ignore[union-attr]
    clock.advance(5)
    stopped = state.request_stop(job.id, "operator")
    assert stopped is not None and stopped.stop_requested_at == "2026-10-29T12:00:05+00:00"
    assert stopped.stop_reason == "operator" and stopped.status == "running"
    # A second request changes nothing.
    assert state.request_stop(job.id, "again") == stopped
    with pytest.raises(ValueError, match="unknown job fields"):
        state.update_job(job.id, nope=1)
    assert state.job_by_name("astra-demo") == stopped
    assert state.job_by_name("other") is None


def test_queued_job_stop_and_delete(state: State) -> None:
    a = state.create_job("gate", "a", {})
    b = state.create_job("gate", "b", {})
    gone = state.request_stop(a.id, "changed my mind")
    assert gone is not None and gone.status == "stopped" and gone.finished_at
    assert "removed from the queue" in gone.notes[0]
    assert state.delete_queued(b.id) is True
    assert state.delete_queued(a.id) is False  # not queued any more
    assert state.get_job(b.id) is None
    finished = state.update_job(a.id, status="finished")
    assert state.request_stop(finished.id, "x") is None


def test_schema_version_guard(tmp_path: Path) -> None:
    path = tmp_path / "runner.db"
    State(path)
    con = sqlite3.connect(path)
    con.execute("UPDATE schema_version SET version = 99")
    con.commit()
    con.close()
    with pytest.raises(RuntimeError, match="newer than this backend"):
        State(path)
