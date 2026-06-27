"""Refactor the inline pricing rules in checkout.total into shop/pricing.py.

The four pure helpers must implement the exact rules currently inlined in
``shop.checkout.total`` (fail_to_pass — they are stubs today), and the end-to-end
total must keep returning the same numbers (pass_to_pass — already correct).
"""

import pytest

from shop import checkout
from shop.pricing import discount, shipping, subtotal, tax


def test_subtotal():
    items = [{"price_cents": 250, "qty": 2}, {"price_cents": 100, "qty": 3}]
    assert subtotal(items) == 800
    assert subtotal([]) == 0


def test_discount_codes():
    assert discount(1000, "SAVE10") == 100   # 10%
    assert discount(1000, "HALF") == 500     # 50%
    assert discount(1000, "5OFF") == 500     # flat 500 off
    assert discount(300, "5OFF") == 300      # flat discount cannot exceed subtotal
    assert discount(1000, None) == 0
    assert discount(1000, "BOGUS") == 0      # unknown code
    assert discount(999, "SAVE10") == 99     # floors (99.9 -> 99)


def test_tax_floors():
    assert tax(1000, 825) == 82      # 8.25% of 1000 = 82.5 -> 82
    assert tax(0, 825) == 0
    assert tax(5000, 0) == 0


def test_shipping_threshold():
    assert shipping(4999) == 599     # below threshold
    assert shipping(5000) == 0       # free at/above 5000
    assert shipping(10000) == 0


def test_total_end_to_end():
    # passes pre-refactor: checkout.total already computes these
    items = [{"price_cents": 2000, "qty": 1}]  # sub 2000
    # SAVE10 -> disc 200 -> discounted 1800; tax 8.25% -> 148; ship 599
    assert checkout.total(items, "SAVE10", 825) == 1800 + 148 + 599


def test_total_free_shipping_after_big_order():
    items = [{"price_cents": 3000, "qty": 2}]  # sub 6000
    # HALF -> disc 3000 -> discounted 3000; tax 0; ship 599 (3000 < 5000)
    assert checkout.total(items, "HALF", 0) == 3000 + 0 + 599
    # no code -> discounted 6000 -> free shipping, tax 0
    assert checkout.total(items, None, 0) == 6000
