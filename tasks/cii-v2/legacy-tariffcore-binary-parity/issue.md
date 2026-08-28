# Replace the retired TariffCore engine; customs reconciliation rejects the rewrite's assessments

We are decommissioning the TariffCore duty calculator (a compiled
artifact from 2015 whose source is gone) and replacing it with
`tariffcore.py`. The rewrite follows `docs/SPEC.md` and matches on
straightforward declarations, but reconciliation against the engine's
assessed batches keeps failing in ways nobody can pattern: duties off by
a cent on scattered declarations, weight fees disagreeing on odd weights
and on certain air shipments, a supposedly reserved output column coming
back non-zero for some origin/commodity combinations, large month-end
declarations assessed slightly differently than identical mid-month ones,
some sea shipments with no weight fee at all, capped assessments
distributed across columns differently, and a commodity chapter the spec
calls invalid that the engine happily assesses.

The spec's own header says it: the document has drifted, and **the
engine's behavior is the contract**, because customs reconciliation was
certified against the engine. The retired binary is in `legacy/` for
reference while you work (`legacy/run < declarations.txt`). It is NOT
present in production, so the replacement must reproduce its behavior,
not invoke it.

Make `tariffcore.py` a drop-in behavioral replacement: byte-for-byte
identical result lines and trailer for any declaration batch, matching
how the engine actually validates, rates, rounds, surcharges, caps, and
counts, wherever that differs from the spec. Where the spec IS accurate,
nothing may change.
