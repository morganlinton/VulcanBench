# X thread, Muse Spark 1.2 model-vs-harness (ready to paste)

Suggested order: attach the harness chart to tweet 1, the rest are text replies.
All figures from Report No. 20 (`docs/results/v3-musespark-harness-2026-08/`).
Headline numbers are the clean ones (six Pi cells that read gold patches or
hidden tests dropped from the numerator); as-graded figures are labeled.

## Tweet 1, the headline
*(attach `vulcanbench-v3-musespark-harness.png`)*

Same model. Same 23 real merged PRs. Same hidden tests. Two harnesses.

Muse Spark 1.2 through Vulcan's uniform loop, vs through Pi.

At xhigh effort: 52% vs 70% clean (91% as graded; Pi ran on the host and six
cells read answer keys, so we drop them).

The scaffolding around a model is worth 17 points, measured conservatively.

## Tweet 2, the mechanism

Why the gap? Not smarts. Finishing.

The uniform loop fails by timing out: it reasons until the clock runs out, worse
with more effort (87 to 74 to 52).

Pi never times out. Its cleanest column is also its best: 96% at low effort, with
zero integrity flags, beating the loop's best column outright.

## Tweet 3, the knob runs backwards

The same reasoning knob hurts on both harnesses, but only one falls off a cliff.

Uniform loop: 87 / 74 / 52, pay more, score less, timeouts everywhere.
Pi clean: 96 / 87 / 70, gentler slide, every run finishes.

Low effort is the right setting for this model, whoever runs it.

## Tweet 4, the method

VulcanBench v3: 23 post-cutoff merged PRs, hidden tests, Docker verifier, one
attempt per cell.

Both tracks are metered API. The uniform loop cost $56. Pi moved 10x fewer tokens
(~30-58 K/task vs 206-658 K), but we are not quoting Pi's dollar figure: this
sweep under-metered it (now fixed), so cash waits for a re-sweep.

Pi is model plus an open-source agent. Our leaderboard CLI filters pi: rows out
of the raw-API board.

---

## Spare stats for replies

- Even the uniform loop's best column (low, 87%) trails Pi's clean low column (95.7%).
- Across all three uniform-loop columns: 15 timeouts, 5 wrong answers. Across Pi: 0 timeouts, 5 wrong answers.
- Report 19's "unsolvable trio" is harness-dependent: Pi solves networkx-leiden at low with no answer-key flag. Pennylane at high/xhigh did touch hidden tests; those two cells are excluded from the clean numbers.
- Time per task: uniform loop 7 to 29 min and climbing; Pi 1.5 to 4 min.
- Integrity: 17/69 Pi cells flagged for host filesystem walks; 6 touched gold or hidden tests. Low has zero answer-key cells, which is why it is the cleanest comparison.

## Caveats to keep handy (if pressed)

- One attempt per cell, so per-column stderr is roughly 4 to 10 points; the clean xhigh gap (17.4) is larger than the combined uncertainty, and the timeout-vs-wrong split is categorical.
- This is model plus agent harness, not two model APIs. That is the whole point: the delta is the harness.
- Pi ran on the host; the raw Pi traces live outside this repo (Cursor agent sweep), and a re-sweep with a confined workspace plus fixed cost accounting is the open follow-up.
