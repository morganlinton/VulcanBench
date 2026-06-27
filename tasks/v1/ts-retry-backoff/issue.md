# Retry helper drops the final attempt and lets backoff grow without bound

`retry(fn, options)` in `src/retry.ts` retries an async operation with exponential
backoff. It has two bugs:

- **Off-by-one in the attempt count.** It runs one fewer attempt than `attempts`,
  so an operation that would succeed on its final allowed attempt is reported as a
  failure. The operation must be attempted up to `attempts` times in total (the
  first call plus retries).
- **Backoff is not capped.** The delay before the k-th retry is
  `baseDelayMs * 2 ** (k - 1)` with no upper bound, so it grows unboundedly. Each
  delay must be capped at `maxDelayMs`.

## Expected behavior

- `fn` is attempted up to `attempts` times. The first call that resolves returns
  its value.
- Before the k-th retry (k = 1, 2, …), wait
  `min(baseDelayMs * 2 ** (k - 1), maxDelayMs)` milliseconds. There is no wait
  after the final failed attempt.
- If every attempt rejects, `retry` rejects with the **last** error.

`options.sleep` is an injectable delay (the tests pass a recorder that resolves
immediately and remembers the requested delays), so the backoff schedule is
observable without real timers. Keep the `retry` signature and the `RetryOptions`
shape.
