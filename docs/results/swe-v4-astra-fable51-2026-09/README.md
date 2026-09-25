# Astra and Fable 5.1 comparison

## Effort and API cost card

[Effort and API cost card](astra-vs-fable51-effort-cost.png) adds cache-aware,
per-model API-equivalent cost estimates to every effort, preserving the shared
score/runtime charts and supporting metrics. Total score is displayed as a
percentage using all four weighted metrics; passed-task counts are not shown
on this card. Code quality retains its 20% weight. The standard-rate full-sweep
estimates are Astra $225.04 and Fable $1,159.10. Astra's missing per-request
context sizes are disclosed with a conservative $419.42 long-context bound.
See [API cost methods and totals](API-COSTS.md) for sources and limitations.

## Effort-level comparison card

[Effort comparison card](astra-vs-fable51-effort.png) shows both models at all
five efforts on shared combined-score and runtime axes. Supporting tables
retain Code quality, full passes and raw tokens per task. Lower effort shows
clear observed resource savings for Astra Medium versus Extra-high, but not
for Fable Low or Medium versus Max. Token counts are not API cost estimates.

Reproduce with `.venv/bin/python scripts/cii-v4-board/make_astra_fable_effort_card.py`.
See `astra-vs-fable51-effort.json` for source data and baseline calculations,
and `effort-card-qa.json` for the validation record. Earlier cards are preserved.

## Quick comparison card

[Clean model card](astra-vs-fable51-clean.png) compares each model's highest
observed combined-score effort from the same complete five-effort sweep:
Astra Extra-high and Fable Max with fallbacks. This is a descriptive selection
on these results, not a held-out tuning result. The original detailed card
and [full effort table](effort-comparison.csv) retain all ten groups.
Scores and the 20% Code quality weight are unchanged.

Reproduce with `.venv/bin/python scripts/cii-v4-board/make_astra_fable_clean_card.py`.
See `astra-vs-fable51-clean.json` for the selected source data and
`clean-card-qa.json` for visual and numerical checks.

## Scope

VulcanBench Frontier v4: 23 matched tasks, five efforts, two solver/harness combinations, 230 original solver runs and 1,380 selected retrospective ratings. All saved solver summaries, patches, task definitions, and issue text passed source-hash checks.

## Results

| Model | Effort | Combined /100 | Code quality /100 | Fully passed | Mean minutes | Raw tokens |
|---|---|---:|---:|---:|---:|---:|
| GPT-6 Astra | low | 91.60 | 81.89 | 22/23 | 4.22 | 20,474,694 |
| GPT-6 Astra | medium | 91.73 | 83.04 | 23/23 | 3.82 | 15,736,923 |
| GPT-6 Astra | high | 91.59 | 85.18 | 23/23 | 4.90 | 17,652,773 |
| GPT-6 Astra | extra-high | 92.39 | 86.17 | 23/23 | 8.09 | 21,331,912 |
| GPT-6 Astra | max | 92.25 | 86.54 | 23/23 | 10.28 | 21,847,403 |
| Fable 5.1 with fallbacks | low | 91.04 | 80.72 | 19/23 | 26.90 | 100,833,634 |
| Fable 5.1 with fallbacks | medium | 91.56 | 82.43 | 20/23 | 31.72 | 160,360,100 |
| Fable 5.1 with fallbacks | high | 90.97 | 83.81 | 20/23 | 28.76 | 126,230,303 |
| Fable 5.1 with fallbacks | extra-high | 91.53 | 84.96 | 22/23 | 38.77 | 263,013,721 |
| Fable 5.1 with fallbacks | max | 92.90 | 85.36 | 23/23 | 27.10 | 87,936,207 |

## Time and tokens

Summed solver time: Astra 12.0030 hours; Fable 58.7456 hours; combined 70.7486 hours. These totals exclude post-hoc judging and gaps between solver runs, and are not calendar time.

