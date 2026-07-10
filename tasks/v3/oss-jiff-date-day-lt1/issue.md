# `Date::new` accepts a day of 0 or a negative day

`jiff::civil::Date::new(year, month, day)` should return an `Err` for any day
outside the valid 1..=days_in_month range. It correctly rejects days that are
too large for the month, but it accepts a `day` of `0` or a negative `day`:

```rust
use jiff::civil::Date;

// Expected: Err. Actual (before the fix): Ok(<invalid date>).
let _ = Date::new(1998, 1, 0);
let _ = Date::new(1998, 1, -1);
```

The constructor only guards the upper bound (`day > days_in_month`), so any
`day < 1` slips through and produces an invalid `Date`.

## Expected behavior

`Date::new` must return `Err` when `day < 1`, in addition to the existing
rejection of days greater than the month length. Valid dates (including the
leap-day boundary, e.g. `2004-02-29`) must continue to succeed, and days that
exceed the month length (e.g. `1998-02-29`) must continue to be rejected.

## Acceptance examples

- `Date::new(1998, 1, 0).is_err()` is `true`
- `Date::new(1998, 1, -1).is_err()` is `true`
- `Date::new(2004, 2, 29).is_ok()` is `true`
- `Date::new(1998, 2, 29).is_err()` is `true`
