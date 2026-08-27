"""Orders persistence: the orders table is the ledger of record."""

from __future__ import annotations

from orderflow.ofkit import pg
from orderflow.ofkit.http import ApiError

DB = "orders_db"


def connect():
    return pg.connect(DB)


def create_order(order_id: str, sku: str, qty: int, unit_price_cents: int) -> dict:
    with connect() as conn, conn.cursor() as cur:
        cur.execute(
            "INSERT INTO orders (id, sku, qty, unit_price_cents) "
            "VALUES (%s, %s, %s, %s) "
            "ON CONFLICT (id) DO NOTHING",
            (order_id, sku, qty, unit_price_cents),
        )
        if cur.rowcount == 0:
            raise ApiError(409, "order_exists", f"order {order_id} already exists")
    return get_order(order_id)


def get_order(order_id: str) -> dict:
    with connect() as conn, conn.cursor() as cur:
        cur.execute(
            "SELECT id, sku, qty, unit_price_cents, coupon_code, status, "
            "       charge_id, reservation_id "
            "FROM orders WHERE id = %s",
            (order_id,),
        )
        row = cur.fetchone()
    if row is None:
        raise ApiError(404, "order_not_found", f"no order {order_id}")
    keys = (
        "id",
        "sku",
        "qty",
        "unit_price_cents",
        "coupon_code",
        "status",
        "charge_id",
        "reservation_id",
    )
    return dict(zip(keys, row))


def set_coupon(order_id: str, code: str | None) -> None:
    with connect() as conn, conn.cursor() as cur:
        if code is not None:
            cur.execute(
                "SELECT active FROM coupons WHERE code = %s",
                (code,),
            )
            row = cur.fetchone()
            if row is None or not row[0]:
                raise ApiError(422, "bad_coupon", f"coupon {code} is not active")
        cur.execute(
            "UPDATE orders SET coupon_code = %s, updated_at = now() "
            "WHERE id = %s AND status = 'PLACED'",
            (code, order_id),
        )
        if cur.rowcount == 0:
            get_order(order_id)  # 404 if missing
            raise ApiError(409, "not_editable", "coupon changes require status PLACED")


def coupon_percent(code: str | None) -> int:
    if code is None:
        return 0
    with connect() as conn, conn.cursor() as cur:
        cur.execute("SELECT percent_off FROM coupons WHERE code = %s AND active", (code,))
        row = cur.fetchone()
    return int(row[0]) if row else 0


def mark_paid(order_id: str, charge_id: str, reservation_id: str) -> None:
    with connect() as conn, conn.cursor() as cur:
        cur.execute(
            "UPDATE orders SET status = 'PAID', charge_id = %s, reservation_id = %s, "
            "updated_at = now() WHERE id = %s AND status = 'PLACED'",
            (charge_id, reservation_id, order_id),
        )
        if cur.rowcount == 0:
            current = get_order(order_id)
            if current["status"] == "PAID" and current["charge_id"] == charge_id:
                return  # idempotent replay of the same payment
            raise ApiError(409, "bad_transition", f"cannot pay from {current['status']}")


def mark_complete(order_id: str) -> None:
    with connect() as conn, conn.cursor() as cur:
        cur.execute(
            "UPDATE orders SET status = 'COMPLETE', updated_at = now() "
            "WHERE id = %s AND status IN ('PAID', 'COMPLETE')",
            (order_id,),
        )
        if cur.rowcount == 0:
            current = get_order(order_id)
            raise ApiError(409, "bad_transition", f"cannot complete from {current['status']}")


def cancel(order_id: str) -> dict:
    with connect() as conn, conn.cursor() as cur:
        cur.execute(
            "UPDATE orders SET status = 'CANCELLED', updated_at = now() "
            "WHERE id = %s AND status = 'PLACED' RETURNING reservation_id",
            (order_id,),
        )
        row = cur.fetchone()
        if row is None:
            current = get_order(order_id)
            raise ApiError(409, "bad_transition", f"cannot cancel from {current['status']}")
    return {"released_reservation": row[0]}


def seed_coupon(code: str, percent_off: int) -> None:
    with connect() as conn, conn.cursor() as cur:
        cur.execute(
            "INSERT INTO coupons (code, percent_off) VALUES (%s, %s) "
            "ON CONFLICT (code) DO UPDATE SET percent_off = EXCLUDED.percent_off, active = true",
            (code, percent_off),
        )
