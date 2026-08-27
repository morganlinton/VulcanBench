"""Orders service HTTP API."""

from __future__ import annotations

from orderflow.ofkit.http import Request, Router
from orderflow.orders import pricing, store

router = Router("orders")


@router.route("GET", "/healthz")
def healthz(_request: Request) -> tuple[int, dict]:
    return 200, {"ok": True, "service": "orders"}


@router.route("POST", "/orders")
def create(request: Request) -> tuple[int, dict]:
    order_id, sku, qty, unit_price_cents = request.require(
        "order_id", "sku", "qty", "unit_price_cents"
    )
    order = store.create_order(str(order_id), str(sku), int(qty), int(unit_price_cents))
    return 201, order


@router.route("GET", "/orders/{order_id}")
def get_order(request: Request) -> tuple[int, dict]:
    return 200, store.get_order(request.params["order_id"])


@router.route("POST", "/orders/{order_id}/coupon")
def set_coupon(request: Request) -> tuple[int, dict]:
    (code,) = request.require("code")
    store.set_coupon(request.params["order_id"], None if code == "" else str(code))
    return 200, pricing.quote(request.params["order_id"])


@router.route("GET", "/orders/{order_id}/quote")
def quote(request: Request) -> tuple[int, dict]:
    return 200, pricing.quote(request.params["order_id"])


@router.route("POST", "/orders/{order_id}/paid")
def paid(request: Request) -> tuple[int, dict]:
    charge_id, reservation_id = request.require("charge_id", "reservation_id")
    store.mark_paid(request.params["order_id"], str(charge_id), str(reservation_id))
    return 200, store.get_order(request.params["order_id"])


@router.route("POST", "/orders/{order_id}/cancel")
def cancel(request: Request) -> tuple[int, dict]:
    released = store.cancel(request.params["order_id"])
    return 200, released


@router.route("POST", "/internal/orders/{order_id}/complete")
def complete(request: Request) -> tuple[int, dict]:
    store.mark_complete(request.params["order_id"])
    return 200, store.get_order(request.params["order_id"])


@router.route("POST", "/internal/coupons")
def seed_coupon(request: Request) -> tuple[int, dict]:
    code, percent_off = request.require("code", "percent_off")
    store.seed_coupon(str(code), int(percent_off))
    return 201, {"code": code, "percent_off": percent_off}
