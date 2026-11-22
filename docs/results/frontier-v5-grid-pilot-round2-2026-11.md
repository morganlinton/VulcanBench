# VulcanBench Frontier v5 grid: pilot round 2

Measured November 21, 2026 (America/Los_Angeles). The first calibration of
the pilot cell (C++, smaller tier, on CLI11) ended with every run at 11 of 12
items and no session stopped at its budget
([report](frontier-v5-grid-pilot-calibration-2026-11.md)). Round 2 asks what
it takes for the pilot to separate models. It has two parts: the same
worklist under a much shorter budget (below), and a harder, longer worklist
(in progress when this was written). These are calibration runs, not board
entries.

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

## Part 2: a harder, longer worklist

In progress. Results are added here when the new worklist is validated and
both models have run on it.

## Run conditions

- The same tag, run worktree, Harbor and Codex CLI versions, models, effort
  and private environment commit as the first calibration, with
  `GATE_BUDGET_MIN=7`. One environment per run, one run at a time. Web
  search, connected apps and plugins were off; neither trial recorded a web
  or app call.
- Audit: both trials ran the stated model at max effort, used real tools
  (42 and 49 tool calls) and had clean tamper scans. Run 6 ended with
  Harbor's budget stop (`AgentTimeoutError` at 420 seconds), which is the
  designed stop and is graded; run 7 ended without an exception.
