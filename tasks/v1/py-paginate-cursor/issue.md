# Cursor pagination repeats rows at page boundaries and never ends

The `pagination` package serves records in stable `(ts, id)` order, one page at a
time, using opaque cursors. Two bugs make it return wrong results, and they live
in different files:

- `pagination/cursor.py` — `is_after(record, token)` is meant to be a **strict**
  "does this record sort after the cursor position?" check, but it uses `>=`, so
  the record *at* the cursor is treated as being after it. That record then
  reappears as the first row of the next page.
- `pagination/repository.py` — `page(...)` always returns a `next_cursor` whenever
  the page is non-empty, even when there are no more records after it. A caller
  that paginates until `next_cursor is None` therefore never stops (and on the
  last record, loops forever).

## Expected behavior

- `Repository(records).page(after=None, limit=N)` returns the first `N` records in
  `(ts, id)` order.
- Passing the previous page's `next_cursor` as `after` returns the **next** `N`
  records — each record appears on exactly one page, with no repeats and no gaps.
- Records that share a `ts` are ordered (and cursored) by `id`, so ties never
  cause a row to be skipped or duplicated.
- `next_cursor` is `None` exactly when the page just returned is the last one, so
  callers can loop `while page.next_cursor is not None`.

Fix both files so a full walk visits every record once, in order, and terminates.
Cursors should stay opaque (do not change their encoded format).
