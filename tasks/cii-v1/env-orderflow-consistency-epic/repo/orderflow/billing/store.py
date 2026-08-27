"""Billing persistence: charges are idempotent per caller-supplied key."""

from __future__ import annotations

import uuid

from orderflow.ofkit import pg
from orderflow.ofkit.config import load
from orderflow.ofkit.http import ApiError

DB = "billing_db"


def connect():
    return pg.connect(DB)


def create_charge(order_id: str, amount_cents: int, idem_key: str) -> dict:
    """Create (or replay) a charge. Same key -> same charge, exactly once.

    Settlement is immediate in this deployment (the acquirer sandbox settles
    synchronously); the settle event row is written in the same transaction
    so the worker can never observe a settled charge without its event.
    """
    if amount_cents <= 0:
        raise ApiError(422, "bad_amount", "amount_cents must be positive")
    charge_id = uuid.uuid4().hex
    settle = load().settle_immediately
    with connect() as conn, conn.cursor() as cur:
        cur.execute(
            "INSERT INTO charges (id, order_id, amount_cents, idem_key, status) "
            "VALUES (%s, %s, %s, %s, %s) "
            "ON CONFLICT (idem_key) DO NOTHING",
            (charge_id, order_id, amount_cents, idem_key, "SETTLED" if settle else "CREATED"),
        )
        created = cur.rowcount == 1
        if created and settle:
            cur.execute(
                "INSERT INTO settle_events (charge_id, order_id, amount_cents) "
                "VALUES (%s, %s, %s)",
                (charge_id, order_id, amount_cents),
            )
        cur.execute(
            "SELECT id, order_id, amount_cents, status FROM charges WHERE idem_key = %s",
            (idem_key,),
        )
        row = cur.fetchone()
    return {
        "charge_id": row[0],
        "order_id": row[1],
        "amount_cents": int(row[2]),
        "status": row[3],
        "replayed": not created,
    }


def charges_for_order(order_id: str) -> list[dict]:
    with connect() as conn, conn.cursor() as cur:
        cur.execute(
            "SELECT id, amount_cents, status FROM charges "
            "WHERE order_id = %s ORDER BY created_at",
            (order_id,),
        )
        rows = cur.fetchall()
    return [{"charge_id": r[0], "amount_cents": int(r[1]), "status": r[2]} for r in rows]
