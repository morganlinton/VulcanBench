# Changelog

All notable changes to VulcanBench are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Specification gate (`harness/spec_check.py`)**: task validation now flags
  issues that state a defect or location but never describe the expected
  behavior — the failure mode where a hidden test asserts an output the agent
  cannot infer. `vulcanbench validate-task` / `make validate-tasks` downgrade
  such tasks `PASS -> WARN` (warnings do not fail the run); `scripts/check_spec.py`
  scans the suite offline. A reference-model `solvability_verdict` fails
  trivially small, localized fixes that a capable model still cannot solve.

### Changed

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
