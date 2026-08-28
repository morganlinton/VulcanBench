# Replace the retired QuotaCore engine; invoicing reconciliation rejects the rewrite's batches

We are decommissioning the QuotaCore API-metering biller (a compiled
artifact from the 2015 platform migration; source long gone) and
replacing it with `quotacore.py`. The rewrite follows `docs/SPEC.md` and
matches on simple batches, but invoicing and dunning reconciliation fail
on real metering files: per-key invoices disagree partway through a
billing cycle and then stay wrong, the remaining-quota column drifts for
some keys right from their first event, exactly one event per busy key is
billed wildly higher than the rewrite computes and later events agree
again, premium customers in a couple of regions are billed cheaper than
the published rate card, some overage blocks come out one block short,
mid-cycle plan upgrades that the engine billed happily are being
rejected, zero-call heartbeat events vanish from the engine's output but
show up in ours (throwing off the trailer counts), lowercase region
codes are accepted upstream but rejected by the rewrite, and keys sent
twice with different capitalization come out merged.

The spec's own header says it: the document has drifted, and **the
engine's behavior is the contract**, because everything downstream was
built against the engine. The retired binary is in `legacy/` for
reference while you work (`legacy/run < batch.txt`, one batch per
process, consumption accumulates within a batch). It is NOT present in
production, so the replacement must reproduce its behavior, not invoke
it.

Make `quotacore.py` a drop-in behavioral replacement: byte-for-byte
identical result lines and trailer for any batch, matching how the
engine actually validates, meters, prices, rounds, accumulates, and
counts, wherever that differs from the spec. Where the spec IS accurate,
nothing may change.
