# Replace the retired LodgeCore engine; ledger reconciliation rejects the rewrite's sessions

We are decommissioning the LodgeCore bunk-ledger engine (a compiled
artifact from the 2015 booking-system migration; source long gone) and
replacing it with `lodgecore.py`. The rewrite follows `docs/SPEC.md` and
matches on simple sessions, but replaying real warden-terminal sessions
against it fails reconciliation, and the failures compound: once a
session has a few departures or an airing rota step in it, the two
ledgers rarely line up again.

What the reconciliation reports keep showing: parties that come back
for another stay berth in a different position than the rewrite
predicts, and how their previous stays ENDED seems to matter in ways
the spec's standing formula does not capture; parties that left early
more than once are especially far off. Rooms the rota has cycled
through several times relocate their occupants in a different order
than the rewrite, and sometimes a different party ends up turned away
when space is short. After a seasonal settling the engine reconstructs
different room assignments than the rewrite, and the next airing step
rests a different room entirely. Parties whose stays were repeatedly
bounced around by the rota settle lower than the rewrite expects, while
some parties that once left early settle higher. Bookings taken while a
room was being aired were accepted by the engine and show up berthed
after the room reopened, with the trailer's arrival count moving even
though no arrival command was ever accepted; the rewrite bounces those
bookings outright. And the engine sometimes squeezes a party into a
room the rewrite says is too full, after which every capacity decision
downstream disagrees.

The spec's own header says it: the document has drifted, and **the
engine's behavior is the contract**, because everything downstream was
built against the engine. The retired binary is in `legacy/` for
reference while you work (`legacy/run < session.txt`, one session per
process, state accumulates within a session). It is NOT present in
production, so the replacement must reproduce its behavior, not invoke
it.

Make `lodgecore.py` a drop-in behavioral replacement: byte-for-byte
identical result lines and trailer for any session, matching how the
engine actually validates, books, berths, relocates, settles, and
counts, wherever that differs from the spec. Where the spec IS
accurate, nothing may change.
