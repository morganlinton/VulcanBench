"""Inventory service HTTP API."""

from __future__ import annotations

import datetime as dt

from orderflow.ofkit.http import Request, Router
from orderflow.inventory import store

router = Router("inventory")


@router.route("GET", "/healthz")
def healthz(_request: Request) -> tuple[int, dict]:
    return 200, {"ok": True, "service": "inventory"}


@router.route("GET", "/availability/{sku}")
def availability(request: Request) -> tuple[int, dict]:
    return 200, store.availability(request.params["sku"])


@router.route("POST", "/reservations")
def reserve(request: Request) -> tuple[int, dict]:
    sku, qty, order_id = request.require("sku", "qty", "order_id")
    return 201, store.reserve(str(sku), int(qty), str(order_id))


@router.route("GET", "/internal/reservations/{reservation_id}")
def get_reservation(request: Request) -> tuple[int, dict]:
    return 200, store.reservation(request.params["reservation_id"])


@router.route("GET", "/internal/reservations/by_order/{order_id}")
def by_order(request: Request) -> tuple[int, dict]:
    return 200, {"reservations": store.reservations_for_order(request.params["order_id"])}


@router.route("POST", "/reservations/{reservation_id}/commit")
def commit(request: Request) -> tuple[int, dict]:
    return 200, store.commit(request.params["reservation_id"])


@router.route("POST", "/reservations/{reservation_id}/release")
def release(request: Request) -> tuple[int, dict]:
    return 200, store.release(request.params["reservation_id"])


@router.route("POST", "/internal/products")
def seed_product(request: Request) -> tuple[int, dict]:
    sku, available = request.require("sku", "available")
    store.seed_product(str(sku), int(available))
    return 201, store.product(str(sku))


@router.route("GET", "/internal/products/{sku}")
def product(request: Request) -> tuple[int, dict]:
    return 200, store.product(request.params["sku"])


@router.route("POST", "/internal/restock")
def restock(request: Request) -> tuple[int, dict]:
    sku, qty = request.require("sku", "qty")
    return 200, store.restock(str(sku), int(qty))


@router.route("POST", "/internal/expire")
def expire(request: Request) -> tuple[int, dict]:
    now = None
    if "now" in request.body:
        now = dt.datetime.fromisoformat(str(request.body["now"]))
    return 200, store.expire_pass(now)
