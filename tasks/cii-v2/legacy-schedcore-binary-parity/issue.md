# Replace the retired SchedCore engine; pipeline reconciliation rejects the rewrite's sessions

We are decommissioning the SchedCore dependency-scheduler engine (a
compiled artifact from the 2013 platform migration; source long gone)
and replacing it with `schedcore.py`. The rewrite follows `docs/SPEC.md`
and matches on simple sessions, but replaying real controller sessions
against it fails reconciliation: reruns after a failure execute in a
different order than the first pass did, and once a session has a
failure in it the two schedules never line up again, with the most
recently torn-down jobs consistently going first where the rewrite
predicts definition order. Jobs that have failed twice sink in the
rerun schedule as if they had quietly lost priority. Fan-in jobs, the
ones gating on several dependencies, jump the queue on fresh runs at
certain priorities but only on fresh runs, never on reruns. Jobs whose
dependencies were registered late, after a run had already tried to
place them, never unblock in the engine no matter how many runs follow,
while the rewrite happily schedules them. Dependency lists with a
repeated entry that the engine accepted are bounced by the rewrite,
throwing the reject counts off, and job ids sent with different
capitalization come out as separate jobs in the rewrite where the
engine treated them as one. Commands with stray trailing tokens that
the engine processed are rejected too.

The spec's own header says it: the document has drifted, and **the
engine's behavior is the contract**, because everything downstream was
built against the engine. The retired binary is in `legacy/` for
reference while you work (`legacy/run < session.txt`, one session per
process, state accumulates within a session). It is NOT present in
production, so the replacement must reproduce its behavior, not invoke
it.

Make `schedcore.py` a drop-in behavioral replacement: byte-for-byte
identical result lines and trailer for any session, matching how the
engine actually validates, schedules, retries, blocks, and counts,
wherever that differs from the spec. Where the spec IS accurate,
nothing may change.
