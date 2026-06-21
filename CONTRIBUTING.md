# Contributing to VulcanBench

VulcanBench is a fully open-source, community-driven benchmark. Contributions of all kinds are welcome — bug fixes, new tasks, documentation improvements, and provider integrations.

## Table of Contents

- [Development Setup](#development-setup)
- [Running Tests](#running-tests)
- [Code Style](#code-style)
- [Versioning](#versioning)
- [Adding a New Task](#adding-a-new-task)
- [Submitting a Pull Request](#submitting-a-pull-request)
- [Reporting Issues](#reporting-issues)
- [License](#license)

---

## Development Setup

```bash
git clone https://github.com/morganlinton/VulcanBench.git
cd VulcanBench
make setup                        # creates .venv, installs all dev + test deps
source .venv/bin/activate
vulcanbench --help
```

For the dashboard and backend:

```bash
pip install -e ".[backend]"
uvicorn backend.app:app --port 8000
cd dashboard && npm install && npm run dev   # http://localhost:3000
```

Run `make help` to see all available targets.

## Running Tests

```bash
make test        # fast unit tests (skips slow + Docker), enforces ≥80% coverage
make ci          # full local CI: lint + typecheck + fast tests
make test-all    # everything including slow/Docker tests (requires Docker daemon)
```

A single task can be validated with:

```bash
vulcanbench validate-task tasks/v1/<task-id>
make validate-tasks   # validates all 52 suite tasks (+ hello-world demo skipped)
```

All PRs must pass `make ci` before review.

## Code Style

- **Python** — [Ruff](https://docs.astral.sh/ruff/) (lint + format) and strict [mypy](https://mypy.readthedocs.io/). Zero warnings enforced. Run `make fmt` to auto-fix.
- **TypeScript/TSX** — ESLint inside `dashboard/`. Run `npm run lint` from that directory.
- Keep imports at the top of every file. Avoid broad `except Exception: pass` — scope your error handling and log to the trace where possible.

## Versioning

VulcanBench follows [Semantic Versioning](https://semver.org/) (`MAJOR.MINOR.PATCH`).
While the project is pre-1.0, treat **minor** bumps as the default for new
user-facing features and **patch** bumps for bug fixes and docs-only changes.

**Bump the version in the same PR as the feature.** When you add a provider,
CLI command, metric, or other user-visible capability:

1. **`harness/__init__.py`** — update `__version__` (canonical runtime version;
   used by `vulcanbench --version` and the backend API)
2. **`pyproject.toml`** — set `version` to the same string (PyPI / packaging)
3. **`CHANGELOG.md`** — add a dated entry under the new version (`### Added`,
   `### Changed`, `### Fixed` as appropriate)
4. **`README.md`** — update the version blurb near the top if it references the
   release number
5. **`tests/test_cli.py`** — update the `test_version` assertion to match

`backend/app.py` imports `__version__` from `harness` — no separate edit needed
there.

Release tags use the `v` prefix (e.g. `v0.2.0`). Do not tag or publish from a
feature PR unless explicitly requested; maintainers cut releases from `main`.

## Adding a New Task

See [docs/TASK_CONTRIBUTION.md](docs/TASK_CONTRIBUTION.md) for the full format spec and validation requirements, and [docs/LARGE_TASK_HANDBOOK.md](docs/LARGE_TASK_HANDBOOK.md) for medium/large OSS slices.

In brief:

1. Create `tasks/v1/<task-id>/` with `metadata.json`, `issue.md`, `repo/`, `tests/`, `gold_patch.diff`, and `expected_metrics.json`.
2. Run `vulcanbench validate-task tasks/v1/<task-id>` — the gold patch must solve it (`functional == 1.0`), `fail_to_pass` tests must genuinely fail pre-patch, and scoring must be deterministic.
3. Open a PR using the task-submission PR template. Include provenance and decontamination notes as required by the validator.

## Submitting a Pull Request

1. Fork the repo and create a branch from `main`.
2. Make your changes and ensure `make ci` passes cleanly.
3. Write or update tests for any new behaviour. Coverage must stay at or above 80%.
4. Fill in the PR template — describe what changed, why, and how you tested it.
5. PRs that add or modify tasks must include `make validate-tasks` output.

## Reporting Issues

Use the GitHub issue templates:

- **Bug report** — unexpected behaviour in the harness, CLI, or dashboard.
- **Feature request** — new provider, metric, or tooling idea.
- **Task feedback** — issues with a specific benchmark task (wrong gold patch, flaky test, etc.).

For security vulnerabilities, see [SECURITY.md](SECURITY.md).

## License

Apache 2.0. By contributing you agree your contributions are licensed under the same terms.
