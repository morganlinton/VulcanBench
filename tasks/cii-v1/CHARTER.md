# VulcanBench Coding Intelligence Index (CII) — v1 charter

## What this suite is

The **Coding Intelligence Index** is VulcanBench's flagship coding suite: **30
all-new tasks** sourced from real merged OSS PRs, with **per-task time and step
budgets scaled by task complexity** — so a slow verdict means the model could
not solve the task, never that the clock was too short for the task's shape.

CLI: `vulcanbench run --suite cii-v1` (aliases: `cii`,
`coding-intelligence-index`). Display name everywhere user-facing:
**VulcanBench Coding Intelligence Index**.

## Why it exists (lessons paid for in v3)

1. **Uniform clocks measure repo size, not capability.** v3 ran every task at
   its repo-scale default; models were repeatedly observed working productively
   right up to the buzzer on big-tree tasks (see the July 2026 budget raise in
   `harness/task_metadata.py`). A `system`-complexity fix in a medium repo got
   the same 1200s as a one-line edit.
2. **Suites decay.** 13 of v3's 23 tasks pre-dated a May-2026 training cutoff
   within months of shipping. CII v1 is built all-new so its clean window
   starts as late as possible, and every task records `upstream_merged` so the
   clean subset stays a query, not archaeology.

## Time & step budgets (the TerminalBench page)

Every task carries **explicit** `agent_hints.suggested_max_steps` and
`suggested_timeout_s` in its own `metadata.json` — enforced by
`"require_explicit_budgets": true` in `suite.json` (the validator fails any
task without them). Budgets come from the formula in
`harness.task_metadata.complexity_scaled_budgets`:

    budget = repo_scale baseline x task_complexity multiplier
    multipliers: localized x1.0 | multi_file x1.3 | system x1.6 | architecture x2.0

Stamp with `python scripts/stamp_task_budgets.py tasks/cii-v1`; verify in CI
with `--check`. A task whose measured behavior justifies deviating sets
`agent_hints.budget_hand_tuned: true` next to its hand-set values — deviation
is allowed, silence is not. Budgets are stamped, not applied at run time, so
older suites' run conditions never shift underneath cached-run comparisons.

## Composition targets (30 tasks)

- **All-new**: no task carried over from v1–v4. Upstream fix **merged on or
  after 2026-06-01**; prefer 2026-07-01+ so the clean window survives longer.
  Every task: `source: "oss"`, real `base_commit`, `upstream.url`,
  `upstream_merged`, honest `decontaminated` per the validator's rules.
- **Complexity-first**: target ≥ 12 tasks at `multi_file` or above and ≥ 5 at
  `system`/`architecture` — the complexity-scaled budgets only earn their keep
  if the suite has genuinely complex work. `localized` tasks are the floor and
  anchors, not the body.
- **Languages**: roughly Python 10, TypeScript 5, Go 5, Rust 5, JavaScript 5
  (±2 each; follow the good candidates, not the quota).
- **Difficulty**: measure-then-compose (v2 discipline). Build more candidates
  than needed, measure frontier pass@1 (repeat ≥ 3), and compose so the
  aggregate lands near 70% with a genuine hard tail. A fair task the frontier
  scores ~0% on is desirable; a task everything aces is an anchor at best.

## Grading discipline (unchanged from v3/v4 — the parts that worked)

- Deterministic hidden tests only (`grader: "tests"`), no LLM judge.
- `fail_to_pass`: ≥ 3 tests verified failing at `base_commit`; behavior
  asserted via **public APIs**, never exact error text.
- `pass_to_pass`: a guard that compiles and passes at base (no-regression).
- Acceptance constants generated from the gold patch, never hand-guessed.
- Terse, ticket-style `issue.md`: symptom or desired behavior, never the fix,
  no gold-mimicry. Keep a realistic navigation surface — do not over-slice.
- Admission gate: `scripts/validate_tasks.py --sandbox docker` — gold=1.0,
  pre-patch=0.0, deterministic over 3 runs, plus the explicit-budget check.
- Vendored deps committed with the task (`git ls-files <task>/repo/vendor`
  must be non-empty when setup builds offline — see docs/TASK_CONTRIBUTION.md).

## Status

Suite under construction. `suite.json` lists only validated, admitted tasks;
candidates and measurements live in `CANDIDATES.md` until admitted.
