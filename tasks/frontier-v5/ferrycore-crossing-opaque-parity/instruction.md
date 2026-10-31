# Replace the retired FerryCore engine; the slip boards reject the rewrite's sessions

We are decommissioning the FerryCore crossing-log engine, a compiled
artifact from the ferry company's 2016 server retirement whose source
is long gone, and replacing it with `ferry/ferrycore.py` in `/app`. The
rewrite follows `docs/SPEC.md` and agrees with the engine on simple
days, but replaying real crossing days against it fails reconciliation
all over the place:

- Lines come out in the wrong order. While nobody has been passed over
  the two agree, but once cars have been left on the slip by a sailing
  the engine's lines stop being first come, first served, and after a
  toll audit they reshuffle again into an order the rewrite never
  produces. A car that drives off and comes back sometimes gets its old
  place, sometimes not.
- Sailings load different cars. On busy slips the engine sails with
  room still on deck when the rewrite would have squeezed in another
  car, and the cars it does take are not the ones the spec's walk
  would pick.
- The ferry docks at the wrong slip. On routes with three or more slips
  the engine does not always go to the next slip, and sometimes it does
  not leave at all.
- Cars that have already crossed today stand differently in a line from
  cars that have not.
- The night count lists waiting cars and parked cars differently from
  the spec, and the night itself seems to change how the next day's
  lines sort.
- The toll audit lists cars with equal crossings in a different order.
- The trailer's join and move counts disagree with the commands we
  sent, and the leave count is sometimes higher than the number of
  `X` commands. A heavy car that kept missing the boat on one slip
  vanished from its line on its own.
- A few cars that left their lines repeatedly sank to the very back of
  every line they joined much later, after several nights; and cars
  shuffled between slips several times sometimes lose ground right
  after an audit, depending on where the ferry happens to lie.

The spec's own header says it: the document has drifted, and **the
engine's behavior is the contract**, because everything downstream was
built against the engine. The retired binary is in `legacy/` for
reference while you work (`legacy/run < session.txt`, one session per
process, state accumulates within a session). It is NOT present in
production, so the replacement must reproduce its behavior, not invoke
it.

Make `ferry/ferrycore.py` a drop-in behavioral replacement: for any
session, byte-for-byte identical result lines and trailer, matching how
the engine actually validates, orders, loads, docks, audits, counts and
reports, wherever that differs from the spec. Where the spec IS
accurate, nothing may change. Production deploys the `ferry/` directory
and runs `python3 ferrycore.py < session.txt` from inside it; nothing
outside `ferry/` is deployed.
