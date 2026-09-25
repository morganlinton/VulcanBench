# Replace the retired CellarCore engine; cellar reconciliation rejects the rewrite's sessions

We are decommissioning the CellarCore cellar-ledger engine (a compiled
artifact from the 2011 cellar-book migration; source long gone) and
replacing it with `cellarcore.py`. The rewrite follows `docs/SPEC.md`
and matches on simple sessions, but replaying real clerk-terminal
sessions against it fails reconciliation, and the failures compound:
once a session has a few samplings or a turning round in it, the two
ledgers rarely line up again.

What the reconciliation reports keep showing: wheels that were sampled
more than once between rounds stand lower than the rewrite predicts,
and wheels that were sampled while still waiting on the sill come out
of the next round standing HIGHER than the rewrite has them, as if the
round never heard about those samples. Wheels that have been moved
between racks repeatedly drift further with every move. After a round,
long-racked wheels sit lower than the rewrite expects, and the gap
widens with each further round. The round also drops wheels off racks
that the rewrite says should keep standing, and in a cellar where one
rack has dropped several wheels over its life, the round works the
racks in a different order than the rewrite prints. Wheels standing
equal sometimes list in a different order than the spec's lay-order
rule gives, in the round, the grading, and plain rack inspections
alike. The seasonal grading lists wheels in an order that matches
neither the rack order nor the spec's settle rule, retires wheels the
rewrite keeps, keeps wheels the rewrite retires, and occasionally the
engine even accepts laying in a wheel under a name the grading retired,
which the rewrite refuses outright, after which nothing about that
wheel ever reconciles again. And a few sessions show a dropped wheel
bedded straight back onto the same rack standing far higher than its
record says it should.

The spec's own header says it: the document has drifted, and **the
engine's behavior is the contract**, because everything downstream was
built against the engine. The retired binary is in `legacy/` for
reference while you work (`legacy/run < session.txt`, one session per
process, state accumulates within a session). It is NOT present in
production, so the replacement must reproduce its behavior, not invoke
it.

Make `cellarcore.py` a drop-in behavioral replacement: byte-for-byte
identical result lines and trailer for any session, matching how the
engine actually validates, lays, beds, samples, shifts, turns, grades,
and counts, wherever that differs from the spec. Where the spec IS
accurate, nothing may change.
