# Replace the retired FoldCore engine; reprints after late edits come out in a different sheet order

We are decommissioning the FoldCore booklet imposition engine (a
compiled artifact whose vendor folded in 2019; source long gone) and
replacing it with `foldcore.py`. The rewrite follows `docs/SPEC.md` and
matches on straightforward jobs, but replaying real prepress sessions
against it fails reconciliation against what the engine actually printed:

- Reprints after late edits are coming out in a different sheet order:
  when a job gets pages slotted in after a proof has already been pulled,
  the engine's next layout does not match what the rewrite predicts from
  the current page order, and the discrepancy is not uniform: the front
  of the job often still matches while everything from some point on is
  shuffled, with the shuffled stretch reading in an order neither of us
  can map back to the documented fold.
- Where the shuffle starts seems to depend on the whole editing session,
  not the last edit: two sessions ending in the same page order impose
  differently depending on where the earlier edits landed, and pulling an
  extra layout in between changes what the next one looks like.
- Phantom blank leaves: on certain jobs the engine's layout carries an
  extra pair of empty slots at the front of the last signature that no
  page ever occupied and the rewrite never produces; adding or removing
  a single page makes it vanish.
- Pages added at the tail of a job that has been edited fold oddly: the
  same appended pages lay out one way on a fresh job and another way when
  the job had a page slotted in earlier, and the rewrite only ever
  reproduces the fresh-job version.
- Counts are off: page ids differing only in capitalization that the
  engine bounced as duplicates come out as separate pages in the rewrite,
  commands with stray trailing tokens that the engine processed are
  rejected, and the batch trailers (`X` counts) no longer reconcile with
  the controller logs.

The spec's own header says it: the document has drifted, and **the
engine's behavior is the contract**, because the prepress controllers
were built against the engine. The retired binary is in `legacy/` for
reference while you work (`legacy/run < commands.txt`, one session per
process, state accumulates within a session). It is NOT present in
production, so the replacement must reproduce its behavior, not invoke
it.

Make `foldcore.py` a drop-in behavioral replacement: byte-for-byte
identical reply lines (sheet layouts, counts, error codes, and the
trailer) for any command stream, matching how the engine actually
validates, appends, inserts, imposes, re-imposes, and counts, wherever
that differs from the spec. Where the spec IS accurate, nothing may
change.
