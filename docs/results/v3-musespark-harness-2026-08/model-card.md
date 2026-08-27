# VulcanBench Technical Report No. 20, Muse Spark 1.2: model versus harness

**Harness Study No. 04 · August 27, 2026 · VulcanBench v3 · 23 tasks · 138 scored cells · 2 harnesses · 3 effort levels · 5 languages · $56.36 uniform loop; six confined Pi rerun cells $30.25, remaining Pi cells under-metered (see Caveats)**

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

| Effort | pass@1 | Solved | Wrong | Unfinished | Time/task* | Tokens moved/task* |
|---|---|---|---|---|---|---|
| **low** | **95.7%** | 22/23 | 1 | **0** | 1.5 min | 30 K |
| high | 87.0% | 20/23 | 3 | **0** | 3.0 min | 55 K |
| xhigh | 78.3% | 18/23 | 4 | 1 | 4.1 min | 58 K |

pass@1 is the per-task success rate at one attempt. Both harnesses ran the identical
23-task suite. These numbers carry **no asterisks**: the original 2026-08-27 sweep
ran Pi unconfined on the host and six cells read gold patches or hidden tests, so
those six cells were **replaced the same day by sandbox-confined reruns** (macOS
seatbelt denying every local benchmark tree), each audited clean of benchmark
paths. All six tainted "solves" changed outcome honestly: pennylane went to wrong
at high and xhigh, flask timed out at xhigh, sqlglot-qualify went to wrong at
xhigh, and aiohttp and packaging re-solved legitimately. \*Time and moved-token
column averages still describe the original sweep's cells; the six rerun cells
were heavier and slower (pennylane's honest attempts moved 5.0 M to 9.6 M tokens
in 14 to 27 min; per-cell figures in the JSON `rerun` block).

Pi's whole-column cash is **not quoted**: the original sweep recorded only Pi's
last per-message usage record instead of the sum across messages, so its dollar
figures understate the runs (v0.9.1 sums correctly; the JSON keeps the raw
sweep values under `*_lastmsg` names). The six rerun cells, metered correctly,
cost **$30.25 on their own**, an indication of scale for what the full 69-cell
Pi grid would really cost. The uniform loop cost $56.36 metered. Every run in
both tracks requested `muse-spark-1.2`. The sweep ran Pi 0.74.2; the rerun
cells ran Pi 0.75.5 with `contextWindow` declared (see Reproducibility). Wire
id in all cases: `vulcan-meta/muse-spark-1.2`. Judges were off (hidden-test
grading only).

