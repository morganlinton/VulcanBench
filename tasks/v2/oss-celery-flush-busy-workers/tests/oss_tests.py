"""Hidden test for oss-celery-flush-busy-workers.

When the broker connection drops, the consumer calls ``AsynPool.flush()``.
A worker still executing an accepted (running) task is genuinely busy, so its
inqueue write-fd must remain in ``_busy_workers`` across the flush; clearing it
outright desynchronizes the fair scheduler from reality and lets a new task be
written onto the still-busy worker. Workers that are NOT running an accepted job
must be dropped from the busy set. Expected behavior captured from the upstream
fix (celery/celery PR #10346).

Run with PYTHONPATH=. so the workspace's celery/ is the celery under test.
"""
from __future__ import annotations

from unittest.mock import Mock, patch

from celery.concurrency import asynpool


def _make_pool():
    with patch("billiard.pool.Pool._create_worker_process"):
        pool = asynpool.AsynPool(processes=2, synack=False, threads=False)
    pool._state = asynpool.RUN
    pool.maintain_pool = Mock(name="maintain_pool")
    pool._active_writers.clear()
    pool.outbound_buffer.clear()
    return pool


def _running_job(fd, *, via_write_to=True):
    proc = Mock(name="proc")
    proc.inqW_fd = fd
    proc._is_alive.return_value = True
    job = Mock(name="job")
    job._accepted = True
    job._writer.return_value = None
    if via_write_to:
        job._write_to = proc
        job._scheduled_for = proc
    else:
        job._write_to = None
        job._scheduled_for = proc
    return job


def test_flush_preserves_busy_worker_running_accepted_job():
    pool = _make_pool()
    pool._cache = {1: _running_job(7)}
    pool._busy_workers = {7}
    pool.flush()
    assert 7 in pool._busy_workers


def test_flush_preserves_busy_worker_via_scheduled_for_fallback():
    # Job accepted but body not yet fully written: _write_to unset, fall back to
    # _scheduled_for for the busy fd.
    pool = _make_pool()
    pool._cache = {1: _running_job(5, via_write_to=False)}
    pool._busy_workers = {5}
    pool.flush()
    assert 5 in pool._busy_workers


def test_flush_drops_worker_without_accepted_job():
    # A worker in _busy_workers that is not running any accepted job must be
    # removed (the flush must not simply keep the whole set unchanged).
    pool = _make_pool()
    pool._cache = {}
    pool._busy_workers = {9}
    pool.flush()
    assert 9 not in pool._busy_workers
