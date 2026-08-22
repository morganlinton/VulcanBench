# Offset checked subtraction adds instead of subtracting

`jiff-core`'s `tz::Offset::checked_sub` computes the wrong result: it adds
the given number of seconds to the offset instead of subtracting it.

```rust
use jiff_core::tz::Offset;

Offset::UTC.checked_sub(1).unwrap()
// returns +00:00:01 — should be -00:00:01

Offset::MIN.checked_sub(1)
// returns Ok(MIN + 1s) — should be a range error: there is nothing below MIN
```

Expected:

- `checked_sub(n)` subtracts: `UTC.checked_sub(1)` is one second below UTC,
  and `UTC.checked_sub(-1)` is one second above.
- The range check follows the subtraction: subtracting past `Offset::MIN`
  (or subtracting a negative past `Offset::MAX`) errors instead of quietly
  landing back inside the range. Take care with `i32::MIN` as the
  subtrahend — its negation does not fit in `i32`.
- `checked_add`, the accessors, and `until` are unchanged.