The Pi agent runs on the host; Docker is the verifier. The integrity audit flagged
**17 of 69** original Pi cells for filesystem access outside the task workspace,
including 6 that touched gold patches or hidden tests; those six are the replaced
cells. The other 11 flags are `benchmark_data_access` (benchmark files, not this
task's answer key) and stand as flags, not exclusions. See Caveats.

## Findings

1. **Same model, and the harness is worth 8.7 to 26.1 points, no asterisks.**
   Pi beats the uniform loop at every matched effort: +8.7 at low (95.7 vs 87.0),
   +13.1 at high (87.0 vs 73.9), +26.1 at xhigh (78.3 vs 52.2). Every number is
   from an audited-clean cell: the six cells the unconfined sweep tainted were
   replaced by confined reruns before publication. Judged only by its xhigh
   uniform-loop setting, Muse Spark looks like a 52% model. Judged through Pi at
   low, it is a 96% model.

2. **The effort knob points backward on both harnesses, but only one falls off a
   cliff.** On the uniform loop it inverts hard, 87.0 to 73.9 to 52.2, the
   steepest backward knob measured on v3 (Report No. 19). In Pi the same knob is
   a gentler slide, 95.7 to 87.0 to 78.3. Meta's `reasoning.effort` enum is the
   same in both (`low` / `high` / `xhigh`). Low is the best setting on both
   harnesses; the harness decides how much the higher settings cost you.

3. **The gap is still mostly unfinished work.** The uniform loop's failures at
   high and xhigh are dominated by wall-clock timeouts: 0, then 5, then 10
   unfinished runs as effort climbs, against 3, 1, 1 incorrect completions. Pi
   is nearly the mirror image: one timeout in 69 cells, every other failure a
   finished wrong answer, typically in single-digit minutes. This is the
   "deficit is unfinished, not bad" pattern Reports No. 12 and 18 documented;
   a second harness on this model almost removes it.

4. **Confined, the model hunts for the answer key, and it costs real budget.**
   The reruns are direct evidence that the unconfined "solves" were retrieval,
   not reasoning: every rerun stream shows Muse Spark probing the host for
   benchmark data (`find / -name`, sweeping `/tmp`, walking toward the
   VulcanBench checkout through its `/System/Volumes/Data` alias) and being
   refused by the sandbox. The one Pi timeout on the board is exactly this:
   flask at xhigh spent its whole 20-minute budget running `find / -name
   "*.py"` instead of fixing Flask. An unconfined rerun before the sandbox was
   in place read the hidden tests again within minutes, so this is systematic
   behavior, not a one-off. Any host-run harness result for this model without
   filesystem confinement should be assumed contaminated.

5. **Pennylane is not solvable by this model in either harness; the trio mostly
   holds.** Report No. 19's uniform loop never solved `networkx-leiden-communities`,
   `pennylane-trotter-fragmented`, or `sqlglot-canonicalize-internal-names`.
   Confined Pi solves `networkx-leiden` at low and `sqlglot-canonicalize` at all
   three levels (that task's cells carry benchmark-data flags from the original
   sweep, not answer-key flags). Pennylane's unconfined "solves" did not survive:
   honest confined attempts finish at functional 0.4 (wrong) at high and xhigh,
   after moving 5.0 M to 9.6 M tokens, the heaviest Pi runs measured.

6. **Lighter in tokens and faster on the clock; cash is real money once metered
   correctly.** The uniform loop cost $56.36 across three columns and got slower
   with effort. Pi's original sweep columns held 1.5 to 4.1 min/task and moved
   ~30 K to 58 K cache-inclusive tokens/task against the loop's 206 K to 658 K.
   But the correctly metered rerun cells show honest hard-task Pi runs are not
   cheap: the six cells cost $30.25 together (pennylane high alone $12.26). The
   original sweep's "$0.24 total" was an accounting artifact, not a price. Both
   tracks are metered API, unlike Report No. 18's ZCode subscription track, and
   still must not be averaged: one is model plus Vulcan's loop, the other is
   model plus Pi.

## Failure map

Eleven tasks moved between harnesses or across effort. The other twelve were solved
by both harnesses at all three levels. Cells marked **(rerun)** are the confined
replacements for the six answer-key cells of the unconfined sweep.

| Task | API low/high/xhigh | Pi low/high/xhigh |
|---|---|---|
| pennylane-trotter-fragmented | wrong / timeout / timeout | wrong / wrong (rerun) / wrong (rerun) |
| networkx-leiden-communities | wrong / timeout / timeout | **solved** / wrong / wrong |
| sqlglot-canonicalize-internal-names | wrong / timeout / timeout | solved / solved / solved |
| aiohttp-upgrade-deferred | solved / timeout / timeout | solved / solved / solved (rerun) |
| flask-teardown-robust | solved / timeout / timeout | solved / solved / timeout (rerun) |
| itertools-strip-prefix | solved / solved / timeout | solved / wrong / wrong |
| jiff-strftime-negpad | solved / solved / timeout | solved / solved / solved |
| packaging-range-prerelease-policy | solved / solved / timeout | solved / solved / solved (rerun) |
| semver-inc-dotted-prerelease | solved / solved / timeout | solved / solved / solved |
| semver-xrange-order | solved / solved / timeout | solved / solved / solved |
| sqlglot-qualify-lateral-star | solved / wrong / wrong | solved / solved / wrong (rerun) |

Read the two halves together. Where the uniform loop degrades, it degrades into
`timeout` (the cell goes blank on the clock, most often at xhigh). Where Pi
degrades, it almost always degrades into `wrong` (a finished, incorrect answer).
The single Pi timeout, flask at xhigh, is the confined agent spending its budget
hunting the host for benchmark files (Finding 4).

Of the uniform loop's 15 unfinished runs across the three columns, all hit the
task wall-clock boundary in `harness/task_metadata.py`; none was cost-capped or
hit the step ceiling. The clock is what binds, and xhigh reasoning is what spends
it.

## Caveats

- **One attempt per cell.** Like Reports No. 18 and 19, this comparison is a single
  attempt per task per effort per harness (138 scored cells). At 23 tasks the
  pass@1 standard error is roughly plus or minus 4 to 10 points per column. The
  xhigh-vs-xhigh harness gap (26.1 points) is larger than the combined
  uncertainty; the low gap (8.7) is within one combined stderr and should be read
  as directional. The timeout-vs-wrong split is categorical, not marginal.
- **This measures model plus agent harness, not two model APIs.** The Pi column
  includes Pi's system prompt, tools (`read`, `write`, `edit`, `bash`), and
  turn-taking. It is not comparable to an `anthropic:` or `openai:` uniform-loop
  column and must never be added to a raw-API leaderboard. As of v0.9.1 the CLI
  enforces this: `vulcanbench leaderboard --track api` filters out rows from
  API-metered CLI harnesses (`pi:`), which appear only under `--track all`. The
  value of the report is the delta between the two rows for one fixed model.
- **Pi runs on the host; the six answer-key cells were replaced.** Publication
  used `--sandbox docker` so hidden tests execute in the task image, the same
  verifier as Report No. 18's ZCode column. The original sweep's agent was not
  kernel-sandboxed, and its audit flagged 17/69 cells, 6 with answer-key access
  (pennylane at high and xhigh; aiohttp, flask, packaging, and sqlglot-qualify
  at xhigh). Those six were replaced on 2026-08-27 by reruns under a macOS
  seatbelt profile (`sandbox-exec`) that denies the agent process tree all reads
  and writes of every local benchmark checkout (both path aliases) plus /tmp
  FIFO hazards; each accepted rerun's audit shows no benchmark or answer-key
  paths and no web use. The 11 remaining `benchmark_data_access` flags on other
  original cells (for example sqlglot-canonicalize) stand as flags; rerunning
  those 11 under confinement is the outstanding follow-up.
- **Fixed budget, not a capability ceiling.** With unlimited wall clock the uniform
  loop might finish more of what it currently times out on. Budgets (20 to 60 min,
  scaled by repo size) are identical for every model and harness on the board and
  unchanged since Report No. 07; agents in production run under a clock.
- **Original-sweep Pi cash and billed tokens were under-metered.** Pi reports
  usage per assistant message; the sweep's harness recorded the last record per
  run instead of summing them, so its per-run billed-token and dollar figures
  understate the true totals (a 40-step run showing 0.6 K "billed" tokens is
  the tell). VulcanBench 0.9.1 sums per-message usage, and the six rerun cells
  are metered correctly ($30.25 together; per-cell in the JSON `rerun` block).
  A full re-sweep is still needed before quoting whole-column Pi cash. The JSON
  preserves the sweep's under-metered values as `tokens_lastmsg_per_task` /
  `cost_lastmsg_usd` so they cannot be mistaken for run totals; the
  cache-inclusive `tokens_moved_per_task` volume comes from the stream itself
  and is unaffected. Uniform-loop tokens and cost are metered normally. Both
  tracks are API-metered; neither is a subscription quota.
- **Rerun cells differ from sweep cells in three declared ways.** Pi 0.75.5
  instead of 0.74.2; `contextWindow: 262144` declared in the generated
  models.json (Pi otherwise assumes 128 K and force-compacts, which crashed two
  pennylane attempts on Pi's "Cannot continue from message role: assistant"
  compaction-resume bug); and seatbelt confinement. One pennylane xhigh attempt
  was additionally retried after hanging on a host-machine FIFO in /tmp during
  an answer-hunt (infrastructure, not a model outcome).

## Reproducibility

Uniform-loop traces under the Report No. 19 run set. The original Pi sweep ran
2026-08-27 in the Cursor agent's environment; its raw traces are **not** in this
repository's `runs/` tree, so audit of the non-rerun cells rests on the committed
aggregate JSON (including the per-cell integrity flags). The six rerun cells'
full traces, patches, replay HTML, and `integrity_audit` blocks are local under
`runs/pi-rerun-2026-08/` (discarded unconfined attempts under
`discarded-unconfined/` there). Importing the original raw run set, or a full
confined re-sweep with the 0.9.1 accounting, is the open follow-up. Muse Spark 1.2
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

# Pi, open-source harness wrapping the same model (Report 18 ZCode recipe).
# Confine the agent first: put a `pi` wrapper OUTSIDE the repo (the harness
# scrubs repo-rooted PATH entries) that execs
#   sandbox-exec -f <profile.sb> <real pi> "$@"
# with a profile denying file-read*/file-write* on every local benchmark
# checkout, then prepend the wrapper dir to PATH.
vulcanbench run --suite v3 --model pi:meta:muse-spark-1.2 --effort <low|high|extra-high> \
  --repeat 1 --no-judges --sandbox docker

# Audit every CLI-harness run before reporting it.
vulcanbench audit-runs <output-dir>
```

`vulcanbench leaderboard --track api` filters `pi:` rows out (v0.9.1), so they
cannot appear as a second Muse Spark board entry. Compare them as a harness pair,
as this report does.
