# Replace the retired PackCore engine; capacity audits no longer reconcile

We are decommissioning the PackCore bin-allocator engine (a compiled
artifact from the 2015 platform migration; source long gone) and
replacing it with `packcore.py`. The rewrite follows `docs/SPEC.md` and
matches on simple sessions, but replaying real provisioner sessions
against it fails reconciliation:

- Compaction is the big one: after a `K` the rewrite frequently reports
  a different bin count than the engine's logs show, usually one bin
  too many, and the layout digests never line up again for the rest of
  the session even when the counts happen to agree.
- Allocations that reuse an id are landing in the wrong place: a
  workload that frees an item and later places it again under the same
  name gets digests the audit tooling has never seen, and sometimes an
  extra bin the engine never opened. Fresh ids placed at the same
  moment behave exactly as predicted.
- Half-capacity items are opening bins the rewrite does not predict:
  some sessions show the engine starting a new bin for a size-50 item
  that plainly fit in the existing one, but only in certain states;
  most size-50 placements pack normally.
- Digests drift after certain frees but the bin counts stay right, and
  the drift vanishes at the next compaction as if nothing happened.
- Some placements the rewrite rejects produce NO response line at all
  in the engine's logs, so every subsequent reply is off by one and the
  trailer counts disagree.
- Item ids sent with different capitalization come out as separate
  items in the rewrite where the engine treated them as one, and
  commands with stray trailing tokens that the engine processed are
  bounced by the rewrite, throwing the trailer counts off further.

The spec's own header says it: the document has drifted, and **the
engine's behavior is the contract**, because everything downstream was
built against the engine. The retired binary is in `legacy/` for
reference while you work (`legacy/run < session.txt`, one session per
process, state accumulates within a session). It is NOT present in
production, so the replacement must reproduce its behavior, not invoke
it.

Make `packcore.py` a drop-in behavioral replacement: byte-for-byte
identical result lines and trailer for any session, matching how the
engine actually validates, places, frees, compacts, digests, and
counts, wherever that differs from the spec. Where the spec IS
accurate, nothing may change.
