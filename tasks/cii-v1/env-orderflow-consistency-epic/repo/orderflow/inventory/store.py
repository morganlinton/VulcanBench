"""Inventory persistence: products, reservations, expiry.

Stock model: ``available`` is the physical on-shelf count; ``reserved`` is
held by ACTIVE reservations. Sellable = available - reserved. A commit ships
the goods (available and reserved both drop); a release returns the hold
(reserved drops). Expiry releases every overdue ACTIVE reservation.
"""

from __future__ import annotations

import datetime as dt
import uuid

from orderflow.ofkit import pg
from orderflow.ofkit.config import load
from orderflow.ofkit.http import ApiError
from orderflow.inventory import cache

DB = "inventory_db"


def connect():
    return pg.connect(DB)


def seed_product(sku: str, available: int) -> None:
    with connect() as conn, conn.cursor() as cur:
        cur.execute(
            "INSERT INTO products (sku, available, reserved) VALUES (%s, %s, 0) "
            "ON CONFLICT (sku) DO UPDATE SET available = EXCLUDED.available, reserved = 0",
            (sku, available),
        )
    cache.set_sellable(sku, available)


def product(sku: str) -> dict:
    with connect() as conn, conn.cursor() as cur:
        cur.execute("SELECT sku, available, reserved FROM products WHERE sku = %s", (sku,))
        row = cur.fetchone()
    if row is None:
        raise ApiError(404, "sku_not_found", f"no product {sku}")
    return {"sku": row[0], "available": int(row[1]), "reserved": int(row[2])}


def availability(sku: str) -> dict:
    cached = cache.get_sellable(sku)
    if cached is not None:
        return {"sku": sku, "sellable": cached, "source": "cache"}
    info = product(sku)
    sellable = info["available"] - info["reserved"]
    cache.set_sellable(sku, sellable)
    return {"sku": sku, "sellable": sellable, "source": "db"}


def reserve(sku: str, qty: int, order_id: str) -> dict:
    if qty <= 0:
        raise ApiError(422, "bad_qty", "qty must be positive")
    cache.note_traffic(sku)
    cached = cache.get_sellable(sku)
    if cached is not None and cached < qty:
        raise ApiError(409, "insufficient_stock", f"{sku}: {cached} sellable")

    reservation_id = uuid.uuid4().hex
    expires_at = dt.datetime.now(dt.timezone.utc) + dt.timedelta(
        seconds=load().reservation_ttl_s
    )
    with connect() as conn, conn.cursor() as cur:
        cur.execute("SELECT available, reserved FROM products WHERE sku = %s", (sku,))
        row = cur.fetchone()
        if row is None:
            raise ApiError(404, "sku_not_found", f"no product {sku}")
        available, reserved = int(row[0]), int(row[1])
        sellable = available - reserved
        if sellable < qty:
            raise ApiError(409, "insufficient_stock", f"{sku}: {sellable} sellable")
        cur.execute(
            "UPDATE products SET reserved = %s WHERE sku = %s",
            (reserved + qty, sku),
        )
        cur.execute(
            "INSERT INTO reservations (id, sku, order_id, qty, status, expires_at) "
            "VALUES (%s, %s, %s, %s, 'ACTIVE', %s)",
            (reservation_id, sku, order_id, qty, expires_at),
        )

    if cache.is_hot(sku):
        # Hot SKUs are refreshed in bulk by the worker's hot sync; skipping the
        # per-request write avoids a Redis stampede at flash-sale volume.
        pass
    else:
        cache.set_sellable(sku, sellable - qty)
    return {"reservation_id": reservation_id, "sku": sku, "qty": qty, "order_id": order_id}


def reservation(reservation_id: str) -> dict:
    with connect() as conn, conn.cursor() as cur:
        cur.execute(
            "SELECT id, sku, order_id, qty, status, expires_at "
            "FROM reservations WHERE id = %s",
            (reservation_id,),
        )
        row = cur.fetchone()
    if row is None:
        raise ApiError(404, "reservation_not_found", f"no reservation {reservation_id}")
    return {
        "id": row[0],
        "sku": row[1],
        "order_id": row[2],
        "qty": int(row[3]),
        "status": row[4],
        "expires_at": row[5].isoformat(),
    }


def reservations_for_order(order_id: str) -> list[dict]:
    with connect() as conn, conn.cursor() as cur:
        cur.execute(
            "SELECT id, sku, qty, status FROM reservations "
            "WHERE order_id = %s ORDER BY created_at",
            (order_id,),
        )
        rows = cur.fetchall()
    return [
        {"id": r[0], "sku": r[1], "qty": int(r[2]), "status": r[3]} for r in rows
    ]


def commit(reservation_id: str) -> dict:
    entry = reservation(reservation_id)
    with connect() as conn, conn.cursor() as cur:
        cur.execute(
            "UPDATE reservations SET status = 'COMMITTED' WHERE id = %s",
            (reservation_id,),
        )
        cur.execute(
            "UPDATE products SET available = available - %s, reserved = reserved - %s "
            "WHERE sku = %s",
            (entry["qty"], entry["qty"], entry["sku"]),
        )
    return {"reservation_id": reservation_id, "status": "COMMITTED"}


def release(reservation_id: str) -> dict:
    entry = reservation(reservation_id)
    with connect() as conn, conn.cursor() as cur:
        cur.execute(
            "UPDATE reservations SET status = 'RELEASED' WHERE id = %s",
            (reservation_id,),
        )
        cur.execute(
            "UPDATE products SET reserved = reserved - %s WHERE sku = %s",
            (entry["qty"], entry["sku"]),
        )
    info = product(entry["sku"])
    cache.refresh_after_return(entry["sku"], info["available"] - info["reserved"])
    return {"reservation_id": reservation_id, "status": "RELEASED"}


def restock(sku: str, qty: int) -> dict:
    if qty <= 0:
        raise ApiError(422, "bad_qty", "qty must be positive")
    with connect() as conn, conn.cursor() as cur:
        cur.execute(
            "UPDATE products SET available = available + %s WHERE sku = %s",
            (qty, sku),
        )
        if cur.rowcount == 0:
            raise ApiError(404, "sku_not_found", f"no product {sku}")
    info = product(sku)
    cache.refresh_after_return(sku, info["available"] - info["reserved"])
    return info


def expire_pass(now: dt.datetime | None = None) -> dict:
    """Release every overdue ACTIVE reservation. Invoked by the ops scheduler."""
    cutoff = now or dt.datetime.now(dt.timezone.utc)
    with connect() as conn, conn.cursor() as cur:
        cur.execute(
            "SELECT id FROM reservations WHERE status = 'ACTIVE' AND expires_at <= %s",
            (cutoff,),
        )
        overdue = [row[0] for row in cur.fetchall()]
    released = 0
    for reservation_id in overdue:
        release(reservation_id)
        released += 1
    return {"examined": len(overdue), "released": released}
