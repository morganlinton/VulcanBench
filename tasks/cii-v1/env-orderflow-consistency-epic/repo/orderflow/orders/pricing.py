"""Order pricing.

A quote is always computed from the order's current state: current unit
price, current quantity, and whatever coupon is attached right now. Coupons
apply a whole-percent discount, rounded down to the cent.
"""

from __future__ import annotations

from orderflow.orders import store


def quote(order_id: str) -> dict:
    order = store.get_order(order_id)
    percent = store.coupon_percent(order["coupon_code"])
    gross = order["qty"] * order["unit_price_cents"]
    total = gross * (100 - percent) // 100
    return {
        "order_id": order_id,
        "sku": order["sku"],
        "qty": order["qty"],
        "gross_cents": gross,
        "percent_off": percent,
        "total_cents": total,
    }
