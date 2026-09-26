# VulcanBench decisions

A running log of operating decisions that are not derivable from the code:
what was decided, the evidence, and when to revisit. Agents working on the
harness or on sweeps should read the entries that touch their area before
changing run conditions. Suite-level policy for v4 lives in
[tasks/coding-intelligence-index-v4/CHARTER.md](../tasks/coding-intelligence-index-v4/CHARTER.md);
entries here record the measurements behind those rules.

## 2026-09-25: GPT-6 Luna and Sol run on a pinned Codex CLI 0.155.0, ahead of Grok 4.7

### Decision

GPT-6 Luna (`gpt-6-luna`) and GPT-6 Sol (`gpt-6-sol`) are swept on
VulcanBench Frontier v4 at Low, Medium, High, Extra-high and Max, through
Codex on the ChatGPT Pro subscription, one run at a time
(`scripts/run_gpt6_luna_sol_v4.sh`, Luna first). These are new models, not
the GPT-5.6 Luna and Sol already on the board; outputs go to
`runs-effort-gpt6-luna/` and `runs-effort-gpt6-sol/`. Owner request in
chat, 2026-09-25. The owner chose to run them before the paused Grok 4.7
chain, which resumes after this chain exits.

These two columns run on Codex CLI 0.155.0, installed in its own prefix
(`~/.local/vulcanbench-codex-0.155.0`) and put first on PATH through
`CODEX_BIN_DIR` for this chain only. The global CLI stays at 0.153.4, so
every other Codex column and rerun is unchanged. The bump is disclosed as
a footnote on the GPT-6 Luna and Sol columns, as with the Claude Code bump
for Opus 5.5 (2026-09-22 entry); incumbent Codex columns are not
re-baselined.

### Evidence

The bump was not optional. On 0.153.4 both models are refused before any
work, while GPT-6 Astra answers on the same login:

    400 invalid_request_error: The 'gpt-6-luna' model is not supported
    when using Codex with a ChatGPT account.

