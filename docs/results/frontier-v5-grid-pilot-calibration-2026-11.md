# VulcanBench Frontier v5 grid: pilot calibration

Measured November 21, 2026 (America/Los_Angeles). These are the calibration
runs that decision R14 calls for (docs/DECISIONS.md, 2026-11-21): the pilot
cell of the grid (C++, smaller tier, on CLI11) run by a stronger and a weaker
model at two time budgets, plus one repeat. Their purpose is to set the time
budget, check that scores spread between models, and see run-to-run noise
before [PLAN-v6](../frontier-v5/PLAN-v6.md) is frozen. They are not board
entries.

## Result

**All five runs finished 11 of 12 items (0.917, ±0.080 binomial stderr,
n=12 items each).** The model, the budget and the repeat made no difference
to the score: every run finished the same 11 items and missed the same one.

| Run | Model | Budget (min) | Items done | Agent minutes | Stopped at budget | Tool calls | Input tokens | Output tokens | Cost (USD) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | GPT-6.2 Sol, max | 15 | 11 of 12 | 14.8 | no | 75 | 5.46M | 37.1k | 1.28 |
| 2 | GPT-6.1 Terra, max | 15 | 11 of 12 | 14.2 | no | 94 | 6.22M | 40.8k | 0.63 |
| 3 | GPT-6.2 Sol, max | 30 | 11 of 12 | 26.4 | no | 80 | 8.02M | 61.4k | 1.81 |
| 4 | GPT-6.1 Terra, max | 30 | 11 of 12 | 28.6 | no | 170 | 22.24M | 81.1k | 1.88 |
| 5 | GPT-6.2 Sol, max (repeat of 1) | 15 | 11 of 12 | 13.6 | no | 69 | 3.99M | 36.0k | 1.08 |

Runs are in start-time order. Minutes measure agent execution only. Input
tokens include cached input. Cost is the API list-price equivalent Codex
reports (the runs used a ChatGPT subscription): $6.69 for all five. The five
runs took 1 hour 45 minutes of wall-clock time, one at a time.

By item kind, every run finished 10 of 11 defects and the 1 feature.

## What the runs show

- **No spread between the models.** GPT-6.1 Terra matched GPT-6.2 Sol item
  for item at both budgets, at about half the cost at 15 minutes.
- **The budget did not bind.** No session was stopped at its budget. Each
  agent was told its budget, paced itself to it (13.6 to 14.8 minutes of 15,
  26.4 and 28.6 of 30) and ended its own turn reporting all 12 items done.
  Doubling the budget raised tokens and cost (three times the cost for
  Terra) and left the score unchanged.
- **No run-to-run noise.** The repeat of run 1 finished the same 11 items.
- **The one miss is a single check, not time.** The unfinished item passed 6
  of its 7 held-out checks in every run. The seventh is the held-out check
  that GPT-6.2 Sol and GPT-6 Astra failed in every attempt at release 1's
  CLI11 task ([measurement](frontier-v5-r1-sol62-max-2026-11.md)).
- **Guards held.** All 1,084 existing tests passed after every run, so the
  guard charge never applied, and every tamper scan was clean.

## Reading the result

- **The run format works.** One session of about 14 minutes produced 12
  graded items for about $1, and the scores were stable. The same repo as a
  single release 1 task took GPT-6.2 Sol 17 to 23 minutes per attempt and
  gave one pass or fail.
- **This worklist does not separate the two models.** It sits at the
  ceiling for both: 11 items are within reach of the weaker model at the
  shorter budget, and the twelfth turns on one sticky check. PLAN-v6 section
  6 asks for items to be added when the strongest model finishes every item
  inside the budget; this result is one check short of that case, for both
  models.
- **These runs cannot set the budget.** Because agents paced themselves to
  whatever budget they were given and scored the same, the runs show that 15
  minutes is enough for this worklist, not where time starts to cost items.
- **Still open:** how many items and how hard they must be for the score to
  spread, whether a budget below 15 minutes binds, and how a model below
  Terra scores. The guard-charge rule was not exercised.

## Run conditions

- Tag `bench/2026-11-21-frontier-v5-grid-pilot-calibration` (`d9305f64b`),
  run worktree pinned to it, Harbor 0.24.0. GPT-6.2 Sol on Codex CLI 0.166.0
  (`gpt-6.2-sol`), GPT-6.1 Terra on Codex CLI 0.159.0 (`gpt-6.1-terra`),
  both with `model_reasoning_effort=max` (confirmed in every trial's logs).
- `scripts/frontier-v5/run_gate.sh --grid <ref>` with `GATE_EFFORT=max` and
  `GATE_BUDGET_MIN` set to 15 or 30, which overrides the grid file's budget.
  One environment per run, one run at a time, so the four-at-a-time batches
  of PLAN-v6 were not exercised. Web search, connected apps and plugins were
  off; no trial recorded a web or app call.
- The pilot environment is private (decision R8) and was pinned at one
  commit for all five runs (task checksum `39fa87d5`). It has 12 items (11
  defects and 1 feature) with 41 held-out checks and 1,084 guard tests. Ten
  of the items are made from upstream fixes that predate the pinned commit
  and are taken back out of the code, which the draft PLAN-v6 does not yet
  allow for; the plan is to be amended before it is frozen.
- Audit: 5 of 5 trials ran the stated model and effort, used real tools (69
  to 170 tool calls each), recorded no web or app calls, had clean tamper
  scans, and ended without a Harbor exception.

## Disclosures

- **First launch stopped before any model call.** The launch at 18:19 PST
  failed at once on an argument-parsing bug in the new run script (a batch
  number passed after the budget option). It was relaunched 39 seconds later
  without the batch number, which runs every batch (the pilot has one). The
  fix goes in a separate engine PR.
- **One environment, one run each.** Each row is a single run on a single
  environment, so each stderr reflects 12 items from one session.
