"""Pure pricing helpers. To be extracted from ``shop/checkout.py``.

All amounts are integer cents; every rule floors (integer division). See issue.md
for the exact contract.
"""


def subtotal(items):
    """Sum of price_cents * qty over items."""
    raise NotImplementedError


def discount(subtotal_cents, code):
    """Discount amount in cents for the given code (0 if none/unknown)."""
    raise NotImplementedError


def tax(amount_cents, rate_bps):
    """Tax on amount_cents at rate_bps basis points, floored."""
    raise NotImplementedError


def shipping(amount_cents):
    """Shipping cost in cents for an order whose discounted amount is given."""
    raise NotImplementedError