One-line probes on 2026-09-25 (low effort, "Reply with the single word
OK."): 0.153.4 and 0.154.0 refused; 0.155.0, 0.155.1, 0.156.0, 0.156.1 and
0.157.0 answered for both models. 0.157.1 (published minutes earlier) does
not start on Node 22.11. So 0.155.0 is the lowest release that runs the
models, and the pin goes no further. The first launch on 0.153.4 produced
70 refused run folders and no summaries; they were moved out of the tree
and are not part of any cell.

List prices, checked 2026-09-25 at developers.openai.com/api/docs/pricing
and each model page (USD per 1M tokens, standard tier, short context):
Sol $2.00 input, $0.20 cached input, $10.00 output; Luna $0.10, $0.01,
$0.50. Prompts over 272K input tokens bill the whole request at 2x input
and cache, 1.5x output. Both models expose effort `none` to `max` in the
API; Codex lists low to max for Luna and adds ultra for Sol, which stays
blocked (`vulcanbench.toml`).

### Revisit when

- the global Codex CLI is upgraded for another reason: record which
  columns ran on which version;
- a GPT-6 Luna or Sol run fails in a way 0.153.4 columns never did.
## 2026-09-24: Verdict reports a diff-size baseline beside every model's ranking

### Decision

On Verdict v1's pass question, ranking fixes by lines changed alone is
reported as a baseline next to "always guessing", in the report, the data
file and Table 1 of the model card, with its cutoff fitted on the
development split like every other cutoff. A model's ranking is only read as
evidence that it reads code when it beats this heuristic, overall and within
size bands. Owner request, in chat, 2026-09-24.

### Evidence

- The owner asked what size of diffs the suite tests. Median fix: 108 lines
  changed (10th to 90th percentile 45 to 277), one file, about 2,900 input
  tokens for Jev. Pass rates climb steeply with size: on the published split,
  7% under 50 lines, 64% at 50 to 199, 90% at 200 or more.
- Lines changed alone ranks the 611 published pass questions at AUROC 0.785
  (bootstrap 0.745 to 0.823) and scores 65.1% at a development-fitted cutoff
  of 127 lines, above Jev on both (0.690, 62.8%). Jev's stated confidence
  tracks size (AUROC 0.816 for telling above-median fixes apart); in the
  50 to 199 line band, 417 fixes, Jev ranks at 0.558 against 0.813 for the
  GPT-6 Astra control.

### Revisit triggers

- A new source of fixes whose pass rate does not rise with size: keep the
  baseline, but expect it near 0.5.
- Other families (regression, four-way outcome) show the same size effect:
  add their size baselines before reading those rankings.

## 2026-09-23: integrity audit false flag cleared on published Claude columns

### Decision

The filesystem integrity audit marked Claude Code runs as
`benchmark_data_access` / `contaminated` whenever the agent read one of the
CLI's own background-command logs
(`/private/tmp/claude-<uid>/<slug>/<session>/tasks/<id>.output`), because it
treated any `/tasks/` path as the benchmark task tree. PR #148 fixed the
rule. On the owner's instruction (in chat, 2026-09-23) the audit field was
refreshed on every affected Claude run set. Annotation only: no score,
pass@1 or published column changes.

### What carried the false flag

- Published Opus 5 summaries (`docs/results/cii-v4-opus5-effort-2026-09/`):
  8 of 93 runs (high: queuecore, settlecore, matchcore, granarycore;
  medium: lodgecore; max: matchcore, cellarcore, snapcore). Their agent
  streams are not on this machine, so they were re-classified from the
  stored path lists under the new rule and carry a `reclassified` note.
- Fable 5.1 Frontier v4 sweep (`runs-effort/`, Claude Code 2.1.259 to
  2.1.261): 20 of 115 runs, fully re-audited from their streams.
- Opus 5.5 sweep in progress (`runs-effort-opus55/`): 37 of 82 runs, fully
  re-audited. Runs finished later by a chain on the pre-#148 code need the
  same refresh.

In every case the flagged paths were the run's own CLI logs; no run read a
gold patch, hidden tests, another task or `runs/`, and none used the web.
After the refresh all three sets report 0 contaminated runs.

## 2026-09-23: Verdict families get an answerability control before a model is blamed

### Decision

When a Verdict v1 family scores at or near the majority floor, it is not
read as a verdict on the model until a frontier model has been given
exactly the same inputs on the same items and shown that the family is
answerable. The control runs through a subscription CLI (Codex here, to stay
clear of the Claude subscription a concurrent sweep was using), in an empty
read-only directory with no tools, with its cutoff fitted on the development
split exactly as the model's was. It is published as a control note on the
report, never as a leaderboard column. While a control that could change the
reading of a published result is running, the results come off the site
entirely, card and data files included, and return together with the
control. Owner requests, in chat, 2026-09-23.

### Evidence

- The owner asked whether the suite was flawed after Jev and always
  guessing tied at 63% on the pass question. All 745 fixes come from
  binary-parity tasks whose hidden tests check undocumented behaviour of a
  retired program, and Jev sees only the bug report and the fix, so the
  question might have been unanswerable from its inputs.
- Control, GPT-6 Astra at high effort, on the 611 published pass questions:
  AUROC 0.866 (bootstrap 0.837 to 0.894) against Jev's 0.690 (0.648 to
  0.730), a gap of 0.13 to 0.22; 72.5% at a development-fitted cutoff and
  67.1% at 0.5, against a 62.7% floor and Jev's 62.8%. Within-task AUROC,
  item-weighted over 19 tasks, 0.905 against 0.727. The family is
  answerable, and Jev's shortfall is ranking as well as calibration.
