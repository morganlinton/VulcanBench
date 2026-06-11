# VulcanBench

[![CI](https://github.com/Zen-Open-Source/VulcanBench/actions/workflows/ci.yml/badge.svg)](https://github.com/Zen-Open-Source/VulcanBench/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![Python](https://img.shields.io/badge/python-3.12%2B-blue.svg)](https://www.python.org/)

Fully open-source, transparent, community-driven benchmarking for LLMs on realistic software engineering tasks.

**v1 MVP** — real tool-calling agent loop (mock/OpenAI/Anthropic), optional
Docker sandbox, full trace+replay, five-metric scoring (functional, quality,
security, efficiency, human_like), a local leaderboard, and a Next.js dashboard
that renders live data from the backend.

## One-command setup

```bash
git clone https://github.com/Zen-Open-Source/VulcanBench.git
cd VulcanBench
make setup
source .venv/bin/activate
vulcanbench --help
```

Dashboard + backend (the dashboard reads live data from the API):
```bash
pip install -e ".[backend]"
uvicorn backend.app:app --port 8000          # serves ./runs at /api/*
cd dashboard && npm install && npm run dev    # http://localhost:3000
```
The dashboard falls back to a friendly empty state if the backend isn't running.
Point it elsewhere with `NEXT_PUBLIC_API_BASE` (see `dashboard/.env.example`).

By default the API reads `./runs/` directly. For a durable, queryable store, set
`DATABASE_URL` (Postgres or SQLite) and the API switches to a database —
`POST /api/runs` and `/api/feedback` become writable, and
`python scripts/ingest_runs.py` bulk-loads existing runs. A Postgres is provided
by `docker compose up db`.

## Example run

```bash
# Offline, deterministic (no API key) — drives the real agent loop end to end:
vulcanbench run --task hello-world --model mock:synthetic

# Any real model via the generic provider interface:
export OPENAI_API_KEY=...      # or ANTHROPIC_API_KEY=...
vulcanbench run --task hello-world --model openai:gpt-4o
vulcanbench run --task hello-world --model anthropic:claude-opus-4-8

# Each run prints all five metrics + cost, e.g.:
#   functional=1.0 quality=1.0 security=1.0 human_like=0.8 total=0.974 cost=$0.0
# and writes ./runs/<id>/{trace.jsonl, summary.json, replay.html, final.patch}

# Run a whole suite: repeat for signal, parallelize, and cap the spend.
vulcanbench run --suite v1 --model openai:gpt-4o --repeat 5 --max-concurrency 4 --max-cost 20.00
# Fast micro/small sweep vs navigation-heavy medium/large tasks:
vulcanbench run --suite v1-micro --model openai:gpt-4o
vulcanbench run --suite v1-large --model openai:gpt-4o --repeat 5 --sandbox docker
vulcanbench leaderboard            # by model: pass@1 ± stderr, pass@k, cost, latency
vulcanbench leaderboard --by run   # per-run drill-down
vulcanbench report -o report.md    # shareable Markdown/JSON report (ranking,
                                   #   per-task breakdown, environment, drift flags)
vulcanbench calibrate              # empirical difficulty calibration from recorded runs
vulcanbench replay <id>

# Run the agent's commands in an isolated container (see Sandbox below):
vulcanbench run --task hello-world --model openai:gpt-4o --sandbox docker

# Use it as a CI regression gate (threshold must be in [0, 1]):
vulcanbench run --suite v1 --model openai:gpt-4o --repeat 5 --fail-under 0.8
```

The gate **fails closed**: it exits `4` if pass@1 is below the threshold, if
pass@1 is unavailable, *or if any suite run errored* — a CI gate never goes
green on a partial or unknown result.

Exit codes: `0` ok · `1` usage/error · `2` provider · `3` sandbox · `4` gate
failed (below `--fail-under`, or a run errored).

`final.patch` is a real `git diff` of the agent's edits; `replay.html` is fully
self-contained (open in any browser). Use `--no-judges` to skip the LLM judge
ensemble, `--timeout SECONDS` to cap a run's wall-clock. Traces, summaries, and
patches are secret-redacted and size-capped before they're written, so run
artifacts are safe to publish. See `make ci`, `make docker-up`, docs/.

## Models

Specify a model as `provider:model`:

- `mock:synthetic` — deterministic, offline; used by tests and demos.
- `openai:<model>` — OpenAI Chat Completions (or any OpenAI-compatible endpoint
  via `OPENAI_BASE_URL`). Needs `OPENAI_API_KEY`.
- `anthropic:<model>` — Anthropic Messages API. Needs `ANTHROPIC_API_KEY`.

## Sandbox

The agent's tool execution can run in an isolated Docker container instead of on
the host:

```bash
# Build the base image once (git, ripgrep, ruff, bandit, radon, pytest):
docker build -t vulcanbench/sandbox:base -f sandbox/Dockerfile.base .

vulcanbench run --task hello-world --model openai:gpt-4o --sandbox docker
# --sandbox local|docker|auto   (default: local)
# --image vulcanbench/sandbox:base   (default: per-task metadata or vulcanbench/sandbox:base)
# --network                     (off by default; opt in for dependency installs)
```

- `local` (default) runs tools on the host — fast, no Docker required.
- `docker` runs them in a non-root, **network-off**, resource-limited container
  (workspace bind-mounted, cleaned up after each run). It errors out if the
  daemon is unreachable — it never silently falls back to host execution.
- `auto` uses Docker when available, otherwise falls back to local with a warning.

File operations (read/edit/search) always run host-side over the shared mount;
command execution (`run_command`/`run_tests`/`run_lint`) **and the functional
verifier** run inside the container, so the whole run is reproduced in one
isolated environment. Build the all-language image with `docker build -t
vulcanbench/sandbox:base -f sandbox/Dockerfile.base .` (Python + Go + Node).

## Tasks

The `tasks/v1/` suite holds **52** multi-file, gold-verified tasks across Python,
Go, TypeScript, and Rust, plus the `hello-world` demo. Each task ships a starting
`repo/`, **hidden** `tests/` (never shown to the agent), declarative
`fail_to_pass`/`pass_to_pass` test commands in `metadata.json`, and a
`gold_patch.diff` reference solution.

The corpus spans difficulty (`easy` / `medium` / `hard`) and category
(`bug_fix`, `feature`, `refactor`, `concurrency`) so it discriminates between
weak and strong models. Examples: an RFC 6901 JSON Pointer resolver
(`py-jsonpointer`, hard), a race-free bounded worker pool verified under
`go test -race` (`go-worker-pool`, hard), and a prototype-pollution-safe deep
merge (`ts-deep-merge`, hard).

```bash
make validate-tasks                              # validate every task
vulcanbench validate-task tasks/v1/<id>          # one task
```

Validation proves each task is real: the gold patch must solve it
(`functional == 1.0`), the `fail_to_pass` tests must genuinely fail *before* the
fix, and scoring must be deterministic over repeated runs.

**Provenance is labeled and checked.** Every task declares `source`
(`hand-authored` or `oss`) and an explicit `decontaminated` boolean. Hand-authored
tasks are written now (post-cutoff, so `decontaminated: true`); the validator
enforces that. An `oss` task (e.g. `oss-inflection-titleize`, sourced verbatim
from a real MIT-licensed repo with its LICENSE preserved) is honestly labeled
`decontaminated: false` — its fix predates model cutoffs — and the
`vulcanbench report` integrity section flags every run scored against it. Scaffold
one with `python scripts/import_oss_issues.py`. Format details:
[docs/TASK_CONTRIBUTION.md](docs/TASK_CONTRIBUTION.md).

## Architecture & Reproducibility

- Standardized tools (list/read/edit/search/run) behind one protocol, with
  interchangeable local and Docker executors (see [Sandbox](#sandbox))
- Every step captured as JSONL (llm, tool, diff, test, metric) + token usage
- Each run records its `vulcanbench replay <id>` command for reproduction
- Docker sandbox runs untrusted command execution in a non-root, network-off,
  resource-limited container

Full details: docs/ARCHITECTURE.md and the plan `.kilo/plans/1780024121334-silent-circuit.md`

## License

Apache 2.0 (see LICENSE).

## Status

Working end to end: real tool-calling agent loop with a generic provider
interface (mock/OpenAI/Anthropic), an optional Docker sandbox for isolated tool
execution, git-diff patch capture, and the full five-metric evaluator —
`functional` (verifier), `quality` (lint + complexity + maintainability),
`security` (static analysis), `efficiency` (tokens/steps), and `human_like`
(3-judge LLM ensemble) — plus suite runs (`--suite`) with per-run **cost** and
**latency**, a **per-model** aggregate leaderboard (avg score, pass rate, cost,
time), JSONL trace + HTML replay, a read API, and a live dashboard. Core harness
test coverage is ~86%.

Scoring details: Python is analyzed natively (ruff + radon + bandit); Rust uses
cargo fmt + clippy (quality) and cargo audit + unsafe-delta penalty (security);
JS/TS, Go, and Java analyzers run when their toolchains are installed and
otherwise report `null` with a reason (never a fabricated score). The judge ensemble runs by
default reusing the run's model (`--judge-model` to override, `--no-judges` to
skip); see the [Models](#models) section. Cost uses a built-in price table
(overridable via `VULCANBENCH_PRICING`); unknown models report `cost=null`
rather than a guess (`mock` is $0). Empirical difficulty calibration
(`vulcanbench calibrate`) derives per-task difficulty from observed solve rates
across models and surfaces disagreements with the hand-labeled `difficulty` in
the report and dashboard.

Next up (see [docs/ROADMAP.md](docs/ROADMAP.md)): additional OSS-sourced tasks,
Java runtime/CI support, and write-through from `vulcanbench run` to the database.
