# Replace the retired SettleCore engine; downstream reconciliation is rejecting our output

We are decommissioning the legacy SettleCore batch engine (a vendor binary
whose source was lost in the 2021 handover) and replacing it with
`settlecore.py`. The Python rewrite was done from `docs/SPEC.md` and looks
correct on the happy path, but the downstream reconciliation systems,
which were all built against the engine's actual behavior over many years,
are rejecting batch after batch of our output: fees off by a cent or a
few, records rejected that the engine accepted (and vice versa), whole
batches thrown out over response ordering and trailer mismatches.

The spec's own header warns about this: the document has drifted, and
**the engine's behavior is the contract**, not the spec. The retired
binary is available in `legacy/` for reference while you work
(`legacy/run < batch.txt`), but it is NOT present in production, so the
replacement must reproduce its behavior, not invoke it.

Make `settlecore.py` a drop-in behavioral replacement for the engine:
byte-for-byte identical output lines (including check characters and the
trailer) for any input batch, matching how the engine actually parses,
validates, computes, orders, and formats, wherever that differs from what
the spec says. Where the spec IS accurate, nothing may change.
