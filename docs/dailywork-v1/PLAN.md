# VulcanBench DailyWork v1: plan

Status: **draft, 2026-11-18, not adopted.** Owner decisions in section 8 are
open. Once adopted, this plan is frozen the way Frontier v5's plans are (a
`FREEZE.json` beside it), and each decision is recorded as a dated entry in
[docs/DECISIONS.md](../DECISIONS.md).

## 1. The question this suite answers

> Which model, at which effort level, does my engineering team actually need
> for its everyday work?

Most public coding benchmarks answer a different question: can any model
solve the hardest tasks we could find? They rank models by accuracy on work
built to stump the frontier. VulcanBench Frontier v5 is one of them (GPT-6
Astra scored 0 of 21 on its release 1). That is useful for tracking the
ceiling but says little to a team choosing between a fast cheap model and a
slow expensive one for the tickets on its board this week.

DailyWork measures the work a team does every day, on real codebases, and
reports what that work costs: whether the model solves it, how long it
takes, how many tokens and dollars it spends, how it gets there, and how
reliably it does so. Its main output is a recommendation, per kind of work,
of the cheapest model and effort level that does the job.

## 2. Principles

1. **Real work on real code.** Tasks come from real merged changes in
   maintained open-source projects of realistic size, not synthetic
   micro-repositories. The instruction reads like a ticket a teammate would
   write, not a list of failing test names.
2. **A representative mix.** The task categories (section 3) cover what
   engineering teams spend their time on, not only bug fixing.
3. **Solvable by design.** Every task has a known-good solution, and a
   capable model is expected to solve it. Tasks that no tested configuration
   solves belong in Frontier, not here. Easy tasks stay in: they are where a
   cheap configuration proves it is enough.
4. **Measured beyond pass or fail.** Time, tokens, cost, process and
   reliability are first-class results, recorded for every run.
5. **Every effort level.** The effort curve is the product: where along it
   each model stops getting better for each kind of work.
6. **Reproducible and honest.** Frozen suite lock, runs from pinned
   `bench/...` tags, `n=` and ±1 stderr on every figure, and caveats stated,
   as for every VulcanBench suite (docs/HOW_WE_WORK.md).

## 3. Task categories

| Category | What the agent does | How it is graded |
| --- | --- | --- |
| Bug fix | Fix a reported defect from an issue-style report | Held-out fail-to-pass tests plus the project's own suite as a guard wall |
| Feature | Add a small feature from a ticket or short spec | Held-out acceptance tests plus the guard wall |
| Refactor | Restructure code without changing behavior (rename, extract, split a module, remove a deprecated path) | Full existing suite passes, plus structural checks (the old symbol is gone, the new module exists) |
| Tests | Write tests for existing code or a recent change | Mutation score of the new tests against the code under test, and the tests pass on the real code |
| Upgrade or migration | Move to a new major version of a dependency, API or language feature | Builds and passes the suite on the new version; held-out checks for the changed call sites |
| Build and CI | Fix a broken build, lint, type check or CI configuration | The named command succeeds and the suite still passes |

v1 proposal (decision D3): **six categories across six languages** (Python,
JavaScript or TypeScript, Java, Go, Rust, C++), **about 36 tasks**, with each
category represented in at least four languages. Performance work and code
review are candidates for v2 because grading them fairly needs more design
(stable timing on shared hardware; judging review comments).

## 4. Metrics

Most of these are already recorded by the `vulcanbench` harness in every
Frontier v4 run's `summary.json` and `tool-calls.jsonl`; the "New" column
marks what has to be built.

