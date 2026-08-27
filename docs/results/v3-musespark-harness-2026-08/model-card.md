# VulcanBench Technical Report No. 20, Muse Spark 1.2: model versus harness

**Harness Study No. 04 · August 27, 2026 · VulcanBench v3 · 23 tasks · 138 runs · 2 harnesses · 3 effort levels · 5 languages · $56.36 uniform loop; Pi cash under-metered in this sweep (see Caveats)**

Second measurement of Meta's Muse Spark 1.2 on the full v3 suite, run two ways
against the same 23 tasks: through VulcanBench's uniform agent loop on the Meta
Model API (Report No. 19), and through **Pi**, the open-source coding agent, wrapping
the same `meta:muse-spark-1.2` spec. Same twenty-three real merged post-cutoff PRs
(Python 9, TypeScript 4, Rust 4, Go 3, JavaScript 3), same hidden deterministic
tests in a network-isolated Docker verifier, same effort levels (low, high, xhigh).
One attempt per task per effort per harness. The question is not how good Muse Spark
is. It is how much the agent loop around it changes the answer.

## Results

**Raw API (`meta:muse-spark-1.2`), VulcanBench uniform loop, metered (Report No. 19):**

| Effort | pass@1 | Solved | Wrong | Unfinished | Cost | Tokens/task | Time/task | $/solved |
|---|---|---|---|---|---|---|---|---|
| **low** | **87.0%** | 20/23 | 3 | **0** | $8.19 | 206 K | 6.7 min | $0.41 |
| high | 73.9% | 17/23 | 1 | 5 | $20.92 | 483 K | 21.3 min | $1.23 |
| xhigh | 52.2% | 12/23 | 1 | **10** | $27.25 | 658 K | 28.7 min | $2.27 |

**Pi (`pi:meta:muse-spark-1.2`), open-source harness, metered API:**

| Effort | pass@1 (clean) | pass@1 (as graded) | Solved | Wrong | Unfinished | Time/task | Tokens moved/task |
|---|---|---|---|---|---|---|---|
| **low** | **95.7%** | 95.7% | 22/23 | 1 | **0** | 1.5 min | 30 K |
| high | 87.0% | 91.3% | 21/23 | 2 | **0** | 3.0 min | 55 K |
| xhigh | 69.6% | 91.3% | 21/23 | 2 | **0** | 4.1 min | 58 K |

pass@1 is the per-task success rate at one attempt. Both harnesses ran the identical
23-task suite. **Clean** drops from the numerator (n stays 23) the six Pi cells whose
integrity audit recorded reads of gold patches or hidden tests: 1 at high, 5 at
xhigh, 0 at low. **As graded** counts them; the card leads with clean.

Pi's cash and billed-token figures from this sweep are **not quoted**: the harness
recorded Pi's last per-message usage record instead of the sum across messages, so
they understate the runs (v0.9.1 fixes the accounting for future sweeps; the JSON
keeps the raw values under `*_lastmsg` names). What is trustworthy is volume and
time: Pi moved ~30 K to 58 K cache-inclusive tokens/task against the uniform loop's
206 K to 658 K, in 1.5 to 4.1 min/task against 6.7 to 28.7. The uniform loop cost
$56.36 metered. Every run in both tracks requested `muse-spark-1.2`. Pi 0.74.2
reported the wire id `vulcan-meta/muse-spark-1.2`. Judges were off (hidden-test
grading only).

The Pi agent runs on the host; Docker is the verifier. The integrity audit flagged
**17 of 69** Pi cells for filesystem access outside the task workspace, including
**6 cells that touched gold patches or hidden tests**. Those six are what the clean
column excludes, and they are called out on the failure map. The low column is the
least confounded (0 answer-key cells). See Caveats.

## Findings

1. **Same model, and the harness is worth 17 points at xhigh, cleanly measured.**
   At matched `xhigh` effort, Pi's clean column scores 69.6% against the uniform
   loop's 52.2%, a 17.4-point gap from scaffolding alone; counting the six
   answer-key cells as graded would stretch it to 39.1, which is why this card
   leads with the clean number. The cleanest comparison on the whole grid is low,
   where the audit flagged nothing: Pi 95.7% against the loop's 87.0%, and that
   Pi-at-low figure beats the uniform loop's best column outright. Judged only by
   its xhigh uniform-loop setting, Muse Spark looks like a 52% model. Judged
   through Pi at low, it is a 96% model.

