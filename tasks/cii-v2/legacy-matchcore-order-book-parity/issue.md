# Replace the retired MatchCore engine; the certified consumers reject the rewrite's sessions

We are decommissioning the MatchCore matching engine (a compiled artifact
from the 2020 exchange migration whose source no longer exists) and
replacing it with `matchcore.py`. The rewrite follows `docs/SPEC.md` and
handles simple sessions correctly, but certification replays against
recorded production sessions fail all over the place: fills coming out in
the wrong order at a price level, orders rejected that the engine
accepted (and accepted that it rejected), cancel acknowledgements with
the wrong quantity, market sweeps filling deeper than the engine ever
did, self-trade situations handled completely differently, and
end-of-session summaries whose book hash almost never matches.

The spec's own header warns that it has drifted: **the engine's behavior
is the contract**, and every downstream consumer was certified against
the engine, not the document. The retired binary is in `legacy/` for
reference while you work (`legacy/run < session.txt`, one session per
process). It is NOT present in production, so the replacement must
reproduce its behavior, not invoke it.

Make `matchcore.py` a drop-in behavioral replacement: line-for-line
identical responses for any message session, matching how the engine
actually validates, prioritizes, matches, cancels, and summarizes,
wherever that differs from the spec. Where the spec IS accurate, nothing
may change.
