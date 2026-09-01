# Replace the retired DepotCore engine; depot reconciliation rejects the rewrite's sessions

We are decommissioning the DepotCore rural parcel-depot engine (a
compiled artifact from the 2015 platform migration; source long gone)
and replacing it with `depotcore.py`. The rewrite follows `docs/SPEC.md`
and matches on simple sessions, but replaying real depot sessions
against it fails reconciliation, and the longer the session the worse it
gets.

Van rounds come out in the wrong order: parcels from senders who lodge
steadily keep cutting ahead of heavier parcels the rewrite puts first,
and parcels that bounced back drift toward the back of the van in ways
the declared heft does not explain, further each time they bounce. On
lanes that have already run several rounds, freshly shelved parcels slip
behind older ones the rewrite ranks below them, and some well-used lanes
send out three-parcel rounds where the rewrite insists on four. The
shelving crew's PUT lines come out grouped differently than the rewrite
prints them, returned parcels are shelved ahead of the day's arrivals
instead of behind them, and per-sender holdings listings come out in a
different order.

The seasonal square is the worst of it. The spec calls it a bookkeeping
formality, but after a square the engine and the rewrite never agree
again: parcels still sitting in the arrival bay or on the return ledge
show up shelved in the posted listings, parcels a sender had withdrawn
reappear on their shelves (and can even be withdrawn a second time),
bounce reports the rewrite accepts right after a square are refused by
the engine, and shelf orders that matched before the square come back
subtly rearranged after it, with twice-bounced parcels sitting higher
than the rewrite expects. Sessions with empty-shelf rounds before a
square disagree about round sizes afterward. The trailer counts then
drift because commands one side accepted the other rejected.

The spec's own header says it: the document has drifted, and **the
engine's behavior is the contract**, because everything downstream was
built against the engine. The retired binary is in `legacy/` for
reference while you work (`legacy/run < session.txt`, one session per
process, state accumulates within a session). It is NOT present in
production, so the replacement must reproduce its behavior, not invoke
it.

Make `depotcore.py` a drop-in behavioral replacement: byte-for-byte
identical result lines and trailer for any session, matching how the
engine actually validates, shelves, dispatches, bounces, waives,
squares, and counts, wherever that differs from the spec. Where the
spec IS accurate, nothing may change.
