# X thread, Muse Spark 1.2 model-vs-harness (ready to paste)

Suggested order: attach the harness chart to tweet 1, the rest are text replies.
All figures from Report No. 20 (`docs/results/v3-musespark-harness-2026-08/`).

## Tweet 1, the headline
*(attach `vulcanbench-v3-musespark-harness.png`)*

Same model. Same 23 real merged PRs. Same hidden tests. Two harnesses.

Muse Spark 1.2 through Vulcan's uniform loop, vs through Pi.

At xhigh effort: 52% vs 91%.

The scaffolding around a model is worth 39 points.

## Tweet 2, the mechanism

Why the gap? Not smarts. Finishing.

The uniform loop fails by timing out: it reasons until the clock runs out, and worse with more effort (87 to 74 to 52).

Pi fails by getting it wrong: zero timeouts, 1.5 to 4 minutes, scoring 96 / 91 / 91.

## Tweet 3, the knob runs backwards

The same reasoning knob points opposite directions on the same model.

Uniform loop: pay more, score less.
Pi: high and flat.

Judge Muse Spark by its xhigh API setting and you rate it 39 points below what it does inside Pi.

## Tweet 4, the method

VulcanBench v3: 23 post-cutoff merged PRs, hidden tests, Docker verifier, one attempt per cell.

Both tracks are metered cash ($56 uniform loop, $0.24 Pi).

Pi is model plus an open-source agent. Don't mix it into a raw-API leaderboard.

Pi runs on the host. Six cells touched gold patches or hidden tests; dropping those still leaves a 17-point xhigh gap and zero timeouts.

---

## Spare stats for replies

- Even the uniform loop's best column (low, 87%) trails Pi's worst (91.3%).
- Across all three uniform-loop columns: 15 timeouts, 5 wrong answers. Across Pi: 0 timeouts, 5 wrong answers.
- Report 19's "unsolvable trio" is harness-dependent: Pi solves networkx-leiden at low with no answer-key flag. Pennylane at high/xhigh did touch hidden tests; treat those two as tainted.
- Time per task: uniform loop 7 to 29 min and climbing; Pi 1.5 to 4 min.
- Pi bills ~1 K to 5 K tokens/task vs 206 K to 658 K on the uniform loop.

## Caveats to keep handy (if pressed)

- One attempt per cell, so within-Pi high vs xhigh is a tie; the xhigh harness gap is larger than the uncertainty, and the timeout-vs-wrong split is categorical.
- This is model plus agent harness, not two model APIs. That is the whole point: the delta is the harness.
- Integrity: 17/69 Pi cells flagged for host filesystem walks; 6 touched gold or hidden tests. Low column has zero answer-key cells (95.7%).
