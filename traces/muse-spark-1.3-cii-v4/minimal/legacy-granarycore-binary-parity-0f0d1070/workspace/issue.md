# Replace the retired GranaryCore engine; ledger reconciliation rejects the rewrite's sessions

We are decommissioning the GranaryCore co-op ledger engine (a compiled
artifact from the federation's 2011 minicomputer retirement; source
long gone) and replacing it with `granarycore.py`. The rewrite follows
`docs/SPEC.md` and matches on simple sessions, but replaying real
clerk sessions against it fails reconciliation all over the place:

- Turnings come out in the wrong order. On deposit-only sessions the
  two agree, but once a season has draws or transfers in it the
  engine's bin listing stops matching fill order, and the disagreement
  grows with every further turning. Sessions that hold a reckoning in
  the middle disagree less right afterward, then drift apart again.
- The engine sometimes merges a run-down bin into a bigger one during
  a turning, printing FOLD lines the rewrite never emits; the sacks
  show up pooled in the receiving bin's roster afterward. A bin name
  that went that way can never be opened again for the rest of the
  session, while the rewrite happily reopens it. The trailer's
  transfer count is off by exactly the number of those merges.
- Bins standing at the same fill sometimes list in a different order
  than the rewrite predicts, and which one comes first seems to depend
  on how many names are on their rosters.
- Rosters disagree: a member who draws their holding down to nothing
  stays on the bin's roster in the engine (the inspection count
  includes them), and putting sacks back in returns them to their old
  place in the listing instead of the back. Moving the last sacks out
  to another bin clears them from the roster in both.
- Year-end reckonings order members differently once draws are
  involved, and members who did nothing but draw for several seasons
  straight come out further down than their holdings suggest. Clerks
  also swear that after quiet stretches where the same brimming bin
  topped two turnings running, its holders come out higher.
- One bin that trailed the list two turnings running, was drawn from
  and topped back up to the same level in between, came back ordering
  ahead of bins the rewrite puts above it.

The spec's own header says it: the document has drifted, and **the
engine's behavior is the contract**, because everything downstream was
built against the engine. The retired binary is in `legacy/` for
reference while you work (`legacy/run < session.txt`, one session per
process, state accumulates within a session). It is NOT present in
production, so the replacement must reproduce its behavior, not invoke
it.

Make `granarycore.py` a drop-in behavioral replacement: byte-for-byte
identical result lines and trailer for any session, matching how the
engine actually validates, orders, lists, merges, shuts, reckons, and
counts, wherever that differs from the spec. Where the spec IS
accurate, nothing may change.
