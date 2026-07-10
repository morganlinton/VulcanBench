# strftime pads negative values one character too wide

When `strftime` formats a negative integer field (such as a negative year), the
minus sign should count toward the requested field width. Instead, jiff writes
the `-` first and then zero-pads the digits to the full width, producing output
one character wider than requested:

```rust
use jiff::civil::Date;

// %Y pads to width 4. Expected "-024"; before the fix this yields "-0024".
let s = Date::new(-24, 7, 14).unwrap().strftime("%Y").to_string();

// %06Y pads to width 6. Expected "-02025"; before the fix "-002025".
let s = Date::new(-2025, 1, 13).unwrap().strftime("%06Y").to_string();
```

## Expected behavior

For negative values with a non-zero pad width, the sign is part of the total
width. With zero padding, `%06Y` on year -2025 is `"-02025"` (6 chars total).
With space padding (`%_6Y`), the spaces come before the sign, e.g. `"   -25"`.
Positive values are unaffected. This matches the C `strftime`/`printf`
convention where the sign occupies one of the padded columns.

## Acceptance examples

- `strftime("%Y")` on `-24-07-14` is `"-024"` (not `"-0024"`)
- `strftime("%G")` on `-19-11-30` is `"-019"` (not `"-0019"`)
- `strftime("%06Y")` on `-2025-01-13` is `"-02025"` (not `"-002025"`)
- `strftime("%Y")` on `2024-07-14` is `"2024"` (positive unaffected)
- `strftime("%06Y")` on `2025-01-13` is `"002025"` (positive unaffected)
