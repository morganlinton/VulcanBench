# VulcanBench Frontier v5 grid: the anchor pilot cell

Measured November 23, 2026 (America/Los_Angeles). The first two pilot cells
(C++ smaller on CLI11, C larger on curl) held worklists of small upstream
fixes, and every frontier model finished nearly all of them inside the
budget ([curl report](frontier-v5-grid-pilot-curl-2026-11.md)). The fixes
were small and fully specified, so the score could not move. This report
tests the change proposed after those runs: keep about eight small fixes as
a floor, and add a large upstream feature (an "anchor") graded in several
units, so the worklist holds more work than a model can finish in the
budget. A third pilot cell was built that way for the JavaScript and
TypeScript, larger-tier slot on the AI SDK monorepo (vercel/ai, about
260,000 lines), and three models ran it once each. These are calibration
runs, not board entries.

**For the first time on the grid, the budget bound every model and the
scores differed.** All three models finished all eight floor fixes and used
the whole 30 minute budget on the feature; they finished 4, 6 and 6 of its
7 units. A trace of the misses found one of the five unfinished units to be
the environment's fault.

## The cell

- **15 graded units:** 8 floor units (one upstream fix each) and 7 units of
  one feature anchor (an upstream feature series of 1,343 changed lines over
  27 files across the core library, its UI stream protocol, the chat client
  and four framework bindings), graded by its held-out tests in groups.
- **A unit is done** when all of its held-out checks pass; the whole
  existing suite of six packages (about 12,000 cases) is the regression
  wall, and a broken existing test takes one done unit off the count.
- **Admitted before any run** with no model calls: base 0 of 15, reference
  15 of 15 on three identical repeats, each reference alone exactly its own
  units, five verifier probes 0, Harbor oracle 15 of 15 and nop 0 of 15, the
  visible suites passing on the base and with every reference, a second
  implementation of the floor written from the worklist alone passing 8 of
  8, and nothing in the agent's image that gives a check away.

## Result

| Run | Model | Budget (min) | Units done | Floor | Feature units | Agent minutes | Existing tests broken | Tool calls | Cost (USD) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 13 | GPT-6.2 Sol, max | 30 | 12 of 15 | 8 of 8 | 4 of 7 | 30.0 | 0 | 161 | 3.58 |
| 14 | GPT-6.1 Terra, max | 30 | 14 of 15 | 8 of 8 | 6 of 7 | 29.0 | 0 | 286 | 2.68 |
| 15 | GPT-6 Astra, max | 30 | 14 of 15 | 8 of 8 | 6 of 7 | 29.2 | 0 | 154 | 15.11 |

Run numbers continue from the curl report. Each row is one run of 15 units:
12 of 15 is 0.800 (±0.103 binomial stderr), 14 of 15 is 0.933 (±0.064).
Cost is the API list-price equivalent Codex reports (the runs used a ChatGPT
subscription): $21.37 for the three. Audits clean for all three: the session
log states the model and `max`, only shell tool calls, tamper scan clean,
the must-fail sentinel failed as it must, no Harbor exception.

- **No model ended its own turn early.** On the curl cell GPT-6 Astra
  finished 27 small items in under 15 minutes and stopped.
- **The floor added nothing at the top:** 8 of 8 for all three. It will
  matter for weaker models.
- **The feature separated the models:** 4, 6 and 6 of 7 units, 29 points
  between best and worst.
- **The Claude Fable 5.2 run did not start.** Its usage check returned
  "credits required" on the subscription account; it waits for usage credits
  there.

## Trace of the misses

Each missed unit's failing cases were read from the verifier's reports
against the worklist text.

| Model | Unit | Failing cases | Verdict |
| --- | --- | --- | --- |
| Sol | feature unit A | 1 of 22: an abort during a tool call resolves, where the unchanged code rejects | Model: it removed existing behaviour |
| Sol | feature unit B | 19 of 32, most from one runtime error | Model: unfinished at the budget |
| Sol | feature unit C | 4 of 6 | Model: unfinished at the budget |
| Terra | feature unit B | 2 of 32: two cases that turn on how two stated rules combine | Environment: the text did not say how they combine |
| Astra | feature unit B | 9 of 32, on rules the text states | Model (one of the nine borderline) |

- **4 of the 5 missed units are the model's.** Terra's corrected reading is
  15 of 15 inside 29 minutes. This is a reading of the logs, not a rescore;
  the scored result stays 14 of 15.
- The three rules were added to the text afterwards (environment 0.1.1); the
  graded set did not change.

## What it means

- **The anchor does what more small items, a shorter budget and a larger
  repo did not:** it makes the budget bind and the scores differ.
- **One anchor is not enough at 30 minutes:** one model finished the
  worklist on the corrected reading. The cell now has a second, independent
  anchor (environment 0.2.0, 21 units), admitted the same way.
- **Admission by reading still lets rule interactions through on a large
  feature.** A model run read for "followed the text and still failed"
  found them. The plan now requires a second implementation written from the
  worklist alone for floor items and at least two anchor units (PLAN-v6,
  section 6, check 9).
- One run each is thin; Sol below Terra is not the expected order and may
  be noise.

The plan change that follows from these runs (a floor plus one or two
anchors per environment, units as the score, a 30 minute budget, and three
more admission checks) is in the PLAN-v6 draft
([PR #340](https://github.com/morganlinton/VulcanBench/pull/340)). All
twelve environments have since been built and admitted that way; the
calibration runs on all twelve follow.
