# Replace the retired ChoirCore engine; concert seatings no longer match

We are decommissioning the legacy ChoirCore seating engine (a compiled
artifact from the old parish IT contractor, source lost in 2018) and
replacing it with `choircore.py`. The rewrite follows `docs/SPEC.md`
and matches on simple seasons, but replaying full season logs against
it keeps producing different concert seatings, and the choir
administrators trust the printed rotas from the old engine:

- Regulars who sing week after week come out seated higher in the old
  engine than their attendance record accounts for, and a single
  missed week puts them back in line with it. The rewrite seats them
  strictly by the recorded arithmetic, so whole blocks of the seating
  come out shifted.
- Singers with a lot of recorded absences stop climbing the bench the
  way the arithmetic says they should: their rehearsals seem to count
  for less, for a while, and then count fully again after a concert
  goes well for them. The rewrite pays every rehearsal the same.
- Singers on level standing come out in a different order than the
  enrollment book says, but not always: sometimes the book order
  holds, sometimes it does not, and right after a reseat audit the old
  engine agrees with the book again for a stretch.
- The reseat audit itself is the strangest part. Its `WOK` count
  agrees with the rewrite every single time, yet the seatings AFTER an
  audit can differ from the seatings before it in the old engine, and
  once a season has a couple of concerts in it, who "carried" which
  concert stops matching the rewrite entirely, and every later seating
  in the season drifts further off.
- A concert called with nobody enrolled prints a bare `CEND 0` line in
  the old engine; the rewrite prints nothing at all for it.
- Newcomers who enroll mid-season sometimes start partway up the bench
  in the old engine instead of at the bottom, and a reseat audit drops
  them back down. The rewrite always starts them at the bottom.

The reject lanes and the `X` trailer counts reconcile exactly in every
log we have replayed; the drift is all in the seating order and the
shape of the concert block.

The spec's own header warns about this: the document has drifted, and
**the engine's behavior is the contract**, because the rota boards and
seating printers were built against the engine. The retired binary is
available in `legacy/` for reference while you work
(`legacy/run < season.txt`, one season per process). It is NOT present
in production, so the replacement must reproduce its behavior, not
invoke it.

Make `choircore.py` a drop-in behavioral replacement for the engine:
byte-for-byte identical reply lines (echoes, seating order, `CEND` and
`WOK` counts, error codes, and the trailer) for any command stream,
across enrollment, rehearsals, absences, concerts, and reseat audits,
matching how the engine actually parses, validates, accrues, seats,
resets, audits, and counts, wherever that differs from what the spec
says. Where the spec IS accurate, nothing may change.