2. **The effort knob points backward on both harnesses, but only one falls off a
   cliff.** On the uniform loop it inverts hard, 87.0 to 73.9 to 52.2, the steepest
   backward knob measured on v3 (Report No. 19). In Pi the as-graded line is high
   and flat (95.7 / 91.3 / 91.3); the clean line declines more gently (95.7 / 87.0 /
   69.6) and never times out. Meta's `reasoning.effort` enum is the same in both
   (`low` / `high` / `xhigh`). Low is the best setting on both harnesses; the
   harness decides how much the higher settings cost you.

3. **The gap is unfinished work, not worse work.** Almost every uniform-loop failure
   at high and xhigh is a wall-clock timeout: 0, then 5, then 10 unfinished runs as
   effort climbs, against 3, 1, 1 incorrect completions, while time per task climbs
   6.7 to 28.7 minutes. Pi is the mirror image: **zero timeouts at any level**, every
   failure a returned wrong answer (1, then 2, then 2), finishing in 1.5 to 4.1
   minutes. The uniform loop reasons until it runs out of budget, and more effort
   makes that worse. Pi drives every run to a finished answer. This is the same
   "deficit is unfinished, not bad" pattern Reports No. 12 and 18 documented; a
   second harness on this model removes it entirely.

4. **The unsolvable trio is harness-dependent.** Report No. 19's uniform loop never
   solved `networkx-leiden-communities`, `pennylane-trotter-fragmented`, or
   `sqlglot-canonicalize-internal-names` at any setting. Pi solves `networkx-leiden`
   at low (no answer-key flag), `sqlglot-canonicalize` at all three levels (filesystem
   audit: benchmark-data paths, not hidden tests), and `pennylane-trotter` at high and
   xhigh **after touching gold patches and hidden tests**, so those two pennylane
   solves are not a clean capability claim. The honest pennylane cell is low: a
   finished 0.2, not a hang. On the two tasks Pi actually misses (`itertools-strip-prefix`
   and `networkx-leiden` at high and xhigh), it misses as a wrong answer in a few
   minutes, not as a 60-minute blank.

5. **Lighter in tokens and faster on the clock; cash not yet cleanly metered.**
   The uniform loop cost $56.36 across three columns and got slower with effort.
   Pi held 1.5 to 4.1 min/task and moved ~30 K to 58 K cache-inclusive tokens/task
   against the uniform loop's 206 K to 658 K, an order of magnitude lighter. Pi's
   exact cash is withheld: this sweep recorded only the last per-message usage
   record per run (see Caveats), so its dollar figures understate. Both columns
   are metered API, unlike Report No. 18's ZCode subscription track, and still
   must not be averaged: one is model plus Vulcan's loop, the other is model
   plus Pi.

## Failure map

Eleven tasks moved between harnesses or across effort. The other twelve were solved
by both harnesses at all three levels.

| Task | API low/high/xhigh | Pi low/high/xhigh |
|---|---|---|
| pennylane-trotter-fragmented | wrong / timeout / timeout | wrong / **solved*** / **solved*** |
| networkx-leiden-communities | wrong / timeout / timeout | **solved** / wrong / wrong |
| sqlglot-canonicalize-internal-names | wrong / timeout / timeout | solved / solved / solved |
| aiohttp-upgrade-deferred | solved / timeout / timeout | solved / solved / solved* |
| flask-teardown-robust | solved / timeout / timeout | solved / solved / solved* |
| itertools-strip-prefix | solved / solved / timeout | solved / wrong / wrong |
| jiff-strftime-negpad | solved / solved / timeout | solved / solved / solved |
| packaging-range-prerelease-policy | solved / solved / timeout | solved / solved / solved* |
| semver-inc-dotted-prerelease | solved / solved / timeout | solved / solved / solved |
| semver-xrange-order | solved / solved / timeout | solved / solved / solved |
| sqlglot-qualify-lateral-star | solved / wrong / wrong | solved / solved / solved* |

\* Pi cell whose integrity audit recorded `answer_key_access` (gold patch and/or
hidden tests on the host). Graded as solved; not a clean harness-delta claim.

Read the two halves together. Where the uniform loop degrades, it degrades into
`timeout` (the cell goes blank on the clock, most often at xhigh). Where Pi
degrades, it degrades into `wrong` (a finished, incorrect answer). There is no
Pi timeout anywhere in the matrix.

