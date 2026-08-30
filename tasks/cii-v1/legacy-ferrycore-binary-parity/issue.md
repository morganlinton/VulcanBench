# Replace the retired FerryCore controller; boarding order no longer reconciles

We are decommissioning the legacy FerryCore island car-ferry boarding
controller (a vendor binary whose source was lost when the harbor
authority's IT contractor was wound down in 2018) and replacing it with
`ferrycore.py`. The Python rewrite was done from `docs/SPEC.md` and looks
correct on quiet days, but replaying real boarding-day sessions against
it fails reconciliation batch after batch:

- Regulars that keep missing the boat climb the pecking order faster
  than the rewrite predicts: a car that has been left on the quay many
  times over the season boards ahead of vehicles the rewrite puts first,
  and the gap widens the longer the season runs. After a seasonal
  squaring the two agree again for a while, then drift apart again.
- Every so often a long-suffering vehicle cuts the whole line: the old
  controller loads it FIRST on some sailing, ahead of vehicles that by
  every reading of the spec outrank it, and only for that one sailing.
  The rewrite never does this, and once one of these jumps happens the
  two controllers disagree about who is waiting and who has crossed for
  the rest of the day.
- Seasonal squarings do not settle things the way the book says: after
  a `K`, the old controller's subsequent boarding orders shift in ways
  the rewrite does not reproduce, sometimes for several sailings in a
  row, and occasionally the two `KOK` counts themselves disagree.
- Vehicles with equal standing sometimes board in the order they lined
  up rather than the order they were registered; right after a
  squaring it flips back to registration order, then reverts once
  someone new joins the queue.
- On full boats the old controller squeezes one more small vehicle
  aboard than the rewrite thinks can fit, but only for certain
  season-long regulars; the deck reads over capacity by a unit on the
  rewrite's math and exactly at it by the dock crew's count.
- Sailings with an empty quay print a departure line on the old
  controller; the rewrite prints nothing at all, and the day-file
  parsers downstream lose their place.
- Vehicles registered late in the season start deeper in the pecking
  order on the old controller than the rewrite expects, as if they owe
  standing they never accrued.
- Queue rosters and the `X` trailer counts stop matching the moment any
  of the above happens, and a join the old controller accepts is
  sometimes bounced as `QUEUED` by the rewrite, or the other way
  around.

The spec's own header warns about this: the document has drifted, and
**the controller's behavior is the contract**, not the spec. The retired
binary is available in `legacy/` for reference while you work
(`legacy/run < dayfile.txt`, one day per process, state accumulates
within a session). It is NOT present in production, so the replacement
must reproduce its behavior, not invoke it.

Make `ferrycore.py` a drop-in behavioral replacement for the controller:
byte-for-byte identical reply lines (including boarding order, `GEND`
and `KOK` counts, error codes, and the trailer) for any command stream,
across vehicle registration, queueing, sailings, and seasonal squarings,
matching how the controller actually parses, validates, orders, boards,
squares, and counts, wherever that differs from what the spec says.
Where the spec IS accurate, nothing may change.
