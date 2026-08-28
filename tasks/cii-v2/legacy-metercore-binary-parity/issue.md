# Replace the retired MeterCore engine; downstream billing reconciliation is rejecting our batches

We are decommissioning the legacy MeterCore utility-billing batch engine
(a vendor binary whose source was lost when the metering vendor was
acquired in 2020) and replacing it with `metercore.py`. The Python rewrite
was done from `docs/SPEC.md` and looks correct on simple batches, but the
downstream reconciliation and invoicing systems, which were all built
against the engine's actual behavior over many years, are rejecting batch
after batch of our output: bills off by a cent or by a few percent,
accounts appearing in our output that the engine never emitted (and
accounts merged by the engine that we report separately), readings we
reject that the engine accepted, and trailer totals that never match.

The failures are worst on multi-reading batches: single readings often
match, but the same reading billed after other readings for the account
comes out wrong, so the errors look order- and history-dependent.

The spec's own header warns about this: the document has drifted, and
**the engine's behavior is the contract**, not the spec. The retired
binary is available in `legacy/` for reference while you work
(`legacy/run < readings.txt`), but it is NOT present in production, so the
replacement must reproduce its behavior, not invoke it.

Make `metercore.py` a drop-in behavioral replacement for the engine:
byte-for-byte identical output (R lines, B lines, and the X trailer) for
any input batch, matching how the engine actually parses, validates,
accumulates, bills, orders, and formats, wherever that differs from what
the spec says. Where the spec IS accurate, nothing may change.
