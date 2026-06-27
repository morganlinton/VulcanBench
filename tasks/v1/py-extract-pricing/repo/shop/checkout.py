"""Order checkout. ``total`` currently inlines every pricing rule.

NOTE: the pricing rules below should be extracted into ``shop/pricing.py`` and
reused here without changing the result. See issue.md.
"""


def total(items, code, rate_bps):
    """Order total in integer cents.

    items: list of {"price_cents": int, "qty": int}
    code:  discount code (or None)
    rate_bps: tax rate in basis points (1% == 100 bps)
    """
    sub = 0
    for it in items:
        sub += it["price_cents"] * it["qty"]

    if code == "SAVE10":
        disc = sub * 10 // 100
    elif code == "HALF":
        disc = sub * 50 // 100
    elif code == "5OFF":
        disc = min(500, sub)
    else:
        disc = 0
    discounted = sub - disc

    tax_amt = discounted * rate_bps // 10000
    ship = 0 if discounted >= 5000 else 599
    return discounted + tax_amt + ship
