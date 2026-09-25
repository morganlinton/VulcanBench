# Replace the retired QueueCore engine; dispatch reconciliation rejects the rewrite's sessions

We are decommissioning the QueueCore priority work-queue engine (a
compiled artifact from the 2014 platform migration; source long gone)
and replacing it with `queuecore.py`. The rewrite follows `docs/SPEC.md`
and matches on simple sessions, but replaying real dispatcher sessions
against it fails reconciliation: retried jobs drain in the wrong order,
and once a session has a retry in it the two queues never line up again,
with retried jobs sometimes cutting ahead of jobs that were already
waiting at the same priority and sometimes falling behind them depending
on how they got back in. Some jobs vanish to the dead letter well before
their third strike. Jobs enqueued at the very top priority come back
from a retry in a subtly different position than the rewrite predicts.
Operators are undoing two deep: session logs show a second fail command
right after a first one that the engine accepted and the rewrite
rejects. Job ids sent with different capitalization come out as separate
jobs in the rewrite where the engine treated them as one, and commands
with stray trailing tokens that the engine processed are bounced by the
rewrite, throwing the trailer counts off.

The spec's own header says it: the document has drifted, and **the
engine's behavior is the contract**, because everything downstream was
built against the engine. The retired binary is in `legacy/` for
reference while you work (`legacy/run < session.txt`, one session per
process, state accumulates within a session). It is NOT present in
production, so the replacement must reproduce its behavior, not invoke
it.

Make `queuecore.py` a drop-in behavioral replacement: byte-for-byte
identical result lines and trailer for any session, matching how the
engine actually validates, orders, retries, dead-letters, drains, and
counts, wherever that differs from the spec. Where the spec IS accurate,
nothing may change.