- On the 134 development fixes, drawn from only 4 tasks, the gap looked
  small (0.817 against 0.764), and the page briefly said Jev's shortfall was
  calibration alone. The test split corrected that, which is why results
  were withheld rather than left up while the test-split control ran.

### What this touched

- `scripts/verdict-v1/run_llm_control.py` (new), `export_results.py`
  (control blocks with bootstrap intervals and development-fitted cutoffs),
  `docs/results/verdict-v1-jev-2026-09/` (report, data), and the site report,
  which was withheld on 2026-09-23 and restored with the final control.

### Revisit triggers

- The style family has no control yet; run one before reading its 0.962
  AUROC against any other model.
- A second decision model on the suite reuses the same control run; no need
  to re-query Astra unless the item set changes.

## 2026-09-23: typed-decision scoring reports ranking, not a fixed 0.5 cutoff

### Decision

VulcanBench Verdict v1 never reports a yes/no family's accuracy at a bare
0.5 cutoff as a headline. Every published family carries, at minimum, its
AUROC, the range of probabilities the model actually produced, and the
majority-label share. Where a decision has to be made from a yes/no
probability, the cutoff is fitted on the development split and named in the
report; the 0.5 figure may be shown beside it as a diagnostic, never alone.
Subgroup tables print the majority baseline for each subgroup. Owner
request, in chat, 2026-09-22, after the owner questioned a published result.

### Evidence

- The first Jev report (published 2026-09-22) led with "40.8% against a
  72.4% floor" and "it called every patch broken". Jev's probability that a
  patch passes runs 0.09 to 0.42 across all 611 test items, so a 0.5 cutoff
  sat outside its output range and no item could be answered "passes". The
  headline measured the decision rule, not the model.
- The ordering under that cutoff is informative: AUROC 0.690 on the pass
  question, 0.652 on regression, 0.651 macro on the four-way outcome and
  0.962 on the style pairs, against 0.5 for chance. With cutoffs fitted on
  the development split (0.17 and 0.72) accuracy is 62.8% and 93.1% against
  floors of 62.7% and 94.1%: level with the floor, not 32 points below it.
- The same flaw produced a second false finding. "Accuracy falls as the
  patch gets longer" inverted once the cutoff was fixed, because the share
  of patches that pass rises with size (54%, 89%, 90%); at a fitted cutoff
  each bucket's accuracy equals that bucket's majority baseline exactly.
- The preflight had already shown probabilities clustered at 0.51 to 0.76 on
  one family and a 0.22 mean on another. A printed probability range would
  have caught it before publication, which is why one is now mandatory.

### What this touched

- `harness/verdict/scoring.py` (`auroc`, `family_auroc`, `tuned_thresholds`,
  `majority_label_share`, `p_true_min`/`max`/`mean`,
  `accuracy_at_threshold`), `scripts/verdict-v1/export_results.py`,
  `scripts/verdict-v1/score_predictions.py`,
  `scripts/verdict-v1/make_jev_card.py`, `tests/test_verdict.py`,
  `docs/results/verdict-v1-jev-2026-09/` (report, data and card), and the
  site report, which was corrected in place with the withdrawn numbers kept
  visible beside the corrected ones.

### Revisit triggers

- A model whose probabilities do span 0.5 on these items: the fitted cutoff
  should land near 0.5, and a large gap between the two accuracy columns
  becomes the calibration signal to report.
- A second model on the suite: fit its cutoffs on the same development split
  and publish both columns for every model, so no model is flattered by a
  cutoff chosen after seeing the test items.

## 2026-09-22: Grok 4.7 runs on Cursor and Grok Build, queued serially behind Opus 5.5

### Decision

Grok 4.7 is swept on all three suites (Frontier v4, Routine v1, Safety v1)
on two harnesses, Cursor and Grok Build, so the harnesses can be compared
on identical tasks. Levels are Low, Medium, High and Extra-high; neither
harness exposes a max level for Grok 4.7. The owner considered running
alongside the Opus 5.5 chain and chose to keep sweeps serial (2026-09-13
entry), so `scripts/run_grok47_all_suites.sh` waits for the Opus chain to
finish. Owner request, in chat, 2026-09-22.

