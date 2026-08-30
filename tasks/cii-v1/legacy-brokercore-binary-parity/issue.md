# Replace the retired BrokerCore engine; bookings and settlements no longer agree

We are decommissioning the legacy BrokerCore load-board controller (a
vendor binary whose source was lost when the vendor folded in 2017) and
replacing it with `brokercore.py`. The Python rewrite was done from
`docs/SPEC.md` and looks correct on the happy path, but the dispatcher
terminals and the settlement clerks' batch tools, all built against the
engine's actual behavior over many years, are flagging discrepancies
week after week:

- Busy carriers' scores fall behind: as a carrier stacks up open
  bookings, each new booking from the engine credits a little less than
  the tariff arithmetic says, and the replacement's `A` lines drift
  above the engine's; dropping loads narrows the gap again. No single
  reply is off by an amount anyone can name from the spec.
- Weekly statements move scores in BOTH directions: for busy carriers
  the engine's `S` lines come back HIGHER than the live score it was
  just reporting, for carriers who walked away from loads they come
  back LOWER than the fill-minus-forfeit arithmetic, and either way the
  statement figure becomes the score the next booking builds on. The
  replacement's settlement is a flat identity and never reproduces
  either move.
- A few top-rated carriers book certain loads to better scores than
  `value / 1000` can explain, and the weekly statement quietly walks
  the excess back; the replacement never shows the excess at all.
- Carriers who have walked away twice see their next bookings credit
  roughly half of what the same loads pay anyone else; after they drop
  yet again it goes back to normal. The statements never show any of
  it.
- Dispatchers cannot rebook a carrier onto a load it dropped: the
  engine keeps answering `TAKEN` even though the board shows the load
  open and other carriers book it fine; the replacement happily books
  it, and every count after that disagrees.
- Quiet weeks break the trailers: on an empty board the engine still
  prints the settlement terminator and counts the run, the replacement
  prints nothing, and carriers signed after those quiet weeks start
  visibly below `rating * 100` on the engine, lower the later they
  sign; the replacement starts everyone at `rating * 100`. Batch
  trailers (`X` counts) no longer reconcile with the reject logs either
  way.

The spec's own header warns about this: the document has drifted, and
**the engine's behavior is the contract**, not the spec. The retired
binary is available in `legacy/` for reference while you work
(`legacy/run < commands.txt`), but it is NOT present in production, so
the replacement must reproduce its behavior, not invoke it.

Make `brokercore.py` a drop-in behavioral replacement for the engine:
byte-for-byte identical reply lines (including scores, `S` statements,
counts, error codes, and the trailer) for any command stream, across
registrations, load postings, live bookings, drops, and weekly
settlements, matching how the engine actually parses, validates,
scores, settles, and counts, wherever that differs from what the spec
says. Where the spec IS accurate, nothing may change.
