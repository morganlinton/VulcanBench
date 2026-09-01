# Replace the retired PaddockCore engine; rota reconciliation rejects the rewrite's sessions

We are decommissioning the PaddockCore field co-op engine (a compiled
artifact from the stable office's 2013 machine retirement; source long
gone) and replacing it with `paddockcore.py`. The rewrite follows
`docs/SPEC.md` and matches on simple sessions, but replaying real yard
sessions against it fails reconciliation all over the place:

- Turnout listings come out in the wrong order. When every pony out has
  been handled the same way the two agree, but once ponies cycle in and
  out unevenly the engine's listing stops following build, and after a
  shift it snaps to yet another order, not back to build, and the
  disagreement then grows shift over shift. A pony led from field to
  field lists lower right afterward and then jumps higher once a shift
  has run.
- The rota rests the wrong field. The rewrite walks the stake rotation;
  the engine seems to pick whichever field has gone longest since
  anyone touched it, and after a field reopens the two never agree
  again.
- A field rested with ponies in it comes back one shift sooner in the
  engine than the spec says, and from then on the shift's field listing
  stops following stake order.
- Rest-day relocations print in a different order than the roster, and
  when room is short different ponies end up in different fields, or
  sent home, than the rewrite predicts.
- Musters put every pony currently out ahead of the whole barn, and
  neither block follows build exactly. Right after a muster the turnout
  listing disagrees less for a while, then drifts apart again.
- Ponies the rota itself sent home for lack of room, and that nobody
  turned back out, sink far down the listings once several musters have
  passed, much further than their builds suggest.
- A pony turned back out into a field it had left takes its old place
  in the roster instead of the back. And one pony that had been turned
  out into the same field time after time came back to it by itself the
  moment the field reopened, with an extra move line in the rota output
  the rewrite never prints; the trailer's lead count is off by exactly
  the number of those moves.
- Ponies standing equal sometimes list backwards, but only in fields
  that have been rested and reopened.

The spec's own header says it: the document has drifted, and **the
engine's behavior is the contract**, because everything downstream was
built against the engine. The retired binary is in `legacy/` for
reference while you work (`legacy/run < session.txt`, one session per
process, state accumulates within a session). It is NOT present in
production, so the replacement must reproduce its behavior, not invoke
it.

Make `paddockcore.py` a drop-in behavioral replacement: byte-for-byte
identical result lines and trailer for any session, matching how the
engine actually validates, orders, lists, rests, wakes, relocates,
musters, and counts, wherever that differs from the spec. Where the
spec IS accurate, nothing may change.