Of the uniform loop's 15 unfinished runs across the three columns, all hit the
task wall-clock boundary in `harness/task_metadata.py`; none was cost-capped or
hit the step ceiling. The clock is what binds, and xhigh reasoning is what spends
it.

## Caveats

- **One attempt per cell.** Like Reports No. 18 and 19, this comparison is a single
  attempt per task per effort per harness (138 runs total). At 23 tasks the pass@1
  standard error is roughly plus or minus 4 to 10 points per column, so the
  within-Pi high vs xhigh tie is noise. The xhigh-vs-xhigh harness gap (17.4
  points clean, 39.1 as graded) is larger than the combined uncertainty. The
  timeout-vs-wrong split is categorical, not marginal.
- **This measures model plus agent harness, not two model APIs.** The Pi column
  includes Pi's system prompt, tools (`read`, `write`, `edit`, `bash`), and
  turn-taking. It is not comparable to an `anthropic:` or `openai:` uniform-loop
  column and must never be added to a raw-API leaderboard. As of v0.9.1 the CLI
  enforces this: `vulcanbench leaderboard --track api` filters out rows from
  API-metered CLI harnesses (`pi:`), which appear only under `--track all`. The
  value of the report is the delta between the two rows for one fixed model.
- **Pi runs on the host.** Publication used `--sandbox docker` so hidden tests
  execute in the task image, the same verifier as Report No. 18's ZCode column.
  The agent itself is not kernel-sandboxed. The integrity audit flagged 17/69 Pi
  cells (`contaminated=True`), of which 6 touched gold patches or hidden tests
  (pennylane at high and xhigh; aiohttp, flask, packaging, and sqlglot-qualify at
  xhigh). Low has zero answer-key cells. A follow-up sweep should deny the
  `tasks/` tree from the agent workspace.
- **Fixed budget, not a capability ceiling.** With unlimited wall clock the uniform
  loop might finish more of what it currently times out on. Budgets (20 to 60 min,
  scaled by repo size) are identical for every model and harness on the board and
  unchanged since Report No. 07; agents in production run under a clock.
- **Pi's cash and billed tokens were under-metered in this sweep.** Pi reports
  usage per assistant message; the harness recorded the last record per run
  instead of summing them, so the sweep's per-run billed-token and dollar
  figures understate the true totals (a 40-step run showing 0.6 K "billed"
  tokens is the tell). VulcanBench 0.9.1 sums per-message usage; a re-sweep is
  needed before quoting Pi cash. The JSON preserves the under-metered values as
  `tokens_lastmsg_per_task` / `cost_lastmsg_usd` so they cannot be mistaken for
  run totals. The cache-inclusive `tokens_moved_per_task` volume comes from the
  stream itself and is unaffected. Uniform-loop tokens and cost are metered
  normally. Both tracks are API-metered; neither is a subscription quota.

## Reproducibility

Uniform-loop traces under the Report No. 19 run set. The Pi sweep ran 2026-08-27
in the Cursor agent's environment; its raw traces, patches, and replay HTML are
**not** in this repository's `runs/` tree, so per-run audit currently rests on the
committed aggregate JSON (including the per-cell integrity flags). Importing the
raw run set, or re-sweeping with the 0.9.1 accounting and a confined workspace, is
the open follow-up. Muse Spark 1.2
priced at $1.25 input / $4.25 output per million tokens on the Meta Model API
(implicit cache reads at $0.15/M, factor 0.12). Effort mapping: `low`→`low`,
`high`→`high`, `extra-high`→`xhigh` (Pi `--thinking`). Judges were off. Extra-high
retried four Pi runs after infrastructure errors (`semver-truncate`,
`semver-xrange-order`, `sqlglot-iso8601-nanos`); scored replacements are in the
matrix. Abandoned dirs without `summary.json` are not missing cells.

```
# Raw API, VulcanBench uniform loop (metered cash). Default --sandbox docker.
vulcanbench run --suite v3 --model meta:muse-spark-1.2 --effort <low|high|extra-high> \
  --repeat 1 --no-judges

# Pi, open-source harness wrapping the same model (Report 18 ZCode recipe)
vulcanbench run --suite v3 --model pi:meta:muse-spark-1.2 --effort <low|high|extra-high> \
  --repeat 1 --no-judges --sandbox docker
```

`vulcanbench leaderboard --track api` filters `pi:` rows out (v0.9.1), so they
cannot appear as a second Muse Spark board entry. Compare them as a harness pair,
as this report does.
