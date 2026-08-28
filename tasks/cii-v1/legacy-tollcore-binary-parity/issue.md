# Replace the retired TollCore engine; revenue reconciliation is rejecting our batches

We are decommissioning the legacy TollCore passage rating engine (a
vendor binary whose source was lost in the 2020 tolling-authority
handover) and replacing it with `tollcore.py`. The Python rewrite was
done from `docs/SPEC.md` and looks right on the happy path, but the
downstream revenue reconciliation systems, all built against the engine's
actual behavior over many years, are throwing out batch after batch of
our output.

Symptoms from the reconciliation reports, none of which we can explain
from the spec:

- weekend truck tolls at a handful of mid-numbered gates come out
  consistently high in our output, sometimes by hundreds of cents, and
  occasionally off by exactly one cent even when the rest of the amount
  looks plausible;
- early-morning passages in the 0645-0700 range disagree at the
  river-crossing gates but nowhere else, and some evening passages just
  after 1900 disagree only at the first few gates;
- bus tolls during peak hours are always slightly higher in our output;
- some motorcycle passages with larger axle counts reconcile as if they
  were cars;
- the engine accepted passages that we reject outright (certain midnight
  timestamps, a day-of-week value our validator refuses, and lines with
  stray trailing tokens), so our accept/reject counts and trailer sums
  drift from the engine's on real traffic.

The spec's own header warns about this: the document has drifted, and
**the engine's behavior is the contract**, not the spec. The retired
binary is available in `legacy/` for reference while you work
(`legacy/run < passages.txt`), but it is NOT present in production, so
the replacement must reproduce its behavior, not invoke it.

Make `tollcore.py` a drop-in behavioral replacement for the engine:
identical output lines (including reject codes and the trailer) for any
input batch, matching how the engine actually parses, validates,
computes, rounds, and formats, wherever that differs from what the spec
says. Where the spec IS accurate, nothing may change.
