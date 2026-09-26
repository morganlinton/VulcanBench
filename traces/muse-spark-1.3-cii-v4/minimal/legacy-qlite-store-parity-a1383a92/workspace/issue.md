# Replace the retired QLite store; the reporting pipelines reject the rewrite

We are decommissioning the QLite record store (a compiled artifact from
the 2019 acquisition; its source is gone) and replacing it with
`qlite.py`. The rewrite follows `docs/SPEC.md` and passes simple manual
checks, but replaying recorded production command logs against it fails
constantly: scan results come back in the wrong order after delete-heavy
sessions, searches match names the engine never matched (and miss ones it
matched), range reports are off at the edges, duplicate-key loads behave
completely differently, long-name records come back altered, and the
nightly totals disagree with the engine's, sometimes wildly.

The spec's own header says it: the document has drifted, and **the
engine's behavior is the contract**, because every reporting pipeline
downstream was built against the engine. The retired binary is in
`legacy/` for reference while you work (`legacy/run < commands.txt`, one
store per process). It is NOT present in production, so the replacement
must reproduce its behavior, not invoke it.

Make `qlite.py` a drop-in behavioral replacement: line-for-line identical
responses for any command session, matching how the engine actually
stores, orders, matches, validates, and aggregates, wherever that differs
from the spec. Where the spec IS accurate, nothing may change.
