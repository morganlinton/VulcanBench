# VulcanBench — Gap-Closing Backlog

Status as of 2026-05-29. This is the prioritized work needed to take the current
**vertical-slice skeleton** to the **v1 MVP** described in `docs/SPEC.md` (the
original spec) and the implementation plan in `.kilo/plans/`.

Legend: 🟥 blocker for "can actually benchmark a model" · 🟧 needed for an honest
public v1 · 🟨 polish / completeness · ✅ done in this pass.

---

## Where the build actually landed (honest baseline)

What shipped before this pass:

- ✅ Working CLI scaffold (`run`, `leaderboard`, `replay`, `list-tasks`, stubs).
- ✅ Trace capture (JSONL) + self-contained HTML replay.
- ✅ Local file-based leaderboard aggregation from `./runs/`.
- ✅ Tool protocol (Pydantic models + ABC) and a local executor.
- ✅ One **synthetic** task (`hello-world`) driven by a **hardcoded** action list.
- ✅ Minimal read-only FastAPI backend (3 endpoints).
- ✅ Next.js dashboard with 3 pages rendering **hardcoded demo data**.

What did **not** exist (the core value of the project):

- ❌ Any real LLM integration — no provider clients, no agent loop. `--model
  openai:gpt-4o` printed "not yet implemented."
- ❌ Multi-metric evaluator (`harness/evaluator/` was empty).
- ❌ Docker sandbox (`harness/sandbox/` and top-level `sandbox/` were empty).
- ❌ Real tasks (1 of the 50 in spec; no repo snapshots, hidden tests, or gold patches).
- ❌ Postgres/SQLModel persistence, `POST /api/runs`, `POST /api/feedback`.
- ❌ Dashboard pages `/task/[id]` and `/submit`; live data fetching.
- ❌ Task tooling scripts (`validate_tasks.py`, `import_oss_issues.py`).
- ❌ 80%+ core test coverage (was ~38%, 2 smoke tests).

---

## P0 — Make it actually evaluate a model (the reason the project exists)

These three are addressed in this pass:

- ✅ **Generic LLM provider interface** (`harness/agent/providers.py`)
  - `mock:` (deterministic, offline, drives tests), `openai:<model>` (and any
    OpenAI-compatible `base_url`), `anthropic:<model>`.
  - Uniform `LLMResponse` with `content`, `tool_calls`, and token `usage`.
  - Stdlib-only HTTP (no new heavy SDK deps); keys from env.
- ✅ **Real tool-calling agent loop** (`harness/agent/loop.py::run_agent`)
  - System + issue prompt, up to `--max-steps` iterations, executes tool calls
    through the existing executor, records every LLM/tool/diff event, stops on a
    `finish` signal or when the model stops calling tools.
  - Produces a **real `git diff` patch** from the workspace.
- ✅ **Generalized task loader** (`harness/tasks.py`)
  - Loads any `tasks/v1/<id>/` (metadata, issue, optional `repo_snapshot.tar.gz`,
    verifier) so the loop is not special-cased to `hello-world`.
- ✅ **Full five-metric evaluator** (`harness/evaluator/`)
  - `functional` (verifier) + `efficiency` (tokens/steps) + `quality`
    (ruff + radon complexity/maintainability) + `security` (bandit; npm-audit /
    gosec / spotbugs for other langs when installed) + `human_like` (3-judge LLM
    ensemble, on by default reusing the run model) → weighted `total`.
  - Honest degradation: any metric is `null` + reason when its analyzer/judge is
    unavailable; per-metric breakdowns recorded to the trace and summary.

Remaining P0-adjacent:

- 🟥 **Provider robustness**: streaming, ret/timeout via `tenacity`, structured
  rate-limit handling, cost accounting per model. (Basic retry/timeout done; cost
  table is TODO.)
- 🟥 **Real reference task** with hidden tests + gold patch + `PASS_TO_PASS` /
  `FAIL_TO_PASS`, so a non-trivial run is meaningful.

## P1 — Honest, reproducible public v1

- ✅ **Docker sandbox executor** (`harness/sandbox/docker_executor.py`):
  `DockerToolExecutor` runs command execution in a non-root, network-off,
  resource-limited container (workspace bind-mounted, host UID/GID, always
  cleaned up); file ops stay host-side over the mount. `--sandbox
  {local,docker,auto}` + `--image`/`--network`; `sandbox/Dockerfile.base` +
  `task.Dockerfile.template` provided. *Follow-ups*: pin the base image by
  digest, multi-arch (ARM64 + x86) base.
