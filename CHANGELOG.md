# Changelog

All notable changes to VulcanBench are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Behavior-preserving refactor** `py-extract-pricing` (medium, `refactor`, `multi_file`):
  `shop/checkout.py` inlines every pricing rule in one `total` function; extract them into the
  four pure helpers in `shop/pricing.py` (subtotal, discount, tax, shipping) and have `total`
  compose them without changing results. The pricing tests fail pre-refactor (stubs); the
  end-to-end total tests already pass and must stay green, so any drift in the extracted
  rules (flooring, the flat-discount cap, the free-shipping threshold) is caught. Validated
  (gold=1.0, pre-patch fails, deterministic). Broadens the `refactor` category.
- **Multi-file feature** `go-pubsub` (hard, `multi_file`): an in-memory publish/subscribe
  broker with MQTT-style wildcard topics. `pubsub/match.go` implements segment matching (`+`
  is exactly one segment, `#` matches the trailing remainder including zero segments);
  `pubsub/broker.go` is a mutex-guarded subscription registry delivering in subscription
  order, with callbacks invoked outside the lock so a callback may (un)subscribe. The
  concurrent test runs under `-race`, so a non-thread-safe solution is rejected. Validated
  (gold=1.0, pre-patch fails, deterministic); the gold patch spans both files.
- **Multi-file feature** `ts-state-machine` (medium, `multi_file`): implement a finite state
  machine over a transition table. `src/transitions.ts` indexes `{from, event, to}` triples
  for `nextState`/`allowedEvents`; `src/machine.ts` drives state + history and uses the table
  to validate `send`, throwing `InvalidTransitionError` and staying a no-op on an invalid
  event, and notifying listeners in registration order. First non-localized TypeScript task.
  Validated (gold=1.0, pre-patch fails, deterministic); the gold patch spans both files.
- **Async bug fix** `ts-retry-backoff` (medium, `bug_fix`): an exponential-backoff retry
  helper in `src/retry.ts` with two classic bugs — an off-by-one that runs `attempts - 1`
  tries (so an operation that would succeed on its final attempt is reported failed), and an
  uncapped delay that ignores `maxDelayMs`. An injected `sleep` recorder makes the exact
  backoff schedule (`[10, 20, 40, 50, 50]`) and the call count assertable with no real
  timers. Validated (gold=1.0, pre-patch fails, deterministic).
- **Concurrency bug fix** `go-ratelimit` (hard, `concurrency`): a token-bucket rate limiter
  for throttling an API client that over-grants and races. `bucket/bucket.go` never clamps
  refill to capacity (an idle bucket grants a huge burst) and has no locking (concurrent
  `Allow` callers race and over-grant). The fix needs both a capacity clamp and a mutex held
  across refill+consume. Tests inject a clock for deterministic timing and run under `-race`,
  asserting exactly `capacity` concurrent grants. Validated (gold=1.0, pre-patch fails,
  deterministic).
- **Realistic multi-file bug fix** `py-paginate-cursor` (medium, `multi_file`, `bug_fix`):
  cursor pagination that repeats rows at page boundaries and never terminates. Two bugs in
  two files: `pagination/cursor.py` compares with `>=` (the cursor row is treated as "after"
  itself, so it repeats), and `pagination/repository.py` always advertises a `next_cursor`
  (so a walk never ends, and loops forever on the last record). Ships real buggy code, not
  stubs; tests walk the full dataset asserting no duplicates, no gaps, correct tie-breaking
  on `id`, and termination (with a loop guard so the buggy version fails instead of hanging).
  Validated (gold=1.0, pre-patch fails, deterministic).
- **Fourth multi-file task** `py-event-ledger` (hard, `multi_file`): implement an
  event-sourced bank ledger. `ledger/events.py` holds the immutable event schema and a
  pure reducer (`apply_event`/`replay`); `ledger/bank.py` is the command side that validates
  commands, emits events, and maintains live balances through the reducer. The difficulty is
  cross-file: the live state must always equal a replay of the log, a successful `transfer`
  must emit exactly two events atomically (never a half-applied transfer), rejected commands
  must emit nothing, and `Bank.from_history(history())` must reproduce balances exactly.
  Validated (gold=1.0, pre-patch=0.0, deterministic); the gold patch spans both files.
  Suite v1: 42 tasks, 14 hard, 6 non-localized.
- **Third multi-file task** `py-bytecode-vm` (hard, `multi_file`): implement a bytecode
  compiler and a stack VM that share only a documented instruction set. `vmlang/compiler.py`
  lowers an AST to a flat instruction list (with backpatched forward jumps for `if` and
  short-circuit `and`/`or`); `vmlang/vm.py` executes that list, treating jump operands as
  absolute indices. The difficulty is cross-file: the compiler's opcodes and jump offsets
  must line up exactly with the VM's stack discipline, and short-circuit correctness
  requires the two halves to cooperate so the un-selected operand is never executed.
  Validated (gold=1.0, pre-patch=0.0, deterministic); the gold patch spans both files.
  Suite v1: 41 tasks, 13 hard, 5 non-localized.
