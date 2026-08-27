"""The checkout orchestration.

One checkout call takes an order from PLACED to PAID with committed stock:
reserve, price, charge, record payment, commit the reservation. Charges are
idempotent at billing per key; the key binds the charge to what the customer
is paying for, so it is derived from the order and the amount being charged.
"""

from __future__ import annotations

import hashlib

from orderflow.ofkit import client, topology
from orderflow.ofkit.http import ApiError


def _orders(path: str) -> str:
    return topology.service_url("orders") + path


def _inventory(path: str) -> str:
    return topology.service_url("inventory") + path


def _billing(path: str) -> str:
    return topology.service_url("billing") + path


def charge_key(order_id: str, amount_cents: int) -> str:
    return hashlib.sha256(f"charge:{order_id}:{amount_cents}".encode()).hexdigest()


def run(order_id: str) -> dict:
    order = client.get(_orders(f"/orders/{order_id}"))
    if order["status"] == "PAID":
        return {"order_id": order_id, "status": "PAID", "charge_id": order["charge_id"]}
    if order["status"] != "PLACED":
        raise ApiError(409, "bad_state", f"cannot check out from {order['status']}")

    reservation = client.post(
        _inventory("/reservations"),
        {"sku": order["sku"], "qty": order["qty"], "order_id": order_id},
    )
    quote = client.get(_orders(f"/orders/{order_id}/quote"))
    amount = int(quote["total_cents"])
    charge = client.post(
        _billing("/charges"),
        {
            "order_id": order_id,
            "amount_cents": amount,
            "idem_key": charge_key(order_id, amount),
        },
    )
    client.post(
        _orders(f"/orders/{order_id}/paid"),
        {"charge_id": charge["charge_id"], "reservation_id": reservation["reservation_id"]},
    )
    client.post(_inventory(f"/reservations/{reservation['reservation_id']}/commit"))
    return {
        "order_id": order_id,
        "status": "PAID",
        "charge_id": charge["charge_id"],
        "amount_cents": amount,
        "reservation_id": reservation["reservation_id"],
    }
