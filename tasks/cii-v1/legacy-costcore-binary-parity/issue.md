# Replace the retired CostCore engine; ledger reconciliation rejects the rewrite's batches

We are decommissioning the CostCore inventory costing engine (a compiled
artifact from the 2013 warehouse-systems acquisition; source long gone)
and replacing it with `costcore.py`. The rewrite follows `docs/SPEC.md`
and matches on simple batches, but ledger and fulfillment reconciliation
fail on real stock files: cost of goods issued disagreeing on SKUs with
several receipts on the books, stockout issues billed at amounts the
rewrite never produces, partial shipments the old system allowed that
the rewrite rejects outright, back-to-back receipts at the same price
coming out as one layer where the rewrite keeps two, issues that clear a
bin out costing slightly more than the layers add up to, expensive
receipts valued below what was keyed in, zero-quantity lines that simply
vanish from the old system's output but show up as rejects in ours, and
SKUs keyed twice with different capitalization coming out merged.

The spec's own header says it: the document has drifted, and **the
engine's behavior is the contract**, because everything downstream was
built against the engine. The retired binary is in `legacy/` for
reference while you work (`legacy/run < batch.txt`, one batch per
process, stock and cost layers accumulate within a batch). It is NOT
present in production, so the replacement must reproduce its behavior,
not invoke it.

Make `costcore.py` a drop-in behavioral replacement: byte-for-byte
identical result lines and trailer for any batch, matching how the
engine actually validates, layers, consumes, prices, and counts,
wherever that differs from the spec. Where the spec IS accurate, nothing
may change.
