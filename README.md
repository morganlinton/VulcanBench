# VulcanBench

[![CI](https://github.com/morganlinton/VulcanBench/actions/workflows/ci.yml/badge.svg)](https://github.com/morganlinton/VulcanBench/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![Python](https://img.shields.io/badge/python-3.12%2B-blue.svg)](https://www.python.org/)

VulcanBench is an open-source harness for measuring how well LLM coding agents
do real software engineering work. It runs a model against a task, keeps the
task's hidden tests away from the agent, grades the result deterministically,
and records everything: the full trace, the final patch, tokens, wall-clock,
cost, and a reproducible replay command. It measures a model either through a
raw API or through the product it ships in (Claude Code, Codex, Cursor, Grok
Build, ZCode, Muse Code, Devin CLI), at every reasoning-effort level the
provider exposes.

Published results, model cards and methodology live at
[vulcanbench.com](https://vulcanbench.com). The public record behind each
report is in [docs/results/](docs/results/).

## Quick start

```bash
git clone https://github.com/morganlinton/VulcanBench.git
cd VulcanBench
make setup
source .venv/bin/activate
vulcanbench --help
```

Prerequisites: Python 3.12 or newer, Git with Git LFS, and Docker Desktop for
real runs. Node 20 or newer only if you want the dashboard.

Confirm the harness works end to end without spending anything. The mock model
is deterministic and offline; `--sandbox local` is fine here because its
commands are canned:

```bash
vulcanbench run --task hello-world --model mock:synthetic --sandbox local
```

Then build the sandbox image once and run a real model. Real runs execute
model-written shell commands, so they default to a network-off Docker sandbox:

```bash
make sandbox-image                       # vulcanbench/sandbox:base (Python, Go, Node)
export ANTHROPIC_API_KEY=...             # or OPENAI_API_KEY, XAI_API_KEY, ...
vulcanbench run --task hello-world --model anthropic:claude-opus-5
```

Each run prints its scores and cost and writes
`./runs/<id>/{trace.jsonl, summary.json, replay.html, final.patch}`.
`final.patch` is a real `git diff` of the agent's edits and `replay.html` is a
self-contained replay you can open in any browser. Traces, summaries and patches
are secret-redacted and size-capped before they are written, so run artifacts
are safe to publish. See [docs/QUICKSTART.md](docs/QUICKSTART.md) for the
longer walkthrough.

## How a run is scored

Every run gets five metrics plus a weighted total. A metric is `null` with a
reason when its analyzer is unavailable; a score is never fabricated.

| Metric | Source |
|---|---|
| `functional` | Hidden `fail_to_pass` and `pass_to_pass` tests run after the agent finishes. 1.0 when every required test passes, otherwise proportional to the pass rate. Any regression guard failure zeroes it on the frontier suite. |
| `quality` | Lint and complexity of the changed files: ruff and radon for Python, `cargo fmt` and `clippy` for Rust, toolchain-dependent elsewhere. Reports call this factor lint and complexity so it is never confused with Code quality. |
| `security` | bandit for Python, `cargo audit` plus an unsafe-delta penalty for Rust, gosec for Go, npm audit for JS and TS. |
| `human_like` | Model-based code review. Off with `--no-judges`; choose the judge with `--judge-model` so a model never grades its own work. |
| `efficiency` | Derived from tokens and steps, lower is better. |

The harness total re-normalizes over whichever metrics are present
([harness/evaluator/scorer.py](harness/evaluator/scorer.py)). Published
VulcanBench Frontier v4 reports use a fixed combined score instead:

```
combined = 100 * (0.50 functional + 0.085 lint_and_complexity + 0.085 security + 0.33 code_quality)
```

`lint_and_complexity` is the `quality` metric above. Code quality is a third of the combined score and is measured by the
[Code quality protocol](#code-quality-judging) below, not by the run-time
judge. Time and cost are reported beside the score, never folded into it.
Details: [docs/METRICS.md](docs/METRICS.md).

Cost is recorded per run from a built-in pricing table (`VULCANBENCH_PRICING`
overrides it). Subscription runs record marginal cash, plan allocation, quota
and API-equivalent value separately and are never silently mixed with raw API
runs.

## Models and harnesses

A model is `provider:model`. Effort is `--effort minimal|low|medium|high|extra-high|max`;
each provider maps the labels it supports to its own field and records the
rest as metadata without sending them. Effort labels are each provider's own
scale, so a cross-provider comparison at the same label compares each model at
its own setting, not a calibrated equivalent.

### Raw APIs

| Provider | Spec | Key | Effort |
|---|---|---|---|
| OpenAI | `openai:<model>` | `OPENAI_API_KEY` | Responses API `reasoning.effort` when `--effort` is set; `minimal` and `max` supported |
| Anthropic | `anthropic:<model>` | `ANTHROPIC_API_KEY` | Messages API `output_config.effort`; `extra-high` maps to `xhigh` |
| xAI | `xai:<model>` | `XAI_API_KEY` | `reasoning_effort`; default is `high` and reasoning cannot be disabled, so sweeps should pass an explicit level |
| Meta | `meta:<model>` | `META_MUSE_SPARK_API` | `minimal` to `xhigh` map directly; an unset effort runs at a model-chosen level |
| DeepSeek | `deepseek:<model>` | `DEEPSEEK_API_KEY` | `low`, `high`, `max`; `medium` is recorded only |
| Alibaba Qwen | `qwen:<model>` | `DASHSCOPE_API_KEY` | `low`, `medium`, `xhigh`; `high` is recorded only |
| Moonshot Kimi | `kimi:<model>` | `MOONSHOT_API_KEY` | `extra-high` maps to `max`; others recorded only |
| Z.ai | `zai:<model>` | `ZAI_API_KEY` | recorded only |
| OpenRouter | `openrouter:<vendor>/<model>` | `OPENROUTER_API_KEY` | pinned to one upstream endpoint so a column stays one serving stack |
| Ollama | `ollama:<model>` | none | local inference through any OpenAI-compatible server; cost records as $0 and duration measures your hardware |
| Mock | `mock:synthetic` | none | deterministic, offline |

### Subscription harnesses

A harness run drives the product's own coding-agent CLI on a subscription while
VulcanBench keeps task preparation, the final diff, hidden verification and
scoring. The result measures the model plus its product harness. Every harness
records structured events, and every harness except Cursor streams token usage.

```bash
vulcanbench harness list                 # adapters and their execution boundaries
vulcanbench harness doctor codex         # installation and sign-in check, no model call
vulcanbench run --suite cii-v4 --harness codex --billing subscription \
  --model gpt-6-astra --effort high --sandbox docker --no-judges
```

| Harness | `--harness` | Signs in with | Notes |
|---|---|---|---|
| Claude Code | `claude-code` | Claude subscription | native permission auto mode on the host workspace |
| Codex CLI | `codex` | ChatGPT sign-in | `workspace-write` sandbox; can run inside a container with `--agent-container` |
| Cursor CLI | `cursor` | `cursor-agent login` | no token stream, so API-equivalent cost is recorded as unavailable |
| Grok Build | `grok-build` | `grok login` | custom kernel profile: workspace writes allowed, repository reads denied |
| ZCode | `zcode` | `zcode login` (GLM Coding Plan) | permission mode `yolo`, web tools removed |
| Muse Code | `muse-code` | Muse account | macOS outer sandbox, isolated session data, repository read denied |
| Devin CLI | `devin` | `devin auth login` (Devin account) | print mode, permission mode `dangerous`, web tools disabled; effort is the model id's last token (`swe-2-medium|high|max`); no API price, so cost is recorded as unavailable |

Muse Code runs only from a content-pinned binary so an auto-updating launcher
can never change the system under test mid-sweep:

```bash
export VULCANBENCH_MUSE_BINARY=/absolute/path/to/muse
export VULCANBENCH_MUSE_SHA256=<sha256 of that file>
vulcanbench harness doctor muse-code
vulcanbench run --suite cii-v4 --harness muse-code --model muse-spark-1.3 \
  --effort high --timeout 5400 --sandbox docker --no-judges
```

Muse requires an explicit model and effort and a positive wall-clock timeout,
and records the stream idle timeout it ran under. Per-harness behaviour,
verified CLI versions and boundaries: [docs/HARNESS_BENCHMARKING.md](docs/HARNESS_BENCHMARKING.md).

## Suites

| Suite | `--suite` | What it holds |
|---|---|---|
| VulcanBench Frontier v4 | `cii-v4` ([tasks/coding-intelligence-index-v4](tasks/coding-intelligence-index-v4/)) | 23 behavioural-reconstruction tasks. Each ships a retired compiled binary whose real behaviour departs from its written spec in documented ways, a naive rewrite made from the spec, and hidden tests captured from the binary. The agent must characterise the black box and make the rewrite match it. Every task passes a frontier admission gate ([CHARTER.md](tasks/coding-intelligence-index-v4/CHARTER.md)) and its verdict is logged in [CANDIDATES.md](tasks/coding-intelligence-index-v4/CANDIDATES.md). |
| Coding Intelligence Index v1 | `cii-v1` ([tasks/cii-v1](tasks/cii-v1/)) | 41 tasks mined from open-source pull requests merged after the measured models' training cutoffs, with complexity-scaled budgets, hidden fail-to-pass tests and regression guards. |
| v1 | `v1`, `v1-micro`, `v1-large`, `v1-diamond`, `v1-carbyne` ([tasks/v1](tasks/v1/)) | 52 gold-verified tasks across Python, Go, TypeScript and Rust in three difficulty tiers, plus the `hello-world` demo. Diamond and Carbyne tiers use rubric-graded mergeability with terse prompts. |
| v2, v3 | `v2`, `v3` | Earlier coding suites, kept so their archived reports stay reproducible. Results are only comparable within one suite. |
| VulcanCyber v1 | `vulcancyber-v1` ([tasks/vulcancyber-v1](tasks/vulcancyber-v1/)) | 16 defensive security tasks: real merged fixes for vulnerabilities, graded by the project's own regression tests. Defensive posture only. [docs/CYBER_EVAL.md](docs/CYBER_EVAL.md) |
| Voice v1 | `vulcanbench voice` ([tasks/voice-v1](tasks/voice-v1/)) | 200 held-out questions rendered through TTS under a voices, rate and noise matrix to measure the score a model loses when the same question arrives as speech. [docs/VOICE_EVAL.md](docs/VOICE_EVAL.md) |

Every task ships a starting `repo/`, hidden `tests/` never shown to the agent,
declarative `fail_to_pass` and `pass_to_pass` commands in `metadata.json`, a
`gold_patch.diff` reference solution, and labelled provenance (`source`,
`decontaminated`). Validation proves each task is real: the gold patch must
score 1.0, the fail-to-pass tests must fail before the fix, and grading must be
deterministic over repeated runs.

```bash
make validate-tasks                              # every task
vulcanbench validate-task tasks/v1/<id>          # one task
```

A task's functional score normally comes from hidden tests. A task can opt into
an agentic grader (`metadata.grader: "agentic"`) that judges the diff against
plain-English acceptance criteria the agent never sees, so the prompt can be as
terse as a real ticket; `scripts/grader_eval.py` reports a grader's accuracy,
false-pass rate and self-consistency on labelled cases before you rely on it.
Task format and contribution rules: [docs/TASK_CONTRIBUTION.md](docs/TASK_CONTRIBUTION.md).

## Code quality judging

The run-time `human_like` judge is a quick signal. Published Frontier v4 scores use
a separate, frozen protocol, [docs/judging/code-quality-maintenance-v3.md](docs/judging/code-quality-maintenance-v3.md),
because automated metrics reward compression and a model reads dense code for
free.

- **One rubric for every task, frozen by hash.** Six dimensions scored 0 to 4 in
  half steps for a named human reader: naming, presentation and intent (human
  readability), structure, changeability and verifiability (maintainability).
  Every score must cite an exact excerpt; the host computes the sub-scores.
- **Ground truth where it exists.** An intent-recovery probe asks the judge,
  given only the spec and the code, to list where the code departs from the
  spec; a separate call matches the list against a frozen answer key.
- **Neutral judges.** The scored panel comes from labs with no model on the
  board being compared, so no judge grades a relative. Each judge passes a
  calibration exam on ten held-out programs (clear, compressed, over-abstracted,
  misleadingly commented, and so on), five reviews each against twenty
  pre-declared gates, before it scores a single submission. A judge that fails
  is published as failed.
- **Every intervention on the record.** Protocol versions are frozen by hash
  and older versions run from a checkout at their freeze commit. The operator
  wrapper applies a small set of documented recovery rules and halts for a
  person on anything else; each application is logged with the receipt it
  touched.

```bash
python harness/maintenance_review_v3_resume.py --out runs-code-quality-maintenance-v3.4 --calibrate
python harness/maintenance_review_v3_resume.py --out runs-code-quality-maintenance-v3.4 --full
```

The plain-language description is [docs/judging/maintenance-v3-system-summary.md](docs/judging/maintenance-v3-system-summary.md);
the operations log is [docs/judging/maintenance-v3-operations.md](docs/judging/maintenance-v3-operations.md).

## Running a study without re-running everything

Grading is deterministic and every run records the task hash it was scored
against, so comparisons are queries over `./runs`, not re-runs.

```bash
vulcanbench estimate --suite cii-v4 --model anthropic:claude-opus-5      # spend before you run
vulcanbench run --suite cii-v4 --model anthropic:claude-opus-5 --repeat 3 --max-concurrency 4 --max-cost 50
vulcanbench run --suite cii-v4 --model anthropic:claude-opus-5 --effort high --only-missing --max-run-cost 2.50
vulcanbench effort-sweep --suite cii-v4 --model xai:grok-4.6 --efforts low,medium,high,extra-high
vulcanbench compare --suite cii-v4              # model x effort matrix from cached runs only
vulcanbench regrade runs/ --sandbox docker      # re-score against the current task definition at $0
vulcanbench leaderboard                         # pass@1 with standard error, pass@k, cost, latency
vulcanbench report -o report.md                 # shareable Markdown or JSON report
vulcanbench audit-runs runs/                    # web and filesystem leakage audit of CLI-harness runs
vulcanbench replay <id>
```

`--max-run-cost` stops a single run once its own spend crosses the cap and still
grades the partial result. `--only-missing` reuses fresh cached runs and
launches only the gaps. `--fail-under 0.8` turns a suite run into a CI gate
that fails closed: exit code 4 if pass@1 is below the threshold, unavailable,
or any run errored. Exit codes: `0` ok, `1` usage or error, `2` provider, `3`
sandbox, `4` gate failed.

## Sandbox

```bash
vulcanbench run --task <id> --model <spec> --sandbox docker|local|auto --image <tag> [--network]
```

`docker` (the default) runs the agent's commands and the functional verifier in
a non-root, network-off, resource-limited container and errors out if the daemon
is unreachable rather than falling back to the host. `local` runs commands on
the host and is meant for the mock model and trusted development loops. `auto`
uses Docker when available and refuses host execution unless
`VULCANBENCH_ALLOW_HOST_EXEC=1` is set. Resource floors and ceilings
(`--mem-floor`, `--cpu-ceiling`, `--pids-limit`) are recorded with the run.
`--agent-container` runs a subscription CLI itself inside a container built
from the sandbox image (`make agent-image-codex`) so the resource band bounds
the agent phase too. `make sandbox-image-all` builds the Rust and Go images the
security suite needs.

## Dashboard and API

Optional. The backend serves `./runs` as an API and the dashboard reads it:

```bash
pip install -e ".[backend]"
uvicorn backend.app:app --port 8000
cd dashboard && npm install && npm run dev      # http://localhost:3000
```

Set `DATABASE_URL` (Postgres or SQLite) for a durable store; `docker compose up
db` provides Postgres and `python scripts/ingest_runs.py` loads existing runs.
See [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md).

## Repository map

| Path | Contents |
|---|---|
| `harness/` | CLI, agent loop, providers, harness adapters, sandbox executors, evaluator, pricing, reports, Code quality judging |
| `tasks/` | Task suites, each with a charter and candidate log where it has an admission gate |
| `docs/` | Methodology, metrics, reproducibility, judging protocols, published results |
| `scripts/` | Task validation and mining, release figures, chart generators, calibration and gate tools |
| `sandbox/` | Docker images for command execution and verification |
| `backend/`, `dashboard/` | Optional API and web dashboard |
| `tests/` | Harness test suite |

Further reading: [ARCHITECTURE](docs/ARCHITECTURE.md), [METRICS](docs/METRICS.md),
[REPRODUCIBILITY](docs/REPRODUCIBILITY.md), [HARNESS_BENCHMARKING](docs/HARNESS_BENCHMARKING.md),
[CONTRIBUTING](CONTRIBUTING.md), [ROADMAP](docs/ROADMAP.md).

## Development

```bash
make ci            # ruff, mypy, pytest, task validation, as run in CI
make test          # fast tests only
make validate-tasks
```

Never introduce em or en dashes anywhere in the repository; CI rejects them.
Files bound by a frozen judging protocol hash are listed in `pyproject.toml`
and must not be edited or reformatted; a new protocol version gets a new
freeze instead.

## License

Apache 2.0 (see LICENSE and NOTICE).

## Provider terms and data usage

VulcanBench is an independent evaluation harness and is not affiliated with,
sponsored by, or endorsed by any model provider. Model and product names
identify the systems under test.

- **Bring your own keys and subscriptions.** Every call is made under your
  account and your agreement with that provider; VulcanBench never bundles or
  shares credentials. Staying within each provider's terms is your
  responsibility.
- **Outputs are for evaluation, not training.** Run artifacts capture model
  outputs solely for scoring, inspection and reproducibility. Providers
  prohibit using their outputs to train competing models; do not use
  VulcanBench artifacts, or any published corpus of them, for that purpose.
  There is deliberately no feature that exports outputs as a training dataset.

This is not legal advice; consult the current provider terms for authoritative
guidance.
