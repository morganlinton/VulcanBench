# VulcanBench Technical Report No. 19, Muse Spark 1.2 across the effort knob

**August 25, 2026 · VulcanBench v3 · 23 tasks · 69 runs · 3 effort levels · 5 languages · $56.36**

First measurement of Meta's Muse Spark 1.2 on the full v3 suite, through the Meta Model
API and VulcanBench's uniform agent loop. Twenty-three real merged post-cutoff PRs
(Python 9, TypeScript 4, Rust 4, Go 3, JavaScript 3), graded by hidden deterministic
tests in a network-isolated Docker sandbox. One attempt per task per effort level, the
same debut protocol as Reports No. 17 and 18.

## Results

| Effort | pass@1 | Solved | Wrong | Unfinished | Cost | Tokens/task | Time/task | $/task | $/solved |
|---|---|---|---|---|---|---|---|---|---|
| **low** | **87.0%** | 20/23 | 3 | **0** | $8.19 | 206 K | 6.7 min | $0.36 | **$0.41** |
| high | 73.9% | 17/23 | 1 | 5 | $20.92 | 483 K | 21.3 min | $0.91 | $1.23 |
| xhigh | 52.2% | 12/23 | 1 | **10** | $27.25 | 658 K | 28.7 min | $1.18 | $2.27 |

pass@1 is the per-task success rate at one attempt; time/task is the mean (medians run
3.2 / 12.4 / 20.1 minutes, the tail is failed runs). Meta's documented
`reasoning.effort` enum is **minimal / low / medium / high / xhigh**; an unset request
reasons at "a model-determined level" Meta does not name, so there is no measured
default column. The integrity audit flagged zero contaminated runs and every run's
effort metadata confirms the requested level was sent and accepted.

## Findings

1. **The steepest backward knob measured on v3.** Low leads xhigh by **34.8 points**,
   ahead of Qwen3.8-Max's 26.1 (Report No. 12), GLM 5.3's 13.1 on the raw API (Report
   No. 18), and Opus 5's 9 (Report No. 10). At one attempt per cell the per-column
   standard errors are roughly ±7 to ±10 points; the low-vs-xhigh gap is more than
   triple that and does not overlap. The pattern is now four models deep: on this suite,
   dialing up API reasoning effort makes agents worse, and the makers' higher settings
   are consistently the wrong ones.

2. **Effort converts wrong answers into timeouts, and the mechanism is visible in the
   per-step numbers.** At low, every failure is a finished wrong answer (3 wrong, 0
   unfinished). By xhigh the composition flips: 1 wrong, 10 unfinished (0 → 5 → 10
   timeouts as wrong answers fall 3 → 1 → 1). What a timeout means here: every task
   carries a fixed wall-clock budget scaled by repo size (20-, 45-, or 60-minute tier
   in `harness/task_metadata.py`), **identical for every model on the board and
   unchanged since Report No. 07**; all 15 unfinished runs ran that clock out, none hit
   the step ceiling, and none was cost-capped. The cause is not harder work but slower
   steps: at higher effort the model emits 2 to 33x the output tokens per agent step on
   the *same task* (`itertools-strip-prefix`: ~140 tokens/step at low, solved in 7
   minutes; ~4,600 tokens/step at xhigh, killed at the 60-minute wall). Eight of the
   fifteen timed-out cells are tasks the same model solves at low in 1.6 to 12 minutes.
   And the harness grades partial work at kill time, so these are not near-misses: in
   10 of the 15 timeouts the captured patch was zero bytes. The model reasoned for its
   entire budget and never wrote a line to disk.

3. **At low effort this is a board-tying debut.** 87.0% matches the four-model 87.0%
   cluster on the Eval Suite 3 board (DeepSeek V4 Pro, GPT-5.6 Terra, GPT-5.6 Sol,
   Claude Opus 5, Grok 4.6), behind only Grok 4.5 (89.9), Claude Fable 5 (89.5), and
   DeepSeek V4-Flash (88.4). At $0.36/task and $0.41/solved it lands near the cheap end
   of the board's frontier, and its 3.2-minute median makes the solved runs among the
   fastest measured. Judged at xhigh instead, the same model looks mid-board at 52.2%:
   a 35-point identity swing on one undocumented default away.