| Metric | Definition | New? |
| --- | --- | --- |
| Solved | Every held-out check passes, the guard wall holds, the tamper and integrity audits are clean | No (verifier and `integrity_audit` exist) |
| Progress | Fraction of held-out checks passed when the guard wall holds; partial credit for near misses | Small (per-check results are already in `verifier`) |
| Reliability | Of the attempts on a task, how many solve it; reported as pass@1 and "solved every time" | Aggregation only |
| Time | Wall-clock run time (`duration_s`), and time to first passing state where the trace shows it | Partly |
| Tokens | Prompt, cached input, completion and reasoning tokens | No |
| Cost | API-equivalent cost at list price, and plan-based cost for subscription runs (`economics`) | No |
| Cost per solved task | Total cost of all attempts divided by solves; the number a budget owner wants | Aggregation only |
| Turns and tool calls | Model turns, tool calls by kind, tool errors | No |
| Process | From the timed tool-call record: calls and files read before the first edit, whether tests ran before finishing, whether the final state was verified after the last edit, edit churn (code written then reverted), failed commands | Yes (derived from existing records) |
| Change quality | Static lint, complexity and security scores (exist); diff size against the reference solution; optional Code quality judging panel (decision D6) | Partly |

Process metrics describe how a model works. They are reported as
diagnostics, not folded into a score, until a pilot shows each one is stable
across attempts and not trivially gamed (decision D2).

## 5. How results are reported

Not a single accuracy rank. The headline views, per kind of work and
overall:

1. **Cost versus solve rate**, one point per model and effort level, with
   the cost-efficient frontier drawn. A configuration off the frontier is
   beaten on both axes by another.
2. **The recommendation table**: for each category, the cheapest
   configuration whose solve rate is within one standard error of the best,
   and the fastest such configuration.
3. **Effort curves** per model: solve rate, time and cost by effort level,
   showing where more effort stops paying.
4. **Per-model pages** with process diagnostics and reliability.

A sortable table (solve rate, median time, cost per solved task, tokens) sits
under the charts for readers who want to rank.

## 6. Where tasks come from

- **New sourcing (most of v1).** Recent merged pull requests in actively
  maintained projects of realistic size, sourced per category and language,
  merged after the training cutoffs of the models in the launch lineup. The
  instruction is rewritten from the linked issue or ticket, never from the
  diff.
- **Frontier v5's twelve built, non-admitted tasks.** They live in
  `tasks/frontier-v5/` on main, each validated (base fails, reference solution
  passes, cheating probes fail), from real projects in six languages, and
  frontier models solve several of them in 4 to 12 minutes, which is
  everyday difficulty. All of them are bug-fix shaped, so they cover one
  category. PLAN.md for Frontier v5 earmarked solved candidates for Routine
  v1; using them here needs the owner's call under D1.
- **Routine v1 and v2** (private, VulcanRoutine repo; Routine v1 has 504
  recorded runs, Routine v2 has not been swept). Screen their tasks against
  the principles in section 2 once D1 is decided.
- **Not Frontier v4.** All 23 of its tasks are synthetic micro-repository
  Python parity puzzles; they measure something real but not daily work.

### Admission

A candidate is admitted when:

1. its environment builds and the reference solution passes three times on
   the grading architecture, and the unmodified base fails;
2. cheating probes fail (deleting tests, hard-coding outputs, editing the
   verifier);
3. at least one reference configuration (decision D7) solves it in at least
   two of three attempts, so it is solvable in practice;
4. its instruction has been read by someone other than its author and is
   clear without the diff.

There is no upper difficulty bar. Instead, the suite targets a spread: by
the cheapest reference configuration that solves each task, roughly a third
easy, a third medium and a third hard. The calibration run (section 9, step
4) measures it.

## 7. Runner and task format

The harness has what DailyWork measures: adapters for each lab's native
tool (claude-code, codex, cursor, grok-build, zcode, muse-code, pi, mirror,
dsh, kimi-code, devin), token and cost accounting, integrity audits and the
tool-call record. What it lacks is per-task containers for every adapter:
today agents run on the host, and only codex can run in a container
(`--agent-container`). Real multi-language projects need their own
toolchains, which is why Frontier v5 used Harbor and Docker images.

Recommendation (decision D5): **extend the harness's container execution to
every adapter and use the harness's task format**, converting Frontier v5's
task environments (Dockerfiles, verifiers) into it. The alternative, running
on Harbor and importing results, would mean rebuilding the metrics,
economics and audits around Harbor's records.

## 8. Owner decisions

**D1. Routine.** DailyWork and Routine ask nearly the same question.
1. DailyWork replaces Routine: Routine's tasks are screened as sources and
   Routine is retired.
2. DailyWork is public and Routine v2 becomes its private holdout: run
   alongside DailyWork to check that public results hold on tasks no one has
   seen.
