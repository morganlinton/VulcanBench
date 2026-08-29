# Replace the retired BlendCore engine; dispensing and reconciles no longer agree

We are decommissioning the legacy BlendCore ink-blending controller (a
vendor binary whose source was lost when the vendor folded in 2018) and
replacing it with `blendcore.py`. The Python rewrite was done from
`docs/SPEC.md` and looks correct on the happy path, but the shop-floor
dispensers and batch reconcilers, all built against the engine's actual
behavior over many years, are flagging discrepancies batch after batch:

- Tanks run dry ahead of the book: dispenses that the ledger says should
  fit come back `N ... DRY` from the engine (or land short), while the
  replacement grants them in full; the gap is worst on tanks doing large
  pours, and a reconcile in between makes it disappear for a while.
- Reconciles move volumes in BOTH directions: after some batches the
  engine's `B` lines come back higher than what the dispensers could
  actually draw, and after heavier batches they come back LOWER than the
  fill-minus-dispense arithmetic, with the shortfall becoming the tank's
  new baseline; the replacement's reconcile is a flat identity and never
  reproduces either move.
- Bulk white-base jobs drain tanks faster: for some pigments, big pours
  leave measurably less in the tank than the same pours of other stock,
  but only above a certain size, and no `J` total ever shows the
  difference.
- Near-empty tanks give short pours the ledger never shows: jobs
  occasionally get credited slightly less than they asked for right
  before a tank empties, where the replacement rejects the request
  outright; the reconcilers see `J` totals from the engine that no
  granted-in-full model can explain.
- Feeds that always imported cleanly now throw rejects: some upstream
  command streams that the engine accepted for years get `N ... FMT`
  lines from the replacement, a few refills that the engine folded into
  an existing tank now open a second tank (splitting one tank's ledger in
  two and changing every count after it), and very large refills read
  back differently between the two sides. Batch trailers (`X` counts) no
  longer reconcile with the reject logs either way.

The spec's own header warns about this: the document has drifted, and
**the engine's behavior is the contract**, not the spec. The retired
binary is available in `legacy/` for reference while you work
(`legacy/run < commands.txt`), but it is NOT present in production, so
the replacement must reproduce its behavior, not invoke it.

Make `blendcore.py` a drop-in behavioral replacement for the engine:
byte-for-byte identical reply lines (including grants, job totals, `B`
lines, counts, error codes, and the trailer) for any command stream,
across fills, live dispenses, and batch reconciles, matching how the
engine actually parses, validates, dispenses, reconciles, and counts,
wherever that differs from what the spec says. Where the spec IS
accurate, nothing may change.