4. **The unsolvable trio holds.** `networkx-leiden-communities`,
   `pennylane-trotter-fragmented`, and `sqlglot-canonicalize-internal-names` score zero
   at every setting, the same three tasks that survived Qwen3.8-Max at all efforts and
   that GLM 5.3's raw API never finished. Muse Spark at low at least *finishes* all
   three (wrong answers, not hangs); at high and xhigh it times out on all of them.
   Extra reasoning rescues none of the suite's hardest tasks for any model measured so
   far; it only endangers the solvable ones.

5. **The regression is in solved work.** Eight tasks that low solves outright degrade at
   xhigh, seven of them into timeouts (`aiohttp-upgrade-deferred`,
   `flask-teardown-robust`, `itertools-strip-prefix`, `jiff-strftime-negpad`,
   `packaging-range-prerelease-policy`, `semver-inc-dotted-prerelease`,
   `semver-xrange-order`) and one into a finished wrong answer
   (`sqlglot-qualify-lateral-star`). Those eight account for the entire 34.8-point drop.
   As with Qwen, the model is not losing the hard problems at high effort; it is losing
   the ones it already knows how to do.

## Failure map

Eleven tasks moved across the three settings. The other twelve scored identically at
all three (nine solved everywhere, three never solved).

| Task | low | high | xhigh |
|---|---|---|---|
| aiohttp-upgrade-deferred | solved | timeout | timeout |
| flask-teardown-robust | solved | timeout | timeout |
| itertools-strip-prefix | solved | solved | timeout |
| jiff-strftime-negpad | solved | solved | timeout |
| packaging-range-prerelease-policy | solved | solved | timeout |
| semver-inc-dotted-prerelease | solved | solved | timeout |
| semver-xrange-order | solved | solved | timeout |
| sqlglot-qualify-lateral-star | solved | wrong | wrong |
| networkx-leiden-communities | wrong | timeout | timeout |
| pennylane-trotter-fragmented | wrong | timeout | timeout |
| sqlglot-canonicalize-internal-names | wrong | timeout | timeout |

Never solved at any setting: `networkx-leiden-communities`,
`pennylane-trotter-fragmented`, `sqlglot-canonicalize-internal-names`.

Read the columns left to right: low fails by answering wrong, high and xhigh fail by
running out the clock. There is no task anywhere in the matrix that xhigh solves and
low does not.

## Caveats

- **One attempt per cell.** Like Report No. 18's columns, this is a single-attempt
  debut (69 runs); per-column uncertainty is ±7 to ±10 points, so the within-curve
  ordering of high vs xhigh is suggestive while the low-vs-xhigh inversion and the
  wrong-vs-timeout composition are categorical. A repeat-3 rerun would sharpen the
  middle of the curve.
- **`minimal` and `medium` untested.** The sweep measured three of Meta's five effort
  levels. `minimal` in particular might extend the pattern's cheap end and is worth a
  follow-up column.
- **No default column.** Meta does not document what an unset `reasoning.effort` runs
  at, so unlike Qwen (xhigh default) and GLM (max default) this report cannot say what
  an untuned integration gets, only that four of five nameable settings were not it.
- **Fixed budget, not a capability ceiling.** With unlimited wall clock, xhigh might
  finish more of what it times out on. Budgets (20 to 60 minutes, scaled by repo size)
  are identical for every model on the board and unchanged since Report No. 07; agents
  in production run under a clock.

## Reproducibility

Traces, patches, and replay HTML under `runs/` (suite `v3`). Muse Spark 1.2 priced at
$1.25 input / $4.25 output per million tokens on the Meta Model API (implicit cache
reads at $0.15/M, factor 0.12). Effort mapping: `low`→`low`, `high`→`high`,
`extra-high`→`xhigh`. Judges were off (hidden-test grading only, the documented
publication protocol). The low column is 18 fresh runs plus 5 reused by
`--only-missing` from an identical 2026-08-12 invocation.

```
vulcanbench run --suite v3 --model meta:muse-spark-1.2 --effort <low|high|extra-high> \
  --repeat 1 --no-judges
```

Same model through Pi (harness delta; still the api track):

```
npm install -g @earendil-works/pi-coding-agent
vulcanbench harness doctor pi
vulcanbench run --suite v3 --harness pi --billing api \
  --model meta:muse-spark-1.2 --effort <low|high|extra-high> \
  --repeat 1 --no-judges
vulcanbench leaderboard --track api
```
