# Token-bucket rate limiter over-grants and races under concurrency

`bucket.TokenBucket` throttles an API client: it starts full, refills at a fixed
rate up to a fixed capacity, and `Allow()` consumes one token if available. It has
two bugs:

- **Refill is not clamped to capacity.** After an idle period, `refill()` adds
  `elapsed * rate` tokens with no upper bound, so the bucket can hold far more than
  `capacity` tokens and then grants a huge burst. The token count must never exceed
  `capacity`.
- **It is not safe for concurrent use.** `Allow`/`AllowN` read-modify-write the
  token count with no locking, so concurrent callers race and can hand out more
  tokens than exist. Access to the bucket's state must be serialized.

## Expected behavior

- A freshly constructed bucket holds `capacity` tokens.
- `Allow()` returns `true` and consumes a token when at least one is available,
  otherwise returns `false`.
- Tokens accrue at `rate` per second of elapsed time (read via the injected
  `now` clock), but the total never exceeds `capacity`.
- The bucket is safe to call from multiple goroutines at once: with a fixed clock
  and `capacity` tokens, exactly `capacity` concurrent `Allow()` calls may
  succeed — never more.

The tests inject a clock and run under `-race`, so a fix must both clamp the
refill and make concurrent access data-race free. Keep the `New` signature.
