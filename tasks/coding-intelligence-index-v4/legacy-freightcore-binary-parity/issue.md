# Replace the retired FreightCore engine; carrier billing reconciliation is rejecting our charges

We are decommissioning the legacy FreightCore rating engine (a compiled
binary whose source was lost when the rating team was dissolved in 2020)
and replacing it with `freightcore.py`. The Python rewrite was done from
`docs/SPEC.md` and looks correct on the happy path, but the carrier
billing and reconciliation systems, which were all built against the
engine's actual charges over many years, are rejecting manifest after
manifest of our output: charges off by a few percent on some express
lanes, off by whole dollars on some low-class freight, shipments rejected
that the engine rated (and vice versa), and trailer totals that never
match theirs.

The spec's own header warns about this: the document has drifted, and
**the engine's behavior is the contract**, not the spec. The retired
binary is available in `legacy/` for reference while you work
(`legacy/run < manifest.txt`), but it is NOT present in production, so the
replacement must reproduce its behavior, not invoke it.

Make `freightcore.py` a drop-in behavioral replacement for the engine:
byte-for-byte identical output lines (including reject codes and the
trailer) for any input manifest, matching how the engine actually parses,
validates, rates, and totals, wherever that differs from what the spec
says. Where the spec IS accurate, nothing may change.