### Evidence

- Cursor lists `grok-4.7-low`, `-medium`, `-high` and `-xhigh` (plus
  `-fast` tiers, not used). The adapter's bracket form
  (`grok-4.7-low[effort=low]`, `grok-4.7[effort=high]`) is rejected with
  "Cannot use this model", so Cursor legs pass the per-level id and no
  `--effort`, as the Grok 4.6 Cursor sweep did.
- Grok Build 1.0.41 lists `grok-4.7` (default); a one-line probe at
  `--reasoning-effort xhigh` answered on subscription login.

## 2026-09-19: VulcanBench Verdict v1 scores typed decisions against executed tests

### Decision

VulcanBench Verdict v1 (suite id `verdict-v1`) is a separate suite for
models that return a typed decision (a choice, an ordered score, or a yes/no
probability) and cannot write code, starting with TypeSafe AI's Jev. It is
never a column on the Frontier v4 board. Rules:

- Headline numbers use only items whose answer comes from the hidden-test
  verifier or a gold patch. Agreement with the code-quality judges
  (`quality-preference`, reference `llm-panel`) is reported apart and never
  folded into the overall score.
- Accuracy is always published next to calibration (Brier, log loss, ECE)
  and the majority-answer floor. Unanswered items count as wrong.
- Items embed Frontier v4 issues and agent patches, so the item file is
  private, gitignored and canaried. There is no public dev set.
- Jev is pinned to a versioned id (`jev-1.13.0`), not `jev-latest`, and has
  one column: the API has no effort, temperature or seed setting. LLM
  baselines run the same items from Low to Max; ultra stays blocked.
- Latency is wall clock from the operator's machine and includes the network
  round trip. It is published as indicative, with the measuring location
  stated, and is not used to rank.

The name is Verdict, chosen by the owner on 2026-09-20: it says what is
tested (judging a patch, with the tests delivering the real verdict) and
stays vendor-neutral. "System One model" is TypeSafe's term for the class;
use it in taglines and copy, not in the suite name. Working name before
that was Decisions v1, which collided with this file.

Owner request, in chat, 2026-09-19.

### Evidence

- Jev cannot take Frontier v4 or Routine v1: every task needs an agent that
  reads a repo, calls tools and writes a patch, and Jev emits no text.
- TypeSafe's own evals use the average of GPT-6 Astra and Fable 5.1 answers
  as the reference, and the workflows were written by TypeSafe. A survey of
  about 15 independent Jev evals on 2026-09-19 found public labelled
  datasets, rule generators, author labels and LLM agreement as references;
  none covered software-engineering judgments and none used executed tests.
- One independent calibration study (`jev-ood-calibration`) reports ECE
  0.107 on unseen items against a 0.024 noise floor, so calibration is a live
  question and not a given.
- The archive yields 745 distinct uncontaminated agent patches with verifier
  outcomes from seven completed Frontier v4 sweeps (459 all pass, 226 partial,
  55 regression, 5 no progress). Always answering "passes" scores 62.7% on
  the test split with Brier 0.238, which is the floor to beat.
- docs.typesafe.ai (read 2026-09-19): 32k tokens of state per request, up to
  255 choice options, $0.042 per million input tokens, output free, service
  hosted on the US West Coast.

### What this touched

- `harness/verdict/` (items, scoring, TypeSafe adapter),
  `scripts/verdict-v1/build_items.py`, `tests/test_verdict.py`,
  `.gitignore` (`verdict-v1-items/`).

### Not built, and why

- A contamination family. Whether a patch recites a gold patch is a string
  comparison once the gold is in the state, and unanswerable without it, so
  it is not a judgment worth scoring.

### Revisit triggers

