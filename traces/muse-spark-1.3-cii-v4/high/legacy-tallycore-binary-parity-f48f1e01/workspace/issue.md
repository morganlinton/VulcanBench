# Replace the retired TallyCore engine; recounts are calling the wrong results

We are decommissioning the legacy TallyCore ranked-ballot tally engine (a
vendor binary whose source was lost when the vendor folded in 2018) and
replacing it with `tallycore.py`. The Python rewrite was done from
`docs/SPEC.md` and looks correct on the happy path, but the election-night
reporting stack, built against the engine's actual behavior over many
cycles, is misbehaving batch after batch:

- Recounts disagree with the engine: for some ballot sets `W` announces a
  different winner than the engine did, and even when the winner matches,
  the elimination order comes out shuffled. The discrepancies are worst
  in streams that run SEVERAL recounts back to back: the first round
  often reconciles fine and then later rounds drift further and further
  from the engine's calls, for ballot sets that look routine.
- Ballots the engine accepted are now bouncing: the reporting stack logs
  `N ... CAND` rejects on ballot lines the engine counted for years, and
  some registrations that used to come back `OK` with one count now come
  back with another, so candidate totals and the `X` trailer no longer
  reconcile with the archived logs.
- The running leader ticker matches the engine far more often than the
  recounts do, which makes no sense to the operators: the same ballots
  feed both, but only `W` output disagrees, and only for some streams.
- A few close races flipped on transfers: ballots that the engine let
  die with their eliminated candidate are being passed along to a next
  choice by the new code (or counted differently before that), changing
  who survives the middle rounds.

The spec's own header warns about this: the document has drifted, and
**the engine's behavior is the contract**, not the spec. The retired
binary is available in `legacy/` for reference while you work
(`legacy/run < commands.txt`), but it is NOT present in production, so
the replacement must reproduce its behavior, not invoke it.

Make `tallycore.py` a drop-in behavioral replacement for the engine:
byte-for-byte identical reply lines (including `LEAD`, `ELIM`, `WIN`,
`OK` counts, `N` reject lines, and the `X` trailer) for any command
stream, across registration, ballot casting, and recount rounds,
matching how the engine actually parses, validates, counts, eliminates,
and transfers, wherever that differs from what the spec says. Where the
spec IS accurate, nothing may change.
