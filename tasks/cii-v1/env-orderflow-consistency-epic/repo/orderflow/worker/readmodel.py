"""Settle-event processing and the order read model.

The worker turns billing's settle events into order completions and keeps
``order_read_model`` current for support tooling. Reservation status lookups
go through a Redis cache: inventory sits on the flash-sale hot path and one
HTTP round trip per event does not scale.
"""

from __future__ import annotations

from orderflow.ofkit import client, pg, topology
from orderflow.ofkit.config import load
from orderflow.ofkit.resp import Redis


def _status_key(order_id: str) -> str:
    return f"resstate:{order_id}"


def reservation_state(order_id: str) -> str:
    """The order's current reservation status, cached.

    ``NONE`` when the order never reserved stock; otherwise the status of
    the order's most recent reservation.
    """
    with Redis() as redis:
        cached = redis.get(_status_key(order_id))
        if cached is not None:
            return cached
    payload = client.get(
        topology.service_url("inventory") + f"/internal/reservations/by_order/{order_id}"
    )
    entries = payload["reservations"]
    state = entries[-1]["status"] if entries else "NONE"
    with Redis() as redis:
        redis.set(_status_key(order_id), state, ttl_s=load().status_cache_ttl_s)
    return state


def _pending_events(limit: int) -> list[dict]:
    with pg.connect("billing_db") as conn, conn.cursor() as cur:
        cur.execute(
            "SELECT id, charge_id, order_id, amount_cents FROM settle_events "
            "WHERE NOT processed ORDER BY id LIMIT %s",
            (limit,),
        )
        rows = cur.fetchall()
    return [
        {"id": r[0], "charge_id": r[1], "order_id": r[2], "amount_cents": int(r[3])}
        for r in rows
    ]


def _order_row(order_id: str) -> dict | None:
    with pg.connect("orders_db") as conn, conn.cursor() as cur:
        cur.execute("SELECT id, status FROM orders WHERE id = %s", (order_id,))
        row = cur.fetchone()
    return None if row is None else {"id": row[0], "status": row[1]}


def _upsert_read_model(
    order_id: str, status: str, charge_id: str, reservation_state_value: str
) -> None:
    with pg.connect("orders_db") as conn, conn.cursor() as cur:
        cur.execute(
            "INSERT INTO order_read_model (order_id, status, charge_id, reservation_state) "
            "VALUES (%s, %s, %s, %s) "
            "ON CONFLICT (order_id) DO UPDATE SET status = EXCLUDED.status, "
            "  charge_id = EXCLUDED.charge_id, "
            "  reservation_state = EXCLUDED.reservation_state, updated_at = now()",
            (order_id, status, charge_id, reservation_state_value),
        )


def _mark_processed(event_id: int) -> None:
    with pg.connect("billing_db") as conn, conn.cursor() as cur:
        cur.execute("UPDATE settle_events SET processed = true WHERE id = %s", (event_id,))


def tick() -> dict:
    """Process one batch of settle events. Returns counters for observability."""
    processed = completed = 0
    for event in _pending_events(load().poll_batch):
        order = _order_row(event["order_id"])
        if order is not None:
            state = reservation_state(event["order_id"])
            # Payment is the source of truth for fulfillment: once the charge
            # settled, this order is complete from the customer's perspective.
            _upsert_read_model(event["order_id"], "COMPLETE", event["charge_id"], state)
            if order["status"] == "PAID" and state == "COMMITTED":
                client.post(
                    topology.service_url("orders")
                    + f"/internal/orders/{event['order_id']}/complete"
                )
                completed += 1
        _mark_processed(event["id"])
        processed += 1
    return {"processed": processed, "completed": completed}
