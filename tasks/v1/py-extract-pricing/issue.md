# Extract the inlined pricing rules out of `checkout.total`

`shop/checkout.py` computes an order `total` by inlining every pricing rule in one
function. Extract those rules into the four pure helpers in `shop/pricing.py` and
have `checkout.total` call them, **without changing any results**. All amounts are
integer cents and every rule floors (integer division).

Implement in `shop/pricing.py`:

- `subtotal(items)`: the sum of `price_cents * qty` over `items` (a list of
  `{"price_cents", "qty"}`), `0` for an empty list.
- `discount(subtotal_cents, code)`: the discount **amount** in cents —
  `"SAVE10"` is 10% of the subtotal, `"HALF"` is 50%, `"5OFF"` is a flat 500 cents
  but never more than the subtotal, and any other value (including `None`) is `0`.
  Percentage discounts floor (e.g. 10% of 999 is 99).
- `tax(amount_cents, rate_bps)`: `amount_cents * rate_bps` basis points, floored
  (e.g. 825 bps of 1000 is 82).
- `shipping(amount_cents)`: `0` if `amount_cents >= 5000`, otherwise `599`.

Then rewrite `checkout.total(items, code, rate_bps)` to compose them: take the
subtotal, subtract the discount, then add tax on the discounted amount and
shipping based on the discounted amount. The end-to-end totals must stay exactly
the same as before.
