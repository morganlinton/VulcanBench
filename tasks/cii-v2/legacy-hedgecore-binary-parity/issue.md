# Replace the retired HedgeCore engine; marking reconciliation rejects the rewrite's sessions

We are decommissioning the HedgeCore FX position ledger (a compiled
artifact from the desk's original technology provider, wound down in
2015; source long gone) and replacing it with `hedgecore.py`. The
rewrite follows `docs/SPEC.md` and matches on simple sessions, but the
marking and reconciliation systems that consume HedgeCore net values are
flagging real trading files batch after batch:

- End-of-day marks are off by a cent: a revaluation run against the same
  rates the book already carries should print the same net value the
  last trade printed, and with the engine it sometimes does not, by
  exactly one cent, on scattered books, and the rewrite never reproduces
  it.
- Books that traded through flat are drifting: after wash trades that
  take a position through zero, the engine's net values walk away from
  the rewrite's by amounts that grow with the position and never
  reconcile again for that pair.
- Yen books revalue wildly when omitted from the fixing list: a `V` run
  that does not mention a yen pair produces engine net values orders of
  magnitude away from the rewrite on those books, while the same book
  revalued WITH the pair in the list matches the trade-time arithmetic.
- Sessions with certain no-op trades come out misaligned line for line:
  the engine's reply stream is shorter than the rewrite's for the same
  input, and the trailers disagree, so the reconciler cannot even pair
  up the replies.
- Desks that key the same book id with different capitalization get one
  book from the engine and several from the rewrite, and every
  revaluation after that prints a different number of lines on the two
  sides.

The spec's own header warns about this: the document has drifted, and
**the engine's behavior is the contract**, not the spec. The retired
binary is available in `legacy/` for reference while you work
(`legacy/run < session.txt`, one session per process), but it is NOT
present in production, so the replacement must reproduce its behavior,
not invoke it.

Make `hedgecore.py` a drop-in behavioral replacement for the engine:
byte-for-byte identical reply lines (including net values, reject codes,
reply order, and the trailer) for any command stream, across trades,
revaluations, and rejects, matching how the engine actually parses,
validates, books, revalues, rounds, and counts, wherever that differs
from what the spec says. Where the spec IS accurate, nothing may change.