- **Second multi-file task** `py-txn-kvstore` (hard, `multi_file`): implement a
  transactional in-memory key/value store whose `Store` and `UndoJournal` collaborate
  across two files. The store computes the inverse of each mutation (prior value vs. prior
  absence) and the journal owns the frame stack, where rollback runs inverses in reverse
  and a nested commit folds its frame into the parent so an outer rollback still undoes
  inner-committed work. Validated (gold=1.0, pre-patch=0.0, deterministic); the gold patch
  spans both files. Suite v1: 40 tasks, 12 hard, 4 non-localized.
- **First real multi-file task** `py-reactive-sheet` (hard, `multi_file`): implement a
  reactive spreadsheet whose `Sheet` and `DependencyGraph` collaborate across two files,
  requiring transitive topological recomputation, dependency clearing on formula rebind,
  and cycle detection kept consistent across both modules. Validated; the gold patch spans
  both files. Suite v1: 39 tasks, 11 hard, 3 non-localized.
- **Grader trust + variance controls** for agentic grading: `metadata.grader_samples: N`
  grades by majority vote over N calls (ties resolve to incorrect) and reports
  `self_consistency`; `scripts/grader_eval.py` (+ `harness/grader_eval.py`) scores a
  task's grader against labeled `grader_cases.json` candidates, reporting accuracy,
  false-pass rate, and self-consistency so a grader is validated before it's trusted.
  Labeled calibration sets ship for three agentic tasks across task shapes
  (`py-slugify-terse`, `py-parse-bool-terse`, `py-chunk-terse`), each with a gold
  case plus three subtly-wrong variants, so grader trust is measured on more than
  one task.
- **Agentic grader (opt-in)**: a task can set `metadata.grader: "agentic"` with an
  `acceptance_criteria` list, and its `functional` score comes from an LLM verdict
  on the agent's diff (`harness/evaluator/agentic_grader.py`) instead of hidden
  tests — so the prompt can be terse and realistic (CursorBench-style) while the
  grader holds the spec. Runs even with `--no-judges`; uses `--judge-model` so a
  strong, independent model can grade. The spec gate is skipped for agentic tasks;
  `validate-task` checks the wiring offline (gold grades correct, an empty change
  grades incorrect). Demo: `tasks/v1/py-slugify-terse`. Test-graded tasks remain
  the deterministic default.
- **Discrimination report**: `vulcanbench report` now includes a model-separation
  section — per task whether the models split, and per model pair how many tasks
  tell them apart (McNemar discordant counts), plus retirement candidates that
  every model passes or fails. Surfaces ties that aggregate pass@1 hides (e.g.
  two models posting an identical 0.7885 with zero tasks separating them).
- **Hard, discriminating tasks** to raise the suite's ceiling (52 -> 43 -> 31 ->
  35): `py-expr-eval` (a recursive-descent arithmetic evaluator with a subtle
  operator-precedence/associativity bug — `2 + 3 * 4`, `2 ** 3 ** 2`, `-2 ** 2`),
  `go-parallel-map` (a bounded-concurrency ordered map that must preserve input
  order, return the lowest-input-index error, and pass `go test -race`),
  `py-sliding-window-max` (an O(n) deque solution gated by a performance test, so
  a correct naive O(n*k) answer is too slow), `go-ttl-lru-cache` (a
  thread-safe LRU cache with per-entry TTL, verified under `go test -race`, where
  a correct single-threaded implementation that omits the mutex fails), and two
  edge-dense correctness tasks designed to pull frontier models below 100%:
  `py-url-normalize` (RFC 3986 path normalization, percent decoding before
  dot-segment removal, with malformed-input handling) and `py-semver-compare`
  (Semantic Versioning precedence, including the prerelease identifier rules that
  trip strong models). Their hidden tests are split into many independent
  edge-case groups, so missing any one leaves the task unsolved. Suite v1 is now
  37 tasks; the hard floor in the dataset guards rose from 5 to 9.
- **Specification gate (`harness/spec_check.py`)**: task validation now flags
  issues that state a defect or location but never describe the expected
  behavior — the failure mode where a hidden test asserts an output the agent
  cannot infer. `vulcanbench validate-task` / `make validate-tasks` downgrade
  such tasks `PASS -> WARN` (warnings do not fail the run); `scripts/check_spec.py`
  scans the suite offline. A reference-model `solvability_verdict` fails
  trivially small, localized fixes that a capable model still cannot solve.

### Changed

- **Curation discipline**: added composition guards (`tests/test_dataset.py`) for
  a hard-task floor, a medium-or-hard majority, and non-localized coverage so the
  suite can't silently regress to easy filler; documented the discipline
  (specify behavior, aim above the floor, calibrate-then-retire) in
  `CONTRIBUTING.md`; and reconciled the README's task-corpus description with the
  actual composition (predominantly `localized` today, with broader
  `multi_file`/`system`/`architecture` and larger `repo_scale` coverage tracked
  as active work rather than claimed as shipped).
