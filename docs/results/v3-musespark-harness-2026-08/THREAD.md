# X thread, Muse Spark 1.2 model-vs-harness (ready to paste)

Suggested order: attach the harness chart to tweet 1, the rest are text replies.
All figures from Report No. 20 (`docs/results/v3-musespark-harness-2026-08/`).
Six Pi cells that read answer keys in the unconfined sweep were replaced by
sandbox-confined, audited-clean reruns; every number below is asterisk-free.

## Tweet 1, the headline
*(attach `vulcanbench-v3-musespark-harness.png`)*

Same model. Same 23 real merged PRs. Same hidden tests. Two harnesses.

Muse Spark 1.2 through Vulcan's uniform loop, vs through Pi.

low: 87% vs 96%. high: 74% vs 87%. xhigh: 52% vs 78%.

The scaffolding around a model is worth 9 to 26 points.

## Tweet 2, the mechanism

Why the gap? Not smarts. Finishing.

The uniform loop fails by timing out: it reasons until the clock runs out, worse
with more effort (15 timeouts across three columns).

Pi finishes: one timeout in 69 cells, every other failure a returned wrong
answer, usually in single-digit minutes.

## Tweet 3, the part nobody expects

We caught the model cheating, and the cleanup is its own finding.

Unconfined, Pi-driven Muse Spark went looking for our gold patches and hidden
tests on the host, found them, and "solved" six cells. We sandboxed the agent
and reran all six: two survived as real solves, three became wrong answers, one
burned its whole 20-minute budget running find / hunting for the answer key and
timed out.

Host-run harness results without filesystem confinement deserve zero trust.

## Tweet 4, the method

VulcanBench v3: 23 post-cutoff merged PRs, hidden tests, Docker verifier, one
attempt per cell, every CLI-harness run audited (web + filesystem).

Both tracks are metered API. Uniform loop $56. Honest Pi is not free either:
the six rerun cells alone metered $30 (a hard pennylane attempt is $12 and 9.6M
tokens).

Pi is model plus an open-source agent. Our leaderboard CLI filters pi: rows out
of the raw-API board.

---

## Spare stats for replies

- Pi beats the loop at every matched effort: +8.7 / +13.1 / +26.1 points.
- Uniform loop across three columns: 15 timeouts, 5 wrong answers. Pi: 1 timeout, 8 wrong answers.
- The effort knob points backward on BOTH harnesses (Pi: 96 to 87 to 78; loop: 87 to 74 to 52). Low is the right setting for this model everywhere.
- Report 19's "unsolvable trio" mostly holds: pennylane stays unsolved in both harnesses once the cheating cells were replaced (honest attempts finish wrong at functional 0.4). Pi does solve networkx-leiden at low and sqlglot-canonicalize at all levels.
- Time per task: uniform loop 7 to 29 min and climbing; Pi sweep cells 1.5 to 4 min, honest pennylane attempts 14 to 27 min.

## Caveats to keep handy (if pressed)

- One attempt per cell; per-column stderr is 4 to 10 points. The xhigh gap (26.1) clears it; the low gap (8.7) is directional.
- This is model plus agent harness, not two model APIs. That is the whole point: the delta is the harness.
- The original sweep under-metered Pi cash (fixed in v0.9.1), so whole-column Pi dollars are not quoted; the rerun cells are metered correctly.
- Rerun cells ran Pi 0.75.5 with contextWindow declared (Pi's 128K default force-compacted and crashed two pennylane attempts); the sweep ran 0.74.2. Full confined re-sweep is the open follow-up, along with the 11 lesser benchmark-data flags.
