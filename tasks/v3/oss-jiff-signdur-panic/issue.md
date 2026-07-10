# Panic converting a negative `SignedDuration` to `std::time::Duration`

Converting a `jiff::SignedDuration` into a `std::time::Duration` via
`TryFrom` should fail with an `Err` for any negative signed duration, since
`std::time::Duration` is unsigned. It does return `Err` when the whole-seconds
component is negative, but it **panics** when the seconds component is zero and
only the subsecond component is negative.

```rust
use std::time::Duration;
use jiff::SignedDuration;

// Expected: an Err. Actual (before the fix): panics.
let _ = Duration::try_from(SignedDuration::new(0, -1));
```

The internal conversion computes the unsigned seconds fallibly but then does
`u32::try_from(sd.subsec_nanos()).unwrap()`. When `as_secs()` is zero the
subsecond nanoseconds can still be negative, so the `unwrap()` panics instead
of surfacing an error.

## Expected behavior

`Duration::try_from(sd)` must return `Err` (never panic) for every negative
`SignedDuration`, including the zero-seconds / negative-subseconds case. The
subsecond conversion should map its failure to the same conversion error the
seconds path uses. Non-negative signed durations must continue to convert
successfully, and the reverse `SignedDuration::try_from(std::time::Duration)`
conversion is unchanged.

## Acceptance examples

- `Duration::try_from(SignedDuration::new(0, -1)).is_err()` is `true`
- `Duration::try_from(SignedDuration::new(0, -999_999_999)).is_err()` is `true`
- `Duration::try_from(SignedDuration::new(-5, 0)).is_err()` is `true`
- `Duration::try_from(SignedDuration::new(5, 123_000_000))` equals `Duration::new(5, 123_000_000)`
