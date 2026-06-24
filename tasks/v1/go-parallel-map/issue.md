# `pmap.Map` reorders results, races, and reports the wrong error

`pmap.Map(ctx, in, limit, f)` (`example.com/pmap`) should apply `f` to every
element of `in` using at most `limit` concurrent goroutines and return the
results. The current implementation collects results by appending from each
goroutine as it finishes, which is wrong in three ways:

1. **Order is not preserved.** Results come back in completion order, so
   `Map(ctx, []int{5,1,3,2,4}, 5, slow)` returns the values interleaved by which
   call finished first instead of in input order.
2. **It has a data race.** Multiple goroutines append to the same slice (and
   assign `firstErr`) without synchronization; `go test -race` reports a race.
3. **The wrong error is returned.** On failure it returns whichever error
   completed first, not the error belonging to the lowest input index.

Fix `Map` so that:

- results are returned in the **same order as `in`**;
- it is free of data races (the suite runs `go test -race`);
- at most `limit` calls to `f` run concurrently (a `limit` below 1 means 1);
- if one or more calls return an error, `Map` returns the error of the
  **lowest input index** and a nil slice; otherwise it returns the results and a
  nil error;
- the element's `ctx` is forwarded to `f`.

`Map` lives in `pmap/pmap.go`.
