# Replace the retired LockCore engine; lease reconciliation rejects the rewrite's sessions

We are decommissioning the LockCore lease-manager engine (a compiled
artifact from the 2012 platform migration; source long gone) and
replacing it with `lockcore.py`. The rewrite follows `docs/SPEC.md` and
matches on simple sessions, but replaying real coordination-service
sessions against it fails reconciliation: once a session contains
renewals, the two lease timelines drift apart, with the engine's
expiry sweeps releasing heavily renewed leases EARLIER than the rewrite
predicts, and the more a lease was renewed the sooner it goes, as if
renewal were quietly wearing the lease out. Clients that lose a
resource to expiry and come back for it right away end up at the front
of the line in the engine's grant order, jumping waiters who queued
first, but only when they come back soon; return late and the queue is
honest FIFO again. Long-named resources acquired with large ttls get
swept far earlier than their ttl says they should, while short-named
ones behave. Renewing a lease at the very tick its time runs out is
bounced by the engine as if the lease were already gone, even though no
sweep has released it yet, where the rewrite happily renews it. Release
commands for clients holding nothing sail through the engine with a
zero count but are rejected by the rewrite, throwing the reject
tallies and everything after them off, and ids sent with different
capitalization come out as separate clients and resources in the
rewrite where the engine treated them as one. Commands with stray
trailing tokens that the engine processed are rejected too.

The spec's own header says it: the document has drifted, and **the
engine's behavior is the contract**, because everything downstream was
built against the engine. The retired binary is in `legacy/` for
reference while you work (`legacy/run < session.txt`, one session per
process, state accumulates within a session). It is NOT present in
production, so the replacement must reproduce its behavior, not invoke
it.

Make `lockcore.py` a drop-in behavioral replacement: byte-for-byte
identical result lines and trailer for any session, matching how the
engine actually validates, grants, renews, sweeps, queues, and counts,
wherever that differs from the spec. Where the spec IS accurate,
nothing may change.