3. Keep them separate.

Recommendation: option 2 if Routine v2's tasks fit section 2 (to be checked
by reading them), otherwise option 1. A private holdout is the best defence a
public suite has against overfitting and contamination.

**D2. The headline.** Recommendation: no single accuracy rank; the views in
section 5, with process metrics as diagnostics until a pilot validates them.

**D3. Size and mix.** Recommendation: about 36 tasks, six categories, six
languages, each category in at least four languages; performance and code
review in v2.

**D4. Attempts per effort level.** Reliability needs repeated attempts, and
attempts multiply cost.
1. Three attempts at every effort level.
2. One attempt at every level, three at two levels (medium and the model's
   highest level that is not blocked).

Recommendation: option 2 for the launch lineup, option 1 for models a reader
is most likely to choose between.

**D5. Runner.** Recommendation: the harness with container execution
extended to every adapter (section 7).

**D6. Change quality.** Recommendation: static scores and diff size at
launch; the Code quality judging panel added later, since it adds a judging
pass per run.

**D7. Reference configurations for admission.** Recommendation: one cheap
and one strong configuration from different labs (for example Claude
Sonnet 5.5 at medium and GPT-6.1 Terra at medium), so admission does not
depend on one lab's habits.

**D8. Gold and held-out tests.** Recommendation: task statements and
environments public; reference solutions and held-out tests in a private
repository, as Frontier v5's private task does, with only their hashes in
the public lock. Every task carries the canary GUID.

**D9. The other suites.** Recommendation: Frontier v5 stays published as the
ceiling suite and is paused; Frontier v4 keeps its board until DailyWork
launches, then is archived with a dated note. The suite id is
`dailywork-v1`; CLAUDE.md's suite names section gains "VulcanBench DailyWork
v1".

## 9. Phases

1. **Decide and freeze.** D1 to D9 decided, this plan frozen.
2. **Engine (separate PRs from results):** container execution for every
   adapter; progress, reliability and process metrics; the cost-frontier and
   recommendation aggregation.
3. **Pilots.** One task in each of three categories (bug fix, tests,
   upgrade) built end to end and run by two configurations, to prove
   grading, metrics and process diagnostics before sourcing at scale.
4. **Sourcing and building** in pre-registered waves of about six tasks, one
   category per language. Calibration runs with the D7 references after each
   wave measure the difficulty spread.
5. **Freeze release 1:** `tasks/dailywork-v1/suite.lock.json` with task
   hashes, image digests and the private-repo commit.
6. **Launch sweeps** for the launch lineup from a bench tag.
7. **Publish:** results write-up, the section 5 charts, a DailyWork board
   and suite page on vulcanbench.com, public traces for public material.

Later (v1.x or v2): a guide for teams to build a DailyWork-format suite from
their own repository using the harness's suites home
(`~/vulcanbench-suites`), so a team gets the same report on its own code.

## 10. Cost and time (rough)

- **Building:** no model calls except calibration (two reference
  configurations, three attempts, per admitted task: about 220 runs for 36
  tasks, plus rejects).
- **One sweep** (one model, one effort level, one attempt per task): 36
  runs. Tasks are meant to be solvable, so runs should be shorter than
  Frontier's; at 10 to 20 minutes a run that is 6 to 12 serial hours.
- **One model under D4 option 2** (five effort levels, three attempts at
  two of them): 36 x (3 + 2 x 3) = 324 runs, about 55 to 110 serial hours.
  Sweeps run one task at a time today (docs/DECISIONS.md); a launch lineup
  of several models is weeks of machine time at that rate, so concurrency
  for non-speed runs is worth revisiting with its own decision.

## 11. Risks

- **Category grading is uneven.** Bug fix and feature grading is proven;
  refactor, tests (mutation scoring) and upgrades are not. The pilots exist
  to find out before sourcing at scale.
- **Contamination.** Real merged changes are public. Post-cutoff sourcing,
  private gold (D8) and the holdout (D1) are the defences.
- **Process metrics that reward ritual.** A model can run tests it never
  reads. Keep them diagnostic until shown to track outcomes.
- **Machine time.** See section 10.
