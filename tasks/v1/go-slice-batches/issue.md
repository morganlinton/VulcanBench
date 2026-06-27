# Batches share a backing array and corrupt each other on append

`batch.Batches(items, size)` splits a slice into consecutive batches of at most
`size` elements. It returns the right values, but each batch is a sub-slice
(`items[i:end]`) of the input's backing array. Because those sub-slices keep spare
capacity, appending to one batch writes into the next batch's storage — and into
the caller's original slice.

```go
b := batch.Batches([]int{1, 2, 3, 4}, 2) // [[1 2] [3 4]]
_ = append(b[0], 99)                      // overwrites b[1][0]; now b[1] == [99 4]
```

## Expected behavior

- `Batches` groups items into consecutive batches of at most `size` (a `size`
  below 1 is treated as 1) — the grouping and values are already correct.
- Each batch must be **independent**: appending to or growing one batch must not
  change any other batch or the input slice. In particular a returned batch must
  not retain spare capacity over shared storage.

Fix `Batches` so each batch owns its own storage, while keeping the same grouping
and the `size < 1` handling.
