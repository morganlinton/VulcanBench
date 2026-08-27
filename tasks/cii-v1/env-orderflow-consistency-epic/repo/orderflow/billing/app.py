"""Billing service HTTP API."""

from __future__ import annotations

from orderflow.ofkit.http import Request, Router
from orderflow.billing import store

router = Router("billing")


@router.route("GET", "/healthz")
def healthz(_request: Request) -> tuple[int, dict]:
    return 200, {"ok": True, "service": "billing"}


@router.route("POST", "/charges")
def create_charge(request: Request) -> tuple[int, dict]:
    order_id, amount_cents, idem_key = request.require("order_id", "amount_cents", "idem_key")
    result = store.create_charge(str(order_id), int(amount_cents), str(idem_key))
    return (200 if result["replayed"] else 201), result


@router.route("GET", "/internal/charges/by_order/{order_id}")
def by_order(request: Request) -> tuple[int, dict]:
    return 200, {"charges": store.charges_for_order(request.params["order_id"])}