- The three patch families ask different questions about the same 745
  patches, so their errors correlate. If a report needs one number, use
  `patch-outcome` alone or bootstrap by patch, not by item.
- `fix-localization` has 23 items (19 test). Treat it as a probe until the
  task pool grows.
- TypeSafe ships a new version: rerun under the new pinned id as a new
  column; never overwrite the old one.

## 2026-09-18: Devin SWE-2 sweeps run medium, high and max only, unpriced

### Decision

The Devin SWE-2 column on VulcanBench Frontier v4 is swept at exactly the
three effort variants Devin's account catalog lists for the family
(`swe-2-medium`, `swe-2-high`, `swe-2-max`), through the Devin CLI harness
(`--harness devin`), one attempt per task, serial, under the flat 3-hour
task timeout. No low or extra-high column is published for SWE-2, and the
cost column is "unavailable" rather than an API-equivalent estimate.
Launcher: `scripts/cii-v4-board/run_devin_effort_sweep.sh`, queued behind
the Sol chain by `logs/devin-swe2-chain.sh`. Owner request, in chat,
2026-09-18.

### Evidence

- Devin exposes effort only as the last token of a model id; `devin models
  list --format json` on this account lists SWE-2 as `swe-2-medium`,
  `swe-2-high` and `swe-2-max` and nothing else (no `-low`, no `-xhigh`).
  Cognition's SWE-2 announcement names the same three levels. The adapter
  refuses an unlisted uid because the CLI otherwise falls back silently to a
  default model (its log: "did not resolve to an available, allowed model;
  starting on the default").
- The cloud Devin Sessions API cannot pin a model or an effort at all (its
  only knob is `devin_mode`: normal, fast, lite, ultra, fusion), so the local
  CLI is the only route that measures SWE-2 at a known effort. `ultra` there
  is a session mode, not an effort, and is blocked anyway.
- SWE-2 has no public per-token price (catalog cost tier "Free" through
  2026-10-10, Devin-only availability), so an API-equivalent cost would be
  invented. Tokens and Devin's credit/ACU counters are recorded instead.
- A live hello-world run on 2026-09-18 (`swe-2-medium`, CLI 3000.10.31)
  confirmed print-mode flags, per-run config acceptance, session harvest,
  receipt deduplication and the served-model check.

### What this touched

- `harness/agent/devin_cli.py` (new), `harness/agent/cli_agents.py`,
  `harness/effort.py`, `harness/agent/run_audit.py` (`"webfetch"` marker),
  `harness/agent/loop.py` (`.devin/` ignored in workspaces), `harness/cli.py`,
  `harness/pricing.py` (comment only), `tests/test_devin_cli.py`,
  `scripts/cii-v4-board/run_devin_effort_sweep.sh`, `logs/devin-swe2-chain.sh`,
  README and `docs/HARNESS_BENCHMARKING.md`.

### Revisit triggers

- Devin adds `swe-2-low` or `swe-2-xhigh` to the catalog: extend `LEVELS`
  in the launcher and the sweep gains a column, nothing else changes.
- Cognition publishes a per-token API rate for SWE-2: add a `devin:swe-2`
  price entry and re-run `reprice_runs.py`; until then the column stays
  unpriced.

## 2026-09-18: assert statements in test files are not security findings

### Decision

The security factor's bandit scan skips finding B101 (use of `assert`) when
the file is test code: under a `tests/` or `test/` directory, named
`test_*.py` or `*_test.py`, or `conftest.py`. Every other finding, and every
B101 outside test code, counts as before. Owner decision, in chat,
2026-09-18; the skipped count is reported in the metric details.

### Evidence

- On the VulcanBench Routine v1 admission gate, GPT-6 Astra at Low wrote or
  extended a unit test file on all 12 tasks and scored 0.0 to 0.7 on
  security for it: every low-severity finding was B101 in those tests. An
  engineer adding tests to a routine change is the behaviour the suite wants.
- Bandit's own documentation recommends skipping B101 for test code, since
  `assert` is the mechanism of a test, not a weakness in shipped code.
- Across all 438 published Frontier v4 runs no patch touched a test file, so
  no published score changes; the rule matters for the Routine suite and for
  any future run that adds tests.

### What this touched

- `harness/evaluator/security.py`: counts severities from bandit's per-finding
  results instead of its totals, dropping B101 in test files.
- `tests/test_security.py`: a test for the exemption and for B101 outside
  tests still counting.

## 2026-09-16: the "ultra" effort level never runs

### Decision

No VulcanBench benchmark runs at effort "ultra", for any model, harness or
suite. The block lives in `vulcanbench.toml` (`[effort].blocked`) and is
enforced before any model call by `harness/settings.py` at every entry point:
`vulcanbench run`, `vulcanbench effort-sweep`, `run_agent` (which every sweep
driver calls), and the Codex effort launcher. A blocked level is refused with
an error naming the file. Owner decision, in chat, 2026-09-16.

### Evidence

- The Codex model catalogue lists "ultra" for GPT-5.6 Terra only, above max;
  GPT-5.5 stops at xhigh and Luna at max. The board publishes Low to Max, so
  an ultra column would compare against nothing.
- Muse Code's "ultra" is a client-side mode mapped onto the provider's
  highest supported tier, not a distinct API effort (recorded in the Muse
  Contributor sweep protocol), so it does not measure a new level either.
- A settings file rather than a code constant so the rule survives new
  adapters and catalogue changes without anyone remembering it.

### What this touched

- `vulcanbench.toml` (new), `harness/settings.py` (new), `harness/agent/loop.py`,
  `harness/cli.py`, `scripts/cii-v4-board/run_codex_effort_sweep.sh`,
  `tests/test_settings.py`.
- Not touched: `harness/effort.py` and `harness/agent/muse_code.py` still know
  the word so recorded runs and protocol hashes stay valid; the running Muse
  Contributor sweep pins both files. That sweep's protocol lists ultra as its
  final level; under this block it will stop with a refusal before that level
  rather than run it, and its protocol should be amended to drop ultra at its
  next stop.

## 2026-09-13: v4 task timeout lowered to 3 hours; concurrency stays at 1

### Decision

1. Every VulcanBench Frontier v4 (`coding-intelligence-index-v4`) task carries a
   flat 3-hour agent timeout (10800 s, 540 steps at the 20 s/step stamp),
   down from 10 hours. Stamped in each task's `agent_hints` and declared
   once in `suite.json` `flat_budget`; `scripts/stamp_task_budgets.py --check`
   verifies against it.
2. Sweeps keep running one task at a time (`--max-concurrency 1`, the
   harness default). Concurrency was considered and deferred.

### Evidence for the timeout

Measured over every completed v4 solver run on disk at the time
(479 runs: claude-fable-5-1, gpt-5.5, gpt-5.6-luna, gpt-6-astra, muse-spark-1.3, muse-spark-1.3-contributor; efforts from minimal to max).

| Statistic | Minutes |
|---|---|
| Median | 11.2 |
| Mean | 25.3 |
| p90 | 33 |
| p95 | 55 |
| p99 | 404 |
| Max | 953 (two runs exhausted the 10-hour bound, both scored 0) |

Runs above each candidate cutoff, and how many of those still passed
functional:

| Cutoff | Runs over | Passed |
|---|---|---|
| 30 min | 60 | 34 |
| 60 min | 22 | 7 |
| 120 min | 16 | 4 |
| 180 min | 9 | 0 |
| 240 min | 9 | 0 |

None of the nine runs past 3 hours passed (best functional score in that
group 0.88). Eight were Muse Spark 1.3 runs. The ninth was a GPT-5.5
extra-high run on paddockcore that ran 16 hours against the 10-hour cap:
the harness's watchdog fired but killed only the npm launcher, not the
native Codex worker, which kept working and writing to the pipe. That is
fixed (the adapters now start each CLI in its own session and kill the
whole process group; see `_kill_process_group` in
`harness/agent/cli_agents.py`), but the fix has not yet been exercised by
a real Codex timeout. The Muse adapter was verified to cut off at 600
minutes. The slowest passing run on record is
claude-fable-5-1 at extra-high effort on legacy-depotcore-binary-parity,
168 minutes. Apart from that runaway run, Codex-harness models (GPT-6 Astra,
GPT-5.5, GPT-5.6 Luna) never exceeded 61 minutes at any effort. So a 3-hour
bound would have changed no pass@1 result while removing 7 hours of wall
clock per stuck run (13 hours in the runaway case).

Note the margin: the slowest passing run sits 12 minutes under the new bound.
A 4-hour bound has the same zero-loss property with more headroom (no run
landed between 3 and 4 hours). 3 hours was chosen deliberately; if a
Claude Code run at high or max effort passes after more than 2.5 hours,
reconsider 4 hours before treating the timeout as a result.

### Why concurrency was deferred

The harness supports it (thread pool in `harness/suite.py`, one temp
workspace per run, no fixed ports), and no rate-limit rejection has ever
been recorded (867 infra retries across v4 sweeps: 866 were one expired
Codex refresh token, one was a Claude Overloaded error). The blocker is
the wall-clock metric. The speed cards and the v4 report cards rank models
by wall-clock minutes per task, and every v4 suite record on the board was
produced at concurrency 1. Runs sharing one 10-core machine and one
subscription session would take longer per task than they do today, and
their durations would not compare against the serial history. Until the
speed panel is sourced only from serial runs (the suite record stores
`max_concurrency`, so a filter is possible) or is decoupled from wall
clock, sweeps stay serial.

Two smaller reasons: no run has ever exercised the Codex or Claude Code
subscription with more than one session at once, so the rate-limit
behavior is unmeasured, and the infra-retry path has no backoff, so a
burst of usage-limit rejections would exhaust both retries and record
scored failures.

### Revisit triggers

- Any v4 run that passes after more than 2.5 hours: consider 4 hours.
- The first Codex run that hits the 3-hour bound: confirm from its
  `summary.json` that `duration_s` is within a minute of 10800 and that
  `cli_agent.timed_out` is true. If it overran, the process-group kill
  regressed and the bound is not enforced for Codex.
- Speed reporting sourced from serial runs only, or from a metric that
  does not depend on wall clock: concurrency 2 or 3 becomes viable. Add a
  backoff on usage-limit infra retries first.
- A new model family with a materially different duration profile: rerun
  the cutoff table above before its first full sweep.

### What this touched

- `tasks/coding-intelligence-index-v4/*/metadata.json` (23 tasks):
  `agent_hints` restamped to 10800 s / 540 steps, `budget_calibration`
  updated. blendcore had been left on the formula value (5400 s) when the
  10-hour bound was stamped; it now carries the flat bound like the rest.
- `tasks/coding-intelligence-index-v4/suite.json`: `flat_budget` added.
- `tasks/coding-intelligence-index-v4/CHARTER.md`: budgets rule 1 revised.
- `scripts/stamp_task_budgets.py`: checks against `flat_budget` when the
  suite declares one.
- `scripts/cii-v4-board/make_report_card.py`, `scripts/harbor-export/`:
  wording that quoted the 10-hour figure.
- Published snapshots under `docs/results/` describe the conditions their
  runs were made under and were not rewritten.

Runs are comparable across the change: no run in either regime was bound
below 3 hours, so results before and after differ only in how long a
failure could take. The GPT-5.6 Luna extra-high sweep was 17 of 23 tasks
in when the restamp landed on 2026-09-13; its last six runs, and the
queued Astra rerun, run under the 3-hour bound.