- ✅ **In-container verification**: under `--sandbox docker`/`auto`, the
  functional verifier runs its test commands *inside* the container (via the
  executor's `exec`), not on the host — so a run is reproduced in one isolated
  environment. `harness/verifier.py` dispatches through a pluggable `Runner`
  (host by default; container under Docker). `sandbox/Dockerfile.base` now
  bundles Go + Node so Python/Go/TS tasks all verify in-image.
- ✅ **Suite spend cap** (`--max-cost N`): stops *launching* new runs once
  accumulated cost reaches `N` and records the rest in `skipped` (no silent
  truncation). Not a hard ceiling — in-flight runs finish, so the total can
  exceed `N` by up to `--max-concurrency` runs. Requires a priced model (rejects
  unpriced, exit 1) and a positive cap; suite-only. A budget-truncated suite
  fails a `--fail-under` gate closed (partial result).
- ✅ **Bounded parallel suite execution** (`--max-concurrency N`): the
  (task x repeat) grid runs on a thread pool of fully isolated `run_agent` calls
  (own run dir / git workspace / collector / provider). Results are identical to
  sequential (a property test asserts it); one unit's failure is contained in
  `errors` and doesn't sink the suite. *Note*: the local/mock path is live-tested
  here; the Docker+parallel combo shares the same code path but isn't exercised
  without a daemon (CI's docker job builds the image).
- ✅ **`--fail-under` CI gate**: `vulcanbench run ... --fail-under T` exits `4`
  when pass@1 < `T`. It **fails closed** — an unavailable pass@1 *or any errored
  suite run* also fails the gate (a CI gate must never go green on a partial
  result). `T` is validated to be a finite number in `[0, 1]` (rejects `nan`/
  `inf`/out-of-range).
- ✅ **`vulcanbench report`** (`harness/report.py`): a shareable JSON/Markdown
  results report — per-model ranking (pass@1 ± stderr, pass@k, cost, latency),
  per-task breakdown, environment summarized from run manifests, and an
  integrity section that flags runs scored against a now-stale task version.
- ✅ **Statistical rigor — repeated runs** (`--repeat N` on `run`/`run --suite`):
  each task runs N times; the leaderboard reports **pass@1 ± stderr** and
  **pass@k** (per-task solve rates averaged over tasks), not a single noisy run.
  `aggregate_by_model` distinguishes distinct tasks (`n_tasks`) from attempts
  (`n_runs`); a suite aggregate counts only that invocation's runs (suite_id).
- ✅ **Suite runs + per-model aggregate leaderboard** (`harness/suite.py`,
  `harness/pricing.py`, `harness/leaderboard.py`): `vulcanbench run --suite v1`
  runs a model over every task; per-run **cost** (`pricing` table, overridable,
  `null` for unknown models) + **latency** captured; `aggregate_by_model` ranks
  models (avg score, pass rate, cost, time). Model view is the default in the
  CLI (`leaderboard --by model|run`), API (`?by=model`), and dashboard.
- ✅ **Seed tasks + task format** (`tasks/v1/`): **16** multi-file, gold-verified
  tasks across Python/Go/TypeScript with a formalized format — `repo/` start
  state, hidden `tests/`, declarative `fail_to_pass`/`pass_to_pass`,
  `gold_patch.diff`. Declarative verifier in `harness/verifier.py`.
- ✅ **Scaled dataset + difficulty/category spread**: grew 6 → 16 with `medium`
  and `hard` tasks and new categories (`refactor`, `concurrency`) so the
  benchmark discriminates between weak and strong models — e.g. an RFC 6901 JSON
  Pointer resolver (hard), a `-race`-verified bounded worker pool (hard), a
  prototype-pollution-safe deep merge (hard). Plus the first **genuinely
  OSS-sourced** task (`oss-inflection-titleize`, verbatim MIT upstream + real
  fix commit), honestly `decontaminated: false`. Decontamination is now
  *mechanical*: a `decontaminated` boolean enforced by the validator
  (hand-authored ⇒ true; oss ⇒ URL + commit/issue ref + preserved LICENSE), and
  the `report` integrity section flags runs scored against non-decontaminated
  tasks. *Follow-ups*: ~~empirical difficulty calibration~~ (done — `vulcanbench calibrate`);
  Java (needs a runtime + CI); more OSS-sourced tasks; scale toward 50.
- ✅ **`scripts/validate_tasks.py`** (`harness/validate.py`): schema, toolchain-
  aware skip, gold-solves (`functional==1.0`), fail-to-pass-is-real (pre-patch
  `<1.0`), determinism (3×), contamination heuristics. Wired into CI (with Go +
  Node) and the `validate-task` CLI / `make validate-tasks`.
- ✅ **Postgres persistence** (`backend/db.py`, SQLModel): when `DATABASE_URL` is
  set, the API reads/writes a database (Postgres in prod, SQLite in tests);
  otherwise it falls back to scanning `./runs/` (no behavior change for local
  use). `POST /api/runs` records a summary, `POST /api/feedback` stores feedback,
  and `scripts/ingest_runs.py` bulk-loads existing runs. *Follow-ups*: Alembic
  migrations, a compose backend service, write-through from `vulcanbench run`.
- 🟧 **80%+ core coverage**: unit tests for providers (mock), loop, evaluator,
  tasks, leaderboard, CLI; raise `--cov-fail-under`. (Tests added this pass move
  the needle; finish the long tail.)

## P2 — Completeness vs. the original spec

- ✅ **Full multi-metric evaluator**: quality (ruff + radon), security (bandit /
  npm-audit / gosec / spotbugs), and the 3-judge `human_like` LLM ensemble.
  *Follow-ups*: complexity/maintainability for non-Python langs; baseline
  comparison against `expected_metrics.json`. (Per-model cost accounting,
  including judge cost, shipped with the suite/leaderboard work.)
- ✅ **Dashboard `/tasks`, `/task/[id]`, `/submit`** pages: a task browser, a
  per-task page (metadata + issue + all runs), and a submission form that
  pre-fills a GitHub task-submission issue. Backend gains `/api/tasks`,
  `/api/task/{id}`, `/api/run/{id}/patch`.
- ✅ **Trace viewer**: per-run timeline + a colorized `final.patch` diff viewer +
  replay link, fed by the live endpoints.
- ✅ **`POST /api/feedback`** + task/run rating storage (DB-backed; see P1 Postgres).
- ✅ **Reproducibility manifest**: every run summary records a `manifest` —
  Python/platform, the sandbox mode + image + network, the model + judge model,
  and the versions of git/ruff/bandit/radon/go/node on the box. *Follow-ups*:
  pin exact image digests; pin the `docker-compose.yml` Postgres digest.
- 🟨 Task-submission flow: ✅ `scripts/import_oss_issues.py` scaffolder, ✅
  `.github/ISSUE_TEMPLATE/task_submission.md` (+ dashboard `/submit`). *Remaining*:
  auto-labeling + task-PR validation CI.
- 🟨 Scale dataset toward the spec's 50 tasks; add `docs/SPEC.md`.

## P3 — Hardening

- ✅ **Benchmark integrity — task content hashing** (`harness.tasks.task_hash`):
  every run records a deterministic sha256 of the task's *scoring-relevant*
  definition — repo/ (or snapshot), the issue/prompt (`issue.md`), hidden
  tests/, `metadata.tests`, the legacy `verifier.py`, and the gold patch.
  Cosmetic metadata edits don't change it; changing the prompt, any test/source
  file, or the scoring logic does.
  `leaderboard.mark_stale` flags runs scored against a task version that no
  longer matches the current definition (drift detection).
- ✅ Tightened the workspace path-escape guard to `Path.is_relative_to`
  (`str.startswith` let `/ws-evil` masquerade as inside `/ws`).
- ✅ **Safe, bounded, publishable runs** (`harness/redaction.py`): traces,
  summaries, and `final.patch` are passed through secret redaction (credential
  shapes + known secret env values) and a per-field length cap before they touch
  disk; `--timeout SECONDS` enforces a per-run wall-clock budget that aborts the
  loop cleanly (`budget_exceeded` event, `finished=false`).
- 🟨 Replace broad `except Exception: pass` with scoped handling + trace logging.
- 🟨 Pin all container/base images by digest; supply-chain scan in CI.

---

## Wired in this pass (deliverables 2 & 3)

1. `harness/agent/providers.py` — generic provider interface (mock/OpenAI/Anthropic).
2. `harness/agent/loop.py` — `run_agent`, a real tool-calling ReAct loop.
3. `harness/tasks.py` + `harness/evaluator/scorer.py` — task loading + scoring.
4. `harness/cli.py` — `run` now dispatches real models through the loop.
5. `backend/app.py` — adds `total`, a live trace endpoint, and static replay serving.
6. `dashboard/` — leaderboard + run pages now **fetch live data** from the backend
   with a graceful empty-state fallback.
7. `tests/` — provider/loop/evaluator/leaderboard/CLI coverage.
