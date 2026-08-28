# Replace the retired LoyaltyCore engine; member statements disagree with the rewrite

We are decommissioning the LoyaltyCore points engine (a compiled artifact
that survived the 2016 rewrite of everything around it; its source did
not) and replacing it with `loyaltycore.py`. The rewrite follows
`docs/SPEC.md` and matches on simple batches, but statement reconciliation
against the engine's output fails all over real earn files: members
crossing a tier get credited differently than the rewrite computes on
exactly the crossing transaction, a supposedly informational promo code
visibly changes electronics earnings except when it does not, travel earns
come out a point lower here and there, some members reach Gold earlier
than the documented threshold says they should, sub-dollar transactions
appear in the rewrite's output but never in the engine's (and the batch
counters disagree accordingly), and members keyed with different
capitalization come out merged with the earlier spelling.

The spec's own header says it: the document has drifted, and **the
engine's behavior is the contract**, because the statement and
partner-mall systems were built against the engine. The retired binary is
in `legacy/` for reference while you work (`legacy/run < batch.txt`, one
batch per process, lifetime points accumulate within a batch). It is NOT
present in production, so the replacement must reproduce its behavior,
not invoke it.

Make `loyaltycore.py` a drop-in behavioral replacement: byte-for-byte
identical result lines and trailer for any batch, matching how the engine
actually validates, multiplies, rounds, accumulates, promotes, and
counts, wherever that differs from the spec. Where the spec IS accurate,
nothing may change.
