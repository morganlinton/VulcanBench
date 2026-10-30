# Contributing to VulcanBench

VulcanBench is a fully open-source, community-driven benchmark. Contributions of all kinds are welcome, bug fixes, new tasks, documentation improvements, and provider integrations.

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

All PRs must pass `make ci` before review. This applies to direct pushes to
`main` too (maintainers included): the CI format gate fails the whole Python job
before mypy or pytest run, so an unformatted file turns `main` red until the
next fix lands, run `make lint` (or full `make ci`) before any push.

## Code Style

- **Python**: [Ruff](https://docs.astral.sh/ruff/) (lint + format) and strict [mypy](https://mypy.readthedocs.io/). Zero warnings enforced. Run `make fmt` to auto-fix.
- **TypeScript/TSX**: ESLint inside `dashboard/`. Run `npm run lint` from that directory.
- Keep imports at the top of every file. Avoid broad `except Exception: pass`: scope your error handling and log to the trace where possible.

## Versioning

VulcanBench follows [Semantic Versioning](https://semver.org/) (`MAJOR.MINOR.PATCH`).
While the project is pre-1.0, a release with a new user-facing feature is a
**minor** bump, and a release with only bug fixes or docs is a **patch** bump.

**Feature PRs add a changelog line; they do not bump the version.** When a PR
changes something a harness user sees (a provider, harness, CLI command or
flag, metric, run artifact, or a fix to any of these), add a line under
`## [Unreleased]` in `CHANGELOG.md`, in its `### Added`, `### Changed` or
`### Fixed` subsection. Results, task-authoring and docs-only PRs do not need
one. Leave the version alone: parallel PRs that each bump it would conflict,
and the version moves once per release, in the release PR.

### Cutting a release

The maintainer decides when the changes under `[Unreleased]` are worth a
release; agents open a release PR only when asked. Releases are cut from
`main`:

1. **Open a release PR** on a branch from `main` (for example
   `release/0.10.0`) that changes only:
   - `harness/__init__.py`: `__version__`, the canonical runtime version used
     by `vulcanbench --version` and the backend API (`backend/app.py` imports
     it, so it needs no edit)
   - `pyproject.toml`: `version`, set to the same string
   - `uv.lock`: the `vulcanbench` package's `version` (`uv lock --check`
     confirms the lockfile still matches)
   - `tests/test_cli.py`: the `test_version` assertion
   - `README.md`: the version blurb, if it names the release number
   - `CHANGELOG.md`: rename `## [Unreleased]` to `## [X.Y.Z] - YYYY-MM-DD`
     and put a fresh, empty `## [Unreleased]` above it. Add a line for
     anything merged since the last release that is missing one.

   Put the [release post](#release-post) in the PR body.
2. **Merge it** once CI passes.
3. **Tag the release PR's merge commit** and push the tag:

   ```bash
   git fetch origin
   git tag -a vX.Y.Z -m "VulcanBench X.Y.Z" <merge commit>
   git push origin vX.Y.Z
   ```

   Tags use the `v` prefix. Never tag a feature branch or an unmerged commit:
   every release is a commit on `main`.
4. **CI creates the GitHub Release.** The tag starts
   `.github/workflows/release.yml`, which publishes a release with install
   steps and notes generated from the merged PRs. Publishing it also starts
   `publish-pypi.yml`; until PyPI publishing is configured for this
   repository, that run fails without uploading anything.
5. **Announce it** with the release post and image, linking the release.

### Release post

Every release PR carries a ready-to-post X post in its body (or, for an
agent, in the chat reply), so the maintainer can post it with light edits.

**Format**

- Lead with `VulcanBench vX.Y.Z` and one line on what it is (open-source LLM coding benchmark)
- 2 to 4 concrete bullets as short phrases (new command, harness, suite or fix, not internal refactors)
- Link: `https://github.com/morganlinton/VulcanBench`
- Stay under 280 characters as X counts them (a link counts as 23); use a thread only if necessary
- Tone: factual, excited but not hype-y; 0 to 2 relevant hashtags at most

**Image.** Attach one clean 16:9 PNG (2400x1350): white background, the logo
chip and wordmark as the brand section of [CLAUDE.md](CLAUDE.md) specifies,
the version large in Chakra Petch SemiBold, one headline line, at most one
supporting line in IBM Plex Mono, and the repository URL. No charts,
screenshots or extra copy. The fonts are in `scripts/rankings-chart/` and the
logo is `docs/assets/vulcanbench-logo.png`. The image is posted, not
committed.

**Example**

```text
VulcanBench v0.10.0 is out, open-source benchmark for LLMs on real coding tasks.

• Every run now logs its tool calls, one schema across 11 harnesses
• vulcanbench tool-calls backfills older runs
• New Mirror harness for Reflection Beam, with full traces

https://github.com/morganlinton/VulcanBench
```

### Regenerating cost priors

Bundled benchmark cost priors (`harness/data/cost_priors.json`) power cold-start
`vulcanbench estimate` when local `./runs` history is empty. After collecting
reference runs, regenerate before a release:

```bash
python scripts/export_cost_priors.py --suite v1-compare --runs-dir ./runs
```

Override at runtime with `VULCANBENCH_COST_PRIORS=/path/to/priors.json`.

### Docker task validation

Before a large benchmark spend, validate gold patches and verifiers inside the
same Docker sandbox used by `vulcanbench run`:

```bash
make sandbox-image-all
make validate-tasks-docker
# or: python scripts/validate_tasks.py tasks/v1 --sandbox docker
```

Host validation (`make validate-tasks`) is faster but does not catch container-only
issues (e.g. Go `GOCACHE` in non-root sandboxes). Rust tasks automatically use
``vulcanbench/sandbox:rust`` in Docker (build with ``make sandbox-image-rust``).

## Adding a New Task

See [docs/TASK_CONTRIBUTION.md](docs/TASK_CONTRIBUTION.md) for the full format spec and validation requirements, and [docs/LARGE_TASK_HANDBOOK.md](docs/LARGE_TASK_HANDBOOK.md) for medium/large OSS slices.

In brief:

1. Create `tasks/v1/<task-id>/` with `metadata.json`, `issue.md`, `repo/`, `tests/`, `gold_patch.diff`, and `expected_metrics.json`.
2. Run `vulcanbench validate-task tasks/v1/<task-id>`: the gold patch must solve it (`functional == 1.0`), `fail_to_pass` tests must genuinely fail pre-patch, and scoring must be deterministic.
3. Register the task in `tasks/v1/suite.json`, then run `python scripts/sync_task_counts.py` to ratchet the dataset floors in `tests/test_dataset.py` and the documented counts. `python scripts/sync_task_counts.py --check` fails if anything is out of sync (CI-friendly, no writes).
4. Open a PR using the task-submission PR template. Include provenance and decontamination notes as required by the validator.

### Curation discipline

A benchmark is only as good as its ability to *separate* models. To keep the
suite earning its run cost:

- **Specify the expected behavior, not just the bug.** The issue must let the
  agent infer what "correct" means; the hidden test is never shown. Validation
  downgrades `PASS -> WARN` for under-specified issues (`harness/spec_check.py`),
  and `python scripts/check_spec.py` scans offline. An issue that only says "fix
  `run`" is unsolvable by design, not hard.
- **Aim above the floor.** Easy single-file fixes anchor the low end, but every
  strong model passing a task means it discriminates nothing. Favor subtle
  correctness, concurrency, edge cases, and `multi_file` / `system` navigation.
  Composition guards in `tests/test_dataset.py` enforce a hard-task floor and a
  medium-or-hard majority.
- **Calibrate from runs, then retire dead weight.** Use `vulcanbench calibrate`
  for empirical difficulty and the discrimination section of `vulcanbench report`
  to find tasks every model passes or fails (no signal). Prefer `--repeat 5+`
  so pass@1 has real confidence intervals and a model pair's separation is
  measured, not assumed.

### Test-graded vs. agentic-graded tasks

Three grading modes, chosen per task via `metadata.grader`:

- **`tests`** (default): hidden `fail_to_pass`/`pass_to_pass` commands. Deterministic
  and exact, but the `issue.md` must specify the expected behavior (the spec gate
  enforces this), since the agent can't see the tests.
- **`agentic`**: an LLM grades the agent's diff against a `metadata.acceptance_criteria`
  list (plain-English requirements, never shown to the agent). This lets the prompt
  be **terse and realistic**. The spec gate is skipped for these tasks. Author one
  with a `repo/`, a terse `issue.md`, a non-empty `acceptance_criteria`, and a
  `gold_patch.diff`; `validate-task` checks the gold grades correct and an empty
  change grades incorrect using the offline mock grader. Run it against a strong,
  independent `--judge-model` so a model isn't grading its own output. Because LLM
  grading is non-deterministic, keep `tests` for anything needing reproducible scoring.
- **`rubric`**: an LLM grades **mergeability**, not just correctness, the axis that
  still separates frontier models once functional correctness saturates. The task ships
  a `metadata.rubric` with `blocking` criteria (failing **any** scores 0, correctness,
  scope) and `weighted` quality criteria (`{"weight": n, "criterion": "..."}`); the
  `functional` score becomes continuous in `[0, 1]` (the weighted fraction of passing
  quality criteria), so a working-but-unmergeable change scores below a clean one.
  `pass_threshold` (default 1.0) sets when a run counts as a pass@1 solve. Like
  `agentic`, the prompt is terse, the spec gate is skipped, and `validate-task` checks
  the gold grades at/above threshold and an empty change scores 0 via the mock grader.
  See `tasks/v1/py-orders-rubric` for a worked example.

  Before relying on an agentic or rubric task, **prove its grader is trustworthy**. Add a
  `grader_cases.json` with labeled candidate diffs (known-correct and known-incorrect, for
  a rubric, include *working-but-unmergeable* diffs as `incorrect`) and run
  `python scripts/grader_eval.py --task tasks/v1/<id> --model <grader> --samples 5`:
  it reports accuracy, false-pass rate, and self-consistency. A non-zero false-pass rate
  means the grader rubber-stamps wrong answers, fix the criteria (or the grader model)
  before shipping. Use `metadata.grader_samples` to grade by majority vote at run time.

## Submitting a Pull Request

1. Fork the repo and create a branch from `main`.
2. Make your changes and ensure `make ci` passes cleanly.
3. Write or update tests for any new behaviour. Coverage must stay at or above 80%.
4. Fill in the PR template, describe what changed, why, and how you tested it.
5. PRs that add or modify tasks must include `make validate-tasks` output.
6. Keep engine changes (harness, tasks, sandbox, config) and published results
   (`docs/results/`, `traces/`) in separate PRs. Published suites are frozen;
   changing one means a version bump and `make freeze-suite`. The full
   workflow, including how benchmark runs are launched, is in
   [docs/HOW_WE_WORK.md](docs/HOW_WE_WORK.md).
7. A new execution harness needs a tool-call extractor in
   `harness/agent/tool_calls.py` that reads what the adapter records (its
   stream, session log or trace) into the shared schema, plus a fixture test.
   `tests/test_tool_calls.py` fails for an adapter without one. See
   [docs/REPRODUCIBILITY.md](docs/REPRODUCIBILITY.md#tool-calls).

## Reporting Issues

Use the GitHub issue templates:

- **Bug report**: unexpected behaviour in the harness, CLI, or dashboard.
- **Feature request**: new provider, metric, or tooling idea.
- **Task feedback**: issues with a specific benchmark task (wrong gold patch, flaky test, etc.).

For security vulnerabilities, see [SECURITY.md](SECURITY.md).

## License

Apache 2.0. By contributing you agree your contributions are licensed under the same terms.
