# Changelog

All notable changes to VulcanBench are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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

[0.1.0]: https://github.com/Zen-Open-Source/VulcanBench/releases/tag/v0.1.0
