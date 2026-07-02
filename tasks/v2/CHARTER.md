# VulcanBench v2: the realistic hard tier

## Goal
A small suite of tasks that represent **real work an engineering team would give an
agentic coding tool**: a feature to add or a bug to fix in a real application or
framework codebase, requiring navigation of unfamiliar code, adherence to existing
patterns, and not breaking the existing tests. Tasks span difficulty and are composed
so frontier models (Claude Sonnet 5, Claude Opus 4.8) score **about 70 percent aggregate
pass@1 across the whole set, with a genuine hard tail** (a few tasks they cannot yet
solve, near 0 percent). v1 saturates the frontier (48 of 52 tasks solved by both at
every effort); v2 exists to discriminate.

## Two hard truths (from prior experiments)
1. **Hand-authoring cannot produce this.** Synthetic tasks do not trip the frontier no
   matter how clever or how realistic the prompt reads (carbyne 22 traps: Opus 0.996;
   7-file hidden-gate task: Opus 5/5). Difficulty lives in real, unfamiliar codebases
   and real subtle bugs, not in the wording of a ticket. The 70 percent is produced by
   curating and MEASURING real tasks, never by authoring.
2. **Library edge-case fixes are not enough.** A weak-ETag or specifier-wildcard fix is
   real but reads like library maintenance, not engineering-team work, and rarely
   reaches a hard tail. The source must be real PRs from substantial, application-scale
   repos.

## Realism bar (every task)
- Reads like a ticket an engineering team assigns: add feature X, fix bug Y, in a real
  app/framework codebase.
- Terse, ticket-style `issue.md`: state the symptom or the desired behavior, NOT the
  fix. No gold-mimicry (grade the stated requirement plus genuine no-regression, never
  incidental things the gold PR happened to do).
- Keep a realistic navigation surface. Do not over-slice the repo to hand the agent the
  fix location; locating it is part of the task.

## Sourcing (hybrid)
- **Mostly app-scale.** Real merged PRs, merged >= 2026-02-01 (post model cutoff, so the
  fix is novel and decontaminated), from substantial application/framework repos: web
  frameworks (FastAPI, Flask, Starlette, DRF), HTTP clients (httpx, aiohttp), ORMs and
  data tooling (SQLAlchemy, SQLGlot, dbt), dev tooling (poetry, pytest). Run with the
  repo's own dependencies and test suite in a per-task Docker image
  (`sandbox/task.Dockerfile.template`).
- **Some low-dep.** A few clean pure-Python library tasks for the mid/easy band and
  language variety.
- Feature additions and non-trivial multi-file bug fixes preferred over one-line edge
  fixes. Seek a few LARGE or SUBTLE PRs deliberately for the hard tail.

## Grading (deterministic, no judge)
The repo's own tests. `metadata.json`: `grader: "tests"`, `fail_to_pass` = the PR's
new/changed tests, `pass_to_pass` = existing tests that must not regress, `test_timeout_s`.
No LLM judge (rubric grading is too noisy on borderline code). Template:
`tasks/v1/oss-click-choice-brackets/`.

## Difficulty composition (the discipline)
Build roughly 20 to 25 candidates across a difficulty range, MEASURE each (Sonnet 5 and
Opus 4.8, repeat >= 3), then compose the final 10 to 15 so aggregate frontier pass@1 is
about 70 percent WITH a hard tail. Per-task decision:
- **Keep** any well-formed task (gold patch passes, base commit fails the fail_to_pass
  test, deterministic over 3 runs, unambiguous), anywhere on the spectrum. **A task the
  frontier scores near 0 percent on is DESIRABLE if it is fair and well-formed** (that is
  the tail we want).
- **Reject** a task that is trivially aced by both at low effort (unless kept as a
  deliberate easy anchor), OR is flaky / ambiguous / a gold-mimicry gotcha / whose base
  env does not actually reproduce the failure / whose deps do not install reliably.

## Build recipe (per task)
1. `gh` find a qualifying merged PR (post-cutoff, real feature/bug, ships tests).
2. `gh pr diff` gives the gold source change and the tests.
3. `scripts/slice_repo.py` pins the repo at the PR's base commit, preserving the LICENSE
   and a realistic navigation surface; add the per-task Docker env for deps.
4. Write terse `issue.md` (symptom/desired behavior only, no fix leak, no gold-mimicry).
5. Write `metadata.json` (grader:tests, fail_to_pass/pass_to_pass, upstream URL + merge
   date, decontamination_notes).
6. Validate: gold passes, base fails the fail_to_pass test, deterministic x3.
7. Measure: metered run of Sonnet 5 + Opus 4.8; record pass@1; admit or reject.

## Target
10 to 15 admitted tasks; **each frontier model individually** lands 70 to 90 percent
pass@1 (not just the cross-model aggregate), with a genuine hard tail including at least
one task both frontier models fail. Quality over volume. A full 2-model repeat-1
measurement must cost 20 dollars or less (prompt caching in the harness makes this hold).

## Scope: what this suite measures, and what it does not
Measures: issue-driven bug fixes and small features (patches up to roughly 200 lines) in
real, mature, unfamiliar OSS codebases, graded by deterministic hidden tests; plus cost,
steps, tokens, and latency as first-class signals. This is the "here is a ticket, go fix
it in a codebase you have never seen" slice of engineering work.

Does not measure: multi-day or cross-service work, greenfield system design, work
requiring conversation or requirement negotiation, code review, or operating on
proprietary app code. Composition is also deliberately difficulty-weighted (tasks that
discriminate frontier models are over-represented relative to a uniform sample of a real
backlog); the score is a discrimination instrument, not a labor-market survey. Languages:
currently Python plus Go (chi); TypeScript is a known gap.

## Empirical difficulty map (2026-07, Sonnet 5 / Opus 4.8)
- Clean single-site fixes, however subtle: aced by both (never the tail).
- sqlglot optimizer cross-source column resolution: fails Sonnet, aced by Opus.
- App-scale env alone adds NO difficulty (poetry resolver: aced by Opus in 94 steps).
- Multi-site behavioral redesigns with an explicit contract, graded on full structural
  parity (flask#5928 teardown): fail BOTH models robustly (0/3 and 0/3). This is the
  only shape found so far that produces the both-fail tail.
