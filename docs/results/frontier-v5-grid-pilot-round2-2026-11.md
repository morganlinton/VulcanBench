# VulcanBench Frontier v5 grid: pilot round 2

Measured November 21, 2026 (America/Los_Angeles). The first calibration of
the pilot cell (C++, smaller tier, on CLI11) ended with every run at 11 of 12
items and no session stopped at its budget
([report](frontier-v5-grid-pilot-calibration-2026-11.md)). Round 2 asks what
it takes for the pilot to separate models. It has two parts: the same
worklist under a much shorter budget, and a harder, longer worklist. These
are calibration runs, not board entries.

**Neither part separated the two models.** At 7 minutes both still had 11 of
the 12 items passing. On a worklist of 27 items with a harder core, at 15
minutes, GPT-6.2 Sol finished 25 and GPT-6.1 Terra finished 26.

## Part 1: the same 12 items at a 7 minute budget

| Run | Model | Budget (min) | Items done | Agent minutes | Stopped at budget | Existing tests broken | Tool calls | Input tokens | Output tokens | Cost (USD) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 6 | GPT-6.2 Sol, max | 7 | 11 of 12 | 7.0 | yes | 0 | 42 | 1.50M | 18.0k | 0.52 |
| 7 | GPT-6.1 Terra, max | 7 | 10 of 12 | 6.8 | no | 5 | 49 | 2.41M | 18.7k | 0.29 |

Run numbers continue from the first calibration. Each row is one run of 12
items: 11 of 12 is 0.917 (±0.080 binomial stderr), 10 of 12 is 0.833
(±0.108). Minutes measure agent execution only. Input tokens include cached
input. Cost is the API list-price equivalent Codex reports (the runs used a
ChatGPT subscription): $0.81 for both.

- **GPT-6.2 Sol had 11 items done when it was stopped at 7 minutes.** They
  are the same 11 it finished at 15 and 30 minutes. The twelfth item passed
  5 of its 7 held-out checks (6 of 7 with more time).
- **GPT-6.1 Terra also had 11 items passing at 7 minutes**, and ended its
  own turn just inside the budget. Its changes broke 5 of the 1,084 existing
  tests, so the guard charge took one finished item off its count: 10 of 12.
  At 15 and 30 minutes Terra broke none.
- **The budget stop works on a paid run.** The stopped session was graded on
  the tree as it stood and its tokens were recorded.

## Reading part 1

- **Time is not what limits this worklist.** Both models finish 11 of the 12
  items in under 7 minutes, about one item every 40 seconds. A budget short
  enough to bind on items like these would be a few minutes, which measures
  speed, and speed is off the board (decision R13).
- **The only spread so far comes from care, not capability.** The one
  difference between the models in seven runs is Terra breaking existing
  tests when rushed. That is one run, so it is an observation, not a
  measured effect.
- **A longer list of the same kind of item would not help.** At this pace a
  worklist of 24 similar items would still be finished inside 15 minutes.
  The second part of round 2 therefore puts the weight on harder items, not
  only on more of them.

## Part 2: 27 items with a harder core, at 15 minutes

The pilot environment was rebuilt as version 0.3.0: 27 items (23 defects and
4 features) with 87 held-out checks and 1,070 guard tests, built as C++17.
It keeps the original 12 items and adds 15, among them two features with
several parts and a defect whose fix spans several functions. Its
builder rated 5 of the 27 items as likely to take a strong agent more than
about 2 minutes, 6 as borderline and 16 as small: CLI11's history near the
pin is mostly small fixes.

| Run | Model | Budget (min) | Items done | Agent minutes | Stopped at budget | Existing tests broken | Tool calls | Input tokens | Output tokens | Cost (USD) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 8 | GPT-6.2 Sol, max | 15 | 25 of 27 | 15.0 | no | 0 | 65 | 4.43M | 38.8k | 1.15 |
| 9 | GPT-6.1 Terra, max | 15 | 26 of 27 | 14.4 | no | 0 | 82 | 8.89M | 41.1k | 0.79 |

Each row is one run of 27 items: 25 of 27 is 0.926 (±0.050 binomial
stderr), 26 of 27 is 0.963 (±0.036). The two runs cost $1.94.

- **Both models finished nearly everything** in the same 15 minutes that
  the 12-item worklist took. Of the five items rated hardest, Sol finished
  3 and Terra 4. Each ended its own turn at or just inside the budget.
- **Both missed the same item as in every earlier run**, on the same single
  held-out check (6 of its 7 pass).
- **GPT-6.2 Sol also missed one of the new multi-part features** on 1 of
  its 5 held-out checks. GPT-6.1 Terra finished it.
- By kind: Sol 22 of 23 defects and 3 of 4 features; Terra 22 of 23 defects
  and 4 of 4 features.
- No existing test was broken and both tamper scans were clean.

## Reading round 2

- **This environment is at the ceiling for both models at any budget
  tried.** Across nine runs (12 items at 7, 15 and 30 minutes; 27 items at
  15 minutes) each model finished between 83% and 96% of the items, and the
  one-item differences between them go in both directions. That is noise,
  not a ranking.
- **Agents fill the time they are given.** 12 items took 15 minutes when 15
  were allowed and 7 when 7 were allowed; 27 items also took 15. The pace
  is set by the stated budget, not by the work, so the number of items in a
  worklist of this kind does not create difficulty.
- **What both models miss is a few single checks, not items.** Every miss
  in round 2 part 2 is one held-out check short. That is the pattern the
  release 1 measurement showed as a progress score compressed near the top
  ([report](frontier-v5-r1-sol62-max-2026-11.md)).
- **What the pilot has and has not shown.** The format is fast, cheap and
  stable: 27 graded items in 15 minutes for about $1. It has not shown that
  upstream fixes in a small repo can separate strong models. Whether a
  weaker model, a larger repo, or items of a harder kind produce a spread is
  open, and PLAN-v6 should not be frozen until one of them does.

## Run conditions

- The same tag, run worktree, Harbor and Codex CLI versions, models and
  effort as the first calibration. Part 1 used the same private environment
  commit with `GATE_BUDGET_MIN=7`; part 2 used version 0.3.0 of the
  environment (task checksum `414d1139`) with `GATE_BUDGET_MIN=15`. One
  environment per run, one run at a time. Web search, connected apps and
  plugins were off; no trial recorded a web or app call.
- Version 0.3.0 passed the same admission checks as the first version: the
  base scores 0 of 27, the full reference 27 of 27, each item's reference
  alone scores exactly its item, the verifier probes score 0, repeats are
  byte-identical, and Harbor's oracle and no-op runs are accepted. Three of
  its items are built from hand-made inverses of upstream changes that no
  longer revert cleanly; the environment's private design record describes
  each.
- Audit: all four trials ran the stated model at max effort, used real
  tools (42 to 82 tool calls) and had clean tamper scans. Run 6 ended with
  Harbor's budget stop (`AgentTimeoutError` at 420 seconds), which is the
  designed stop and is graded; runs 7, 8 and 9 ended without an exception.
  Run 8's agent phase lasted 15.02 minutes and Harbor recorded no budget
  stop.

## Disclosures

- **One launch stopped by the operator, not scored.** At 21:22 PST a run of
  GPT-6.2 Sol on an intermediate 24-item version of the environment was
  stopped 47 seconds after it started, because a build for the next version
  was running on the same machine and would have skewed a time-budgeted
  score. It was stopped before any result existed. The 24-item version was
  not run again: the 27-item version replaced it.
- **One run per model per condition.** Differences of one item between two
  single runs are within run-to-run noise.
