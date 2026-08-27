"""Shared fixtures and load drivers for the orderflow hidden tests.

Each pytest invocation boots the full five-service deployment from the
workspace code (one OS process per service) against the run's live
postgres/redis stack, and tears it down at exit. Concurrency drivers use
spawn-based process pools behind a barrier so every request storm starts
simultaneously and in-process locks cannot fake cross-process correctness.

State discipline: the compose stack lives for the whole grading session, so
every test namespaces its SKUs and order ids and asserts only on rows it
created.
"""

from __future__ import annotations

import datetime as dt
import multiprocessing as mp
import uuid
from pathlib import Path

import pytest
import requests

from orderflow import launcher

REQ_TIMEOUT = 60


@pytest.fixture(scope="session")
def stack():
    deployment = launcher.start(Path.cwd())
    try:
        yield deployment.urls
    finally:
        deployment.stop()


def uid(prefix: str) -> str:
    return f"{prefix}-{uuid.uuid4().hex[:10]}"


def far_future_iso(seconds: int = 3600) -> str:
    return (dt.datetime.now(dt.timezone.utc) + dt.timedelta(seconds=seconds)).isoformat()


# -- HTTP helpers -----------------------------------------------------------


def seed_product(urls, sku: str, available: int) -> None:
    r = requests.post(
        urls["inventory"] + "/internal/products",
        json={"sku": sku, "available": available},
        timeout=REQ_TIMEOUT,
    )
    assert r.status_code == 201, r.text


def seed_coupon(urls, code: str, percent_off: int) -> None:
    r = requests.post(
        urls["orders"] + "/internal/coupons",
        json={"code": code, "percent_off": percent_off},
        timeout=REQ_TIMEOUT,
    )
    assert r.status_code == 201, r.text


def create_order(urls, order_id: str, sku: str, qty: int, unit_price_cents: int) -> None:
    r = requests.post(
        urls["gateway"] + "/orders",
        json={
            "order_id": order_id,
            "sku": sku,
            "qty": qty,
            "unit_price_cents": unit_price_cents,
        },
        timeout=REQ_TIMEOUT,
    )
    assert r.status_code == 201, r.text


def get_order(urls, order_id: str) -> dict:
    r = requests.get(urls["gateway"] + f"/orders/{order_id}", timeout=REQ_TIMEOUT)
    assert r.status_code == 200, r.text
    return r.json()


def charges_for(urls, order_id: str) -> list[dict]:
    r = requests.get(
        urls["billing"] + f"/internal/charges/by_order/{order_id}", timeout=REQ_TIMEOUT
    )
    assert r.status_code == 200, r.text
    return r.json()["charges"]


def product_row(urls, sku: str) -> dict:
    r = requests.get(urls["inventory"] + f"/internal/products/{sku}", timeout=REQ_TIMEOUT)
    assert r.status_code == 200, r.text
    return r.json()


def reservations_for(urls, order_id: str) -> list[dict]:
    r = requests.get(
        urls["inventory"] + f"/internal/reservations/by_order/{order_id}",
        timeout=REQ_TIMEOUT,
    )
    assert r.status_code == 200, r.text
    return r.json()["reservations"]


def expire_all(urls) -> dict:
    r = requests.post(
        urls["inventory"] + "/internal/expire",
        json={"now": far_future_iso()},
        timeout=180,
    )
    assert r.status_code == 200, r.text
    return r.json()


def tick(urls) -> dict:
    r = requests.post(urls["worker"] + "/admin/tick", json={}, timeout=180)
    assert r.status_code == 200, r.text
    return r.json()


def reconcile(urls) -> dict:
    r = requests.post(urls["worker"] + "/admin/reconcile", json={}, timeout=180)
    assert r.status_code == 200, r.text
    return r.json()


def hot_sync(urls) -> dict:
    r = requests.post(urls["worker"] + "/admin/hot_sync", json={}, timeout=180)
    assert r.status_code == 200, r.text
    return r.json()


# -- concurrency drivers (top-level so spawn can pickle them) ---------------


def job_checkout(args):
    url, order_id, barrier = args
    barrier.wait()
    try:
        r = requests.post(f"{url}/checkout/{order_id}", json={}, timeout=120)
        return ("checkout", order_id, r.status_code)
    except Exception as exc:  # noqa: BLE001
        return ("checkout", order_id, repr(exc))


def job_coupon_churn(args):
    url, order_id, flips, barrier = args
    barrier.wait()
    for i in range(flips):
        code = "SAVE20" if i % 2 == 0 else ""
        try:
            requests.post(f"{url}/orders/{order_id}/coupon", json={"code": code}, timeout=60)
        except Exception:  # noqa: BLE001
            pass
    return ("churn", order_id, flips)


def job_commit(args):
    url, reservation_id, barrier = args
    barrier.wait()
    try:
        r = requests.post(
            f"{url}/reservations/{reservation_id}/commit", json={}, timeout=120
        )
        return ("commit", reservation_id, r.status_code)
    except Exception as exc:  # noqa: BLE001
        return ("commit", reservation_id, repr(exc))


def job_expire(args):
    url, now_iso, barrier = args
    barrier.wait()
    try:
        r = requests.post(f"{url}/internal/expire", json={"now": now_iso}, timeout=180)
        return ("expire", r.status_code, "")
    except Exception as exc:  # noqa: BLE001
        return ("expire", None, repr(exc))


def job_reserve_one(args):
    url, sku, order_id, barrier = args
    barrier.wait()
    try:
        r = requests.post(
            f"{url}/reservations",
            json={"sku": sku, "qty": 1, "order_id": order_id},
            timeout=120,
        )
        return r.status_code
    except Exception as exc:  # noqa: BLE001
        return repr(exc)


def run_jobs(jobs):
    """Run (fn, args) pairs concurrently, all released by one barrier."""
    with mp.Manager() as manager:
        barrier = manager.Barrier(len(jobs))
        ctx = mp.get_context("spawn")
        with ctx.Pool(len(jobs)) as pool:
            handles = [pool.apply_async(fn, ((*args, barrier),)) for fn, args in jobs]
            return [h.get(timeout=300) for h in handles]
