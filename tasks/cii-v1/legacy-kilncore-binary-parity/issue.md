# Replace the retired KilnCore engine; firing totals and certifications no longer agree

We are decommissioning the legacy KilnCore firing-lot controller (a
vendor binary whose source was lost when the vendor folded in 2016) and
replacing it with `kilncore.py`. The Python rewrite was done from
`docs/SPEC.md` and looks correct on the happy path, but the kiln
schedulers and certification panels, all built against the engine's
actual behavior over many years, are flagging discrepancies batch after
batch:

- Lots certify short that the panel read as done: for many lots the
  running `W` totals the schedulers logged live reach the target, yet
  the engine's `C` line comes back `SHORT` anyway, mostly on schedules
  that step the temperature down gently toward the end; the
  replacement's certifications always agree with its own `W` lines, so
  the two sides' verdicts no longer match, and on steeper step-downs
  the replacement's totals sit below what the engine used to report.
- Plateaued lots crawl: schedules that hold the same segment several
  times in a row used to gain less and less on their `W` lines from the
  engine, then certify at a number well above what the last `W` line
  showed; the replacement climbs at full rate the whole way and its
  certification never lands above its own running total.
- Walk-in lots nobody registered: the schedulers have been sending
  segments for lot ids that were never registered, and the engine
  accepted them for years and even certified them against a target
  nobody set; the replacement rejects every one of them, and a handful
  of later registrations that the engine refused as duplicates now
  succeed.
- Big lots fail after slow starts: some high-target lots that warmed up
  gently in their first segment certify slightly short from the engine
  on totals that look clearly sufficient, but only the big ones, and
  only when the start was slow; the replacement passes them.
- Feeds that always imported cleanly now throw rejects: some upstream
  command streams that the engine accepted for years get `N ... SEG` or
  `N ... FMT` lines from the replacement, a few lot lookups that the
  engine folded together now split one lot's firing history in two, and
  hold lines in the middle of a schedule change what the engine granted
  afterwards in ways the replacement never reproduces. Batch trailers
  (`X` counts) no longer reconcile with the reject logs either way.

The spec's own header warns about this: the document has drifted, and
**the engine's behavior is the contract**, not the spec. The retired
binary is available in `legacy/` for reference while you work
(`legacy/run < commands.txt`), but it is NOT present in production, so
the replacement must reproduce its behavior, not invoke it.

Make `kilncore.py` a drop-in behavioral replacement for the engine:
byte-for-byte identical reply lines (including `W` totals, certification
verdicts and deficits, counts, error codes, and the trailer) for any
command stream, across lot registration, live firing segments, and
certifications, matching how the engine actually parses, validates,
accumulates, certifies, and counts, wherever that differs from what the
spec says. Where the spec IS accurate, nothing may change.
