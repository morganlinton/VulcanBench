"""Worker admin API: every job is also triggerable on demand."""

from __future__ import annotations

import os
import threading
import time

from orderflow.ofkit.http import Request, Router
from orderflow.worker import hotsync, readmodel, reconcile

router = Router("worker")


@router.route("GET", "/healthz")
def healthz(_request: Request) -> tuple[int, dict]:
    return 200, {"ok": True, "service": "worker"}


@router.route("POST", "/admin/tick")
def tick(_request: Request) -> tuple[int, dict]:
    return 200, readmodel.tick()


@router.route("POST", "/admin/hot_sync")
def hot_sync(_request: Request) -> tuple[int, dict]:
    return 200, hotsync.run()


@router.route("POST", "/admin/reconcile")
def do_reconcile(_request: Request) -> tuple[int, dict]:
    return 200, reconcile.run()


def start_autopoll() -> None:
    """Background polling loop (disabled when OF_WORKER_AUTOPOLL=0)."""
    if os.environ.get("OF_WORKER_AUTOPOLL", "0") != "1":
        return
    interval = float(os.environ.get("OF_WORKER_POLL_INTERVAL_S", "1.0"))

    def loop() -> None:
        while True:
            try:
                readmodel.tick()
            except Exception:  # noqa: BLE001 - keep polling
                pass
            time.sleep(interval)

    threading.Thread(target=loop, name="worker-autopoll", daemon=True).start()