Raw tokens include cache reads and cache writes. Astra's input count already includes cached input. Fable's per-turn raw usage receipts are summed across all result events; auxiliary CLI model accounting may differ. The old Fable summary field is a cache-price-weighted quantity, not raw tokens, and four runs' summaries account only for their last result. Original summaries are preserved; the card uses receipt-level accounting. No new API-price comparison is claimed.

## Scoring and review

Each run uses 50% partial-credit functional score, 15% automated quality, 15% security, and 20% Code quality. Code quality averages three Astra and three Claude persona ratings equally. The personas examine correctness, readability, and maintainability, using the issue, complete saved patch, and verifier outcome. Solver model/effort labels are omitted; reviewer effort is Medium, tools are disabled, and sessions are separate. LLM judgment is not human ground truth. Persona ratings are not independent human evaluations. The fixed 20% weight is a policy choice.

All 11 Fable solver fallback runs are included: Low 1, Medium 3, High 2, Extra-high 3, Max 2. All 10 Claude reviewer fallback ratings are also included and disclosed. Fallbacks switched to Opus 4.8 after explicit session-level refusal events. The audit checks assistant-message model identities, not only each session's initial model. Astra model identity is requested-only because its stream does not contain a returned model identifier.

Reviewer prompts and scoring match across populations. Claude's reviewer CLI was 2.1.260 for Astra and 2.1.261 for Fable; Fable solver CLI versions also varied. This is a model-plus-harness comparison, not a controlled measurement of base models alone.

## Recovery and accounting

High QueueCore readability on Fable used the first returned rating, 70, after deterministic escaping of unescaped quotes inside inline code in its rationale. The later retry, 72, is excluded. Both original streams and the failed-vote artifact remain intact; no new call was needed for this recovery. See reviewer-accounting.json for every selected/excluded raw call, hash, model fallback, usage, and judging time. Original Astra combined scores are unchanged.

## Reading the card

Dots and intervals use explicitly focused score axes; runtime bars begin at zero. Whiskers show one sample standard error across 23 tasks, not judge uncertainty or a significance test. Full passes require functional = 1.0 and remain distinct from combined scores. All replacement implementations are Python; opaque binaries were built from C.

## Reproduce

Run from the repository root:

```sh
.venv/bin/python -m harness.panel_comparison --include-reviewer-fallbacks
.venv/bin/python -m harness.panel_receipts
.venv/bin/python scripts/cii-v4-board/make_astra_fable_card.py
.venv/bin/python scripts/cii-v4-board/make_comparison_notebook.py
```

Sources and outputs: comparison.json (per-run data and audit), reviewer-accounting.json (all review calls), effort-comparison.csv (ten effort rows), comparison-audit.ipynb (executed calculation checks), astra-vs-fable51.png, and astra-vs-fable51.svg. The notebook's code cells execute top-to-bottom with Python without a Jupyter kernel. This generator does not commit, push, deploy, or publish results.


## Final validation

Share with caveats: all 230 runs and 1,380 selected ratings passed the evidence audit. The static card requires visual inspection after rendering; see visual-qa.json for the inspected output hashes. The companion notebook independently recalculates all ten score means and standard errors, and verifies raw solver-token receipts.

- astra/astra: 345 selected ratings from 345 calls; 0 excluded, 0 format-recovered, 0 fallback ratings. Elapsed review time, including pauses: 25.90 minutes.
- astra/claude: 345 selected ratings from 347 calls; 2 excluded, 0 format-recovered, 10 fallback ratings. Elapsed review time, including pauses: 342.10 minutes.
- fable/astra: 345 selected ratings from 345 calls; 0 excluded, 0 format-recovered, 0 fallback ratings. Elapsed review time, including pauses: 58.30 minutes.
- fable/claude: 345 selected ratings from 349 calls; 4 excluded, 2 format-recovered, 0 fallback ratings. Elapsed review time, including pauses: 67.18 minutes.
