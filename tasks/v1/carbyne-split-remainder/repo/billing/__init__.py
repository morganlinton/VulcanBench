"""Billing helpers. All amounts are integer cents."""


def with_tax(amount_cents, rate_bps):
    return amount_cents + amount_cents * rate_bps // 10000
