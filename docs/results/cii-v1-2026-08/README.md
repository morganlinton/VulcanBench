# CII v1, frontier results (August 2026)

**VulcanBench Coding Intelligence Index v1**: 41 validated tasks, all sourced
from upstream open-source PRs merged **May to August 2026**, after the training
cutoffs of every model measured, with TerminalBench-style complexity-scaled
time budgets (30 min to 8 h), hidden fail-to-pass tests plus regression guards
that zero the functional score on any regression, and a deterministic ×3
admission gate (gold = 1.0, pre-patch = 0.0).

![CII v1 results](cii-v1-results.png)

## Headline

| model (agent harness) | pass@1 | mean functional | tasks | runs |
|---|---|---|---|---|
| **Claude Opus 5** (Claude Code CLI) | **96.4% ± 2.8** | 0.993 | 37 | 57 |
| **Claude Sonnet 5** (Claude Code CLI) | **89.2% ± 4.6** | 0.969 | 37 | 53 |
| **GPT 5.6 Sol** (Codex CLI) | **86.5% ± 4.7** | 0.962 | 37 | 57 |

All billed via subscriptions; scores are **model + vendor harness**, not raw
API. All three columns cover the same symmetric 37-task set, and every task
any model ever missed is measured at n=3 per model.

**Which gaps are real:** paired per-task tests put Opus over Sol at +9.9
points (t ≈ 2.3, resolved) and Opus over Sonnet at +7.2 points (t ≈ 1.5,
suggestive but not resolved at this n). **Sonnet vs Sol is a statistical
tie** (+2.7 points, t ≈ 0.8); the table order is a point estimate, not a
resolved ranking. That tie agrees with the format-matched external
benchmark (SWE-bench Pro: Sol 64.6 vs Sonnet 5 63.2, also a tie), while
Terminal-Bench 3.0 spreads the same two models (34.6 vs 14.6) because it
measures terminal/ops work this suite does not contain: benchmark orderings
are format-conditional.

## How to read this

- **The suite ranks frontier models; it does not ceiling them.** Only one
  task resists all three models (`oss-networkx-digraph-node-cuts`: Opus 0/3,
  Sonnet 1/3, codex 1/3), and it is also the only task where Opus is the
  weakest of the three.
- **The discriminators are the product.** `clap-mangen-override-usage` is
  the cleanest: Opus 3/3 against 0/3 for both Sonnet and Sol.
  `zod-record-intersection-strictness` (3/3 vs 0/3 vs 1/3) and
  `hono-regexp-wildcard-middleware` (3/3 vs 1/3 vs 1/3) point the same way.
  The separation is not one-directional inside a vendor:
  `hono-trie-suffix-wildcard` goes Sonnet 3/3, Opus 2/3, Sol 0/3.
- **Per-task verdicts need n=3.** Single-run measurements flipped character
  on 4 of 5 re-measured tasks, and an interim repeat-1 Sonnet signal
  deflated at n=3 exactly as regression-to-mean predicts. Aggregate
  rankings were stable at n=1; per-task claims were not.
- **Speed/efficiency caveat**: the Claude Code CLI models used roughly 3×
  the tokens and 1.7× the wall clock of GPT 5.6 Sol per task (16-38 agent
  turns vs 1-3), a harness difference as much as a model one.
- **Saturation limits resolution.** At 86-96% pass@1, 37 tasks cannot
  rank-resolve models ~3 points apart; resolving Sonnet vs Sol would need
  suite-wide repeat-3 or more tasks.

## Why there is no "hard tail" (and what we tried)

Ten difficulty hypotheses were tested, five under a formal frontier
admission gate (n=3 per candidate against both Opus 5 and GPT 5.6 Sol,
`scripts/frontier_gate.sh`): architecture-scale features, hindsight traps
from upstream regression pairs, performance-gated tests with calibrated
thresholds, multi-bug algorithmic epics, convention-heavy contracts
(TypeScript index signatures, RFC 9111 caching, Postgres operator
precedence), diagnosis-from-symptom tasks, and a live multi-service
concurrency environment. **Opus 5 solved 21/21 gated runs.** In one
instructive case it rewrote the algorithm from theory in 106 seconds rather
than finding the planted bugs. Gate rejects were recycled into v1 (which is
how the suite grew from 38 to 41 tasks). The full candidate log with
per-run scores is in
[`tasks/cii-v2/CANDIDATES.md`](../../../tasks/cii-v2/CANDIDATES.md); the v2
charter (difficulty-gated, currently empty pending a task-format change) is
in [`tasks/cii-v2/CHARTER.md`](../../../tasks/cii-v2/CHARTER.md).

## Coverage notes

- 37 of 41 tasks have symmetric three-model coverage and form every number
  above. Excluded: `oss-pydantic-none-discriminator` (requires a per-task
  virtualenv the host CLI runner doesn't provide; Docker/API only) and the
  three tasks recycled into v1 after the sweep
  (`oss-sqlglot-pushdown-semantics`, `oss-zod-fromjsonschema-epic`,
  `env-ledger-concurrent-transfers`), which have asymmetric gate-run
  coverage only.
- Saturated tasks are n=1 per model; every task with any miss is n=3 for
  all three models.
- Costs are hypothetical API-list prices (subscription-billed runs):
  ≈ $0.48/task (codex) vs ≈ $1.5-6/task (Claude Code models) on the
  measured sweeps.
- Regenerate the chart: `python scripts/cii-report/make_chart.py`
  (aggregates `./runs` directly and fails if the symmetric set drifts).
