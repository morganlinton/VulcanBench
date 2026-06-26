# Changelog

All notable changes to VulcanBench are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

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
  a correct naive O(n*k) answer is too slow), and `go-ttl-lru-cache` (a
  thread-safe LRU cache with per-entry TTL, verified under `go test -race`, where
  a correct single-threaded implementation that omits the mutex fails). The hard
  floor in the dataset guards rose from 5 to 7.
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
