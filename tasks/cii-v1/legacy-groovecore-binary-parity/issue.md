# Replace the retired GrooveCore controller; cuts and remasters no longer reconcile

We are decommissioning the legacy GrooveCore vinyl-pressing queue
controller (a vendor binary whose source was lost when the vendor folded
in 2018) and replacing it with `groovecore.py`. The Python rewrite was
done from `docs/SPEC.md` and looks correct on the happy path, but the
lathe consoles and pressing-floor reconcilers, all built against the
controller's actual behavior over many years, are flagging discrepancies
shift after shift:

- Quality numbers sag as a title's sides pile up: the old controller's
  quality replies for consecutive sides of the SAME title come down in
  even steps, side after side, then jump back to full after a batch
  remaster; identically timed sides of a freshly started title come out
  higher. The replacement reports one flat number per title forever, so
  the maintenance queue that used to open lathe tickets on sagging
  quality went silent.
- Remaining-allotment reports disagree after busy stretches: for titles
  that cut a lot of sides between remasters, the `R` lines the old
  controller prints after a remaster come out LOWER than six minus the
  sides cut, and the shortfall grows with every remaster. And for
  long-running titles it goes the other way around entirely: the old
  controller refuses further sides (`N ... SPENT`) far earlier than the
  ledger says it should, then a remaster suddenly shows MORE remaining
  than the live refusals implied. The replacement always agrees with
  six-minus-sides on both paths.
- Some fourth sides come in exactly 15 points low: on busy shifts with
  several titles in progress at once, a title's fourth side is
  occasionally graded 15 under what the sag pattern accounts for; the
  same title cut on a quiet board is not. The replacement never does
  this.
- Spent titles come back from the dead: titles the old controller had
  refused as SPENT sometimes accept exactly one more side after a
  remaster, but only ever once per title; the replacement keeps
  refusing them.
- Remasters run on an idle press disagree in shape: the old controller
  prints the full per-title `R` report and counts the remaster in the
  trailer even when nothing was cut since the last one; the replacement
  prints only the `MEND` line and does not count it. Batch trailers
  (`X` counts) no longer reconcile with the shift logs either way.
- Titles registered in the middle of a busy shift start life graded
  low: their very first side comes out below `1000 - 3 * minutes`, by
  more the busier the board was at registration, while titles
  registered before the shift starts grade exactly to formula. The
  replacement grades everyone to formula.

The spec's own header warns about this: the document has drifted, and
**the controller's behavior is the contract**, not the spec. The retired
binary is available in `legacy/` for reference while you work
(`legacy/run < commands.txt`), but it is NOT present in production, so
the replacement must reproduce its behavior, not invoke it.

Make `groovecore.py` a drop-in behavioral replacement for the controller:
byte-for-byte identical reply lines (including quality numbers, remaining
allotments, remaster reports, error codes, and the trailer) for any
command stream, across title registration, side cutting, and batch
remasters, matching how the controller actually validates, grades,
consumes allotment, remasters, and counts, wherever that differs from
what the spec says. Where the spec IS accurate, nothing may change.