- **Suite v1 pruned to 31 tasks** (from 52) to raise discriminating power. First,
  9 placeholder Python scaffolds (`oss-py-cache-evict`, `oss-py-m2-03/06/09`,
  `oss-py-m3-03/06/09/12/15`) whose issues asked to "correct `run`" with no
  statement of intended behavior — unsolvable by design, so all three benchmarked
  models scored 0. Then 12 zero-discrimination `Double`/`x*2` one-liners across Go
  and TypeScript (`oss-go-m2-04/07`, `oss-go-m3-01/07/10/13`, `oss-ty-m2-05/08`,
  `oss-ty-m3-02/08/11/14`) that every model solved. Two easy anchors per language
  are kept; `oss-py-m2-00` and `oss-py-m3-00` were re-specified as honest anchors.

## [0.5.1] - 2026-06-23

### Fixed

- **Rust tasks in Docker**: auto-select `vulcanbench/sandbox:rust` for Rust tasks;
  writable `CARGO_HOME` in non-root sandboxes; `make sandbox-image-rust` builds the
  Rust toolchain image

## [0.5.0] - 2026-06-22

### Added

- **`validate_tasks --sandbox docker`**: run gold-patch verification inside the
  Docker sandbox (same environment as `vulcanbench run`); `make validate-tasks-docker`

## [0.4.0] - 2026-06-22

### Added

- **Bundled cost priors**: `vulcanbench estimate` and `run --dry-run` use shipped
  benchmark cost data when local `./runs` history is missing (cold-start installs)
- **`--no-priors`**: disable bundled priors and fall back to legacy defaults only
- **`scripts/export_cost_priors.py`**: regenerate `harness/data/cost_priors.json`
  from local reference runs

## [0.3.0] - 2026-06-22

### Added

- **`vulcanbench estimate`**: pre-run USD cost ranges per provider/model from local
  `./runs` history, with recommended credit to load before a benchmark
- **`run --dry-run`** now prints a cost estimate for priced models
- **`v1-compare` suite**: 12-task trimmed head-to-head set (Go / Python / TS / Rust)

### Fixed

- **Docker sandbox Go verification**: set writable `HOME` / `GOCACHE` so `go test`
  scoring works for non-root containers (fixes false `functional=0.0` on Go tasks)
- **GPT-5 Chat Completions**: omit `temperature` for GPT-5 / o-series models that
  only accept the API default

## [0.2.0] - 2026-06-21

### Added

- **Z.ai provider** (`zai:<model>`): first-class support for GLM models via
  `ZAI_API_KEY` and OpenAI-compatible Chat Completions (`glm-5.2`, etc.)
- Built-in token pricing for `zai:glm-5.2`, `zai:glm-5.1`, `zai:glm-5`, and
  `zai:glm-5-turbo`

## [0.1.0] - 2026-06-19

### Added

- **v1 MVP harness**: `vulcanbench run` with mock, OpenAI, and Anthropic providers
- **Docker sandbox** (default): isolated, non-root, network-off command execution
- **52-task benchmark suite** across Python, Go, TypeScript, and Rust with gold-patch validation
- **Five-metric scoring**: functional, quality, security, efficiency, human_like (3-judge ensemble)
- **Suite tooling**: `--repeat`, `--max-concurrency`, `--max-cost`, `--fail-under`, effort sweeps
- **Artifacts**: JSONL trace, `summary.json`, `final.patch`, self-contained `replay.html`
- **Leaderboard, report, and calibration** CLI commands
- **FastAPI backend** with filesystem or Postgres storage
- **Next.js dashboard**: leaderboard, tasks, run viewer, submission flow
- **Optional DB write-through** from CLI when `VULCANBENCH_API_BASE` is configured
- **Alembic migrations** for database schema evolution
- **Production Docker stack** (`docker-compose.prod.yml`, backend/dashboard Dockerfiles)
- **Documentation**: METRICS.md, REPRODUCIBILITY.md, DEPLOYMENT.md
- **CI**: lint, typecheck, ≥80% harness coverage, task validation, dashboard build, sandbox image build
- **Release workflows** for GitHub Releases and optional PyPI publish

### Notes

- Install from source: `pip install -e ".[dev,test]"` (task corpus and sandbox Dockerfiles ship in the repo clone, not the PyPI wheel)
- Hosted deployment: see [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md)

[0.3.0]: https://github.com/morganlinton/VulcanBench/releases/tag/v0.3.0
[0.2.0]: https://github.com/morganlinton/VulcanBench/releases/tag/v0.2.0
[0.1.0]: https://github.com/morganlinton/VulcanBench/releases/tag/v0.1.0
