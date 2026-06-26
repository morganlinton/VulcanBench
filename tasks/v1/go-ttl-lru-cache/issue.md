# Implement a thread-safe LRU cache with per-entry TTL

`cachettl.New(capacity, ttl)` returns a cache (`example.com/cachettl`) that is
currently an unimplemented skeleton. Implement `Get`, `Put`, and `Len`:

- **Capacity / LRU.** The cache holds at most `capacity` entries. When a `Put`
  of a new key would exceed capacity, evict the **least recently used** entry.
- **Recency.** Both a successful `Get` and a `Put` (insert or update) make that
  key the most recently used.
- **TTL.** An entry expires `ttl` after it was last written (the last `Put` for
  that key). An expired entry is a miss: `Get` returns `(0, false)` and drops it,
  and `Len` counts only live (non-expired) entries.
- **Thread-safety.** `Get`, `Put`, and `Len` may be called concurrently from
  many goroutines. The implementation must be free of data races; the test suite
  runs under `go test -race`.

`Cache` and its methods live in `cachettl/cache.go`. `container/list` is a
convenient building block for the recency ordering.
