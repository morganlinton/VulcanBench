# VulcanBench Technical Report No. 12 — Qwen3.8-Max across the effort knob

**August 4, 2026 · VulcanBench v3 · 23 tasks · 202 runs · 3 effort levels · 5 languages · $126.25**

First measurement of Alibaba's Qwen3.8-Max on the full v3 suite. Twenty-three real merged
post-cutoff PRs (Python 9, TypeScript 4, Rust 4, Go 3, JavaScript 3), graded by hidden
deterministic tests in a network-isolated Docker sandbox. Three attempts per task per effort
level — the first debut on VulcanBench measured at repeat-3 rather than a single attempt.

## Results

| Effort | pass@1 | Solved | Wrong | Unfinished | Cost | Tokens/task | Time/task | Steps | $/task | $/solved |
|---|---|---|---|---|---|---|---|---|---|---|
| **low** | **81.2%** | 56/69 | 3 | **10** | $42.09 | 188 K | 19.6 min | 179 | $0.61 | **$0.75** |
| medium | 71.0% | 49/69 | **0** | 20 | $38.81 | 157 K | 21.7 min | 152 | $0.56 | $0.79 |
| xhigh *(default)* | 55.1% | 38/64 | **0** | **26** | $45.35 | 208 K | 25.5 min | 167 | $0.71 | $1.19 |

pass@1 is the mean per-task success rate across attempts, so uneven attempt counts don't bias
it. Read the score column together with the last two: the deficit is unfinished work, not bad
work. The xhigh column has 64 runs rather than 69 because five died on 600-second API read
timeouts (below) and are excluded rather than scored zero.

Qwen's documented `reasoning_effort` enum is **low / medium / xhigh** — there is no `high` —
and **xhigh is the default** when the field is unset.

## Findings

1. **The knob runs backwards, and steeply.** Low leads xhigh by 26 points, more than triple the
   9-point inversion Report No. 10 recorded for Claude Opus 5. At three attempts per task the
   standard errors are ±7.8, ±9.4 and ±9.7 points and low-vs-xhigh does not overlap, so this is
   not noise. Because xhigh is the API default, an untuned integration gets the worst setting.

2. **Effort converts wrong answers into unfinished runs.** Wrong answers fall to zero
   (3 → 0 → 0) while runs that hit a budget climb (10 → 20 → 26, or 14% → 29% → 41%). At medium
   and xhigh *every single failure* is an incomplete run. Within a level, failed runs burn ~5×
   the completion tokens of successful ones (115–140 K vs 22–29 K) and 3× the clock (40–46 min
   vs 13–15).

3. **The regression is in solvable work.** Three tasks score zero at every setting — extra
   reasoning rescues none of them. Nine tasks regress from low to xhigh, and the six that low
   solves 3-for-3 account for **83%** of the 26-point drop, three collapsing to zero. It is not
   losing the hard problems. It is losing the ones it already knows how to do.

4. **Most expensive, slowest, beaten on every axis.** $126.25 against DeepSeek V4-Flash's $13.60
   on the identical suite. Every DeepSeek column (88.4 / 87.3 / 85.5) outscores every Qwen
   column (81.2 / 71.0 / 55.1) with no overlap, at $0.08 per solved task vs $0.75–1.19. At
   19.6–25.5 min/task it is the slowest model measured on v3, ahead of Kimi K3 at 17.2.

## Failure map

Nine tasks moved. The other fourteen scored identically at all three settings.

| Task | low | medium | xhigh |
|---|---|---|---|
| jiff-strftime-negpad | 3/3 | 3/3 | **0/3** — 3 unfinished |
| semver-inc-dotted-prerelease | 3/3 | **0/3** — 3 unfinished | **0/3** — 3 unfinished |
| sqlglot-qualify-lateral-star | 3/3 | 1/3 — 2 unfinished | **0/3** — 3 unfinished |
| packaging-range-prerelease-policy | 3/3 | 3/3 | 1/3 — 2 unfinished |
| semver-xrange-order | 3/3 | 3/3 | 1/3 — 2 unfinished |
| zod-invert-codec | 3/3 | 3/3 | 1/3 — 2 unfinished |
| itertools-strip-prefix | 3/3 | 3/3 | 2/3 — 1 unfinished |
| flask-teardown-robust | 1/3 — 2 unfinished | 0/3 — 3 unfinished | 0/3 — 3 unfinished |
| networkx-leiden-communities | 1/3 — 2 unfinished | 0/3 — 3 unfinished | 0/1 — 1 unfinished |

Never solved at any setting: `aiohttp-upgrade-deferred`, `pennylane-trotter-fragmented`,
`sqlglot-canonicalize-internal-names`.

Cells are solved/attempted. Of the 56 unfinished runs, 38 died at the 45-minute (large) or
60-minute (xlarge) wall-clock boundary (`harness/task_metadata.py`), only three hit the step
ceiling, and none was cost-capped. The clock, not the step budget, is what binds.

## A second clock: 600-second API read timeouts

Five xhigh runs, on the suite's three heaviest repositories, failed with read timeouts from the
DashScope endpoint after 600 seconds — a single request exceeding ten minutes before returning
any content. No previous report has recorded this failure mode. It is imposed by the provider,
not by VulcanBench, and it sits well inside the harness's own 45- and 60-minute budgets.

The harness records these as run **errors** rather than scoring them zero — the same treatment
safety refusals receive — so they are excluded from the xhigh column rather than counted against
the model. Retrying reproduced the timeouts. At the effort level Alibaba ships as the default,
on the largest repositories in the suite, the model can exceed its own provider's response
window.

## Caveats

- **Fixed budget, not a capability ceiling.** With unlimited time xhigh might close some of the
  gap. This measures capability under a fixed budget, which is how agents run in production.
  Budgets (50–200 steps, 5–60 min, scaled by repo size) are identical for every model on the
  board and unchanged since Report No. 07.
- **`thinking_budget` untested.** Qwen exposes a thinking-token cap that is the knob most likely
  to address the pattern documented here, but its API rejects that parameter alongside
  `reasoning_effort`. A follow-up should test it.
- **Asymmetric rigor, in Qwen's favour.** Every column here is three attempts per task. The
  single-attempt columns established in Reports No. 07, 08 and 10 carry wider uncertainty by
  comparison; this model has the most thoroughly measured debut on the board.

## Reproducibility

Traces, patches, and replay HTML under `runs/` (suite `v3`). Qwen3.8-Max priced at $2/$6 per
million tokens on the international (Singapore) endpoint. Effort mapping added in PR #35:
`low`→`low`, `medium`→`medium`, `extra-high`→`xhigh`; `high` is recorded but never sent, because
Qwen has no such level.

```
vulcanbench run --suite v3 --model qwen:qwen3.8-max --effort <low|medium|extra-high> --repeat 3
```
