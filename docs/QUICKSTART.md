# QUICKSTART.md

One-command local setup and first run (v1).

## Prerequisites
- Python >=3.12
- Node.js >=20 (for dashboard)
- Docker Desktop (for real sandbox runs; optional for dry-run stubs)
- Git + Git LFS (`git lfs install`)

## Setup

```bash
git clone https://github.com/morganlinton/VulcanBench.git
cd VulcanBench
make setup
source .venv/bin/activate
vulcanbench --help
```

Dashboard:
```bash
cd dashboard
npm install
npm run dev
# open http://localhost:3000
```

## Smoke test (offline, free)

`mock:synthetic` is deterministic and free — use it to confirm the harness runs
end-to-end before spending any tokens. `--sandbox local` skips Docker, which is
fine here because the mock model's commands are canned (real models default to
the Docker sandbox):

```bash
vulcanbench run --task hello-world --model mock:synthetic --sandbox local
vulcanbench run --suite v1-micro --model mock:synthetic --no-judges --sandbox local
```

## Your first real run

**1. Build the sandbox image** (real runs execute model-written shell commands —
run them in Docker, not on your host):

```bash
make sandbox-image            # builds vulcanbench/sandbox:base (Python+Go+Node)
```

**2. Set the provider key** for the model you'll test:

```bash
export OPENAI_API_KEY=sk-...           # for openai:* models
export ANTHROPIC_API_KEY=sk-ant-...    # for anthropic:* models
export ZAI_API_KEY=...                 # for zai:* models (GLM)
```

**3. Start small and cheap** — one task, in Docker (the default), judges off,
with a spend cap:

```bash
vulcanbench run --task py-topo-sort-cycle \
  --model openai:gpt-4o-mini \
  --no-judges
vulcanbench run --task py-topo-sort-cycle \
  --model zai:glm-5.2 \
  --no-judges
vulcanbench replay --run-id <latest>   # self-contained HTML trace of the run
```

**4. Scale to the whole suite** with repeats (for pass@k ± stderr), bounded
parallelism, and a hard spend cap, then build a shareable report:

```bash
vulcanbench run --suite v1 \
  --model anthropic:claude-sonnet-4-6 \
  --sandbox docker --repeat 3 --max-concurrency 4 --max-cost 5.00
vulcanbench report --suite v1 -o report.md
```

> **Cost & safety notes**
> - Estimate spend before a benchmark (uses local `./runs` history when present,
>   bundled priors on fresh installs):
>   `vulcanbench estimate --suite v1-compare --model openai:gpt-5.5`
> - Preflight task health before a full suite spend:
>   `make validate-tasks-docker` (gold + verifiers inside Docker; builds base + Rust images)
> - `--judges` is **on by default** (a 3-model `human_like` ensemble reusing the
>   run model) — it roughly triples token cost/latency. Use `--no-judges` for
>   cheap functional-only runs.
> - `--max-cost` is a soft cap that stops launching new runs (suite runs only)
>   and requires a priced model; cost/latency are recorded per run regardless.
> - `--max-run-cost` is a **per-run** hard ceiling: an individual agent run stops
>   once its own spend crosses the value (overshoot bounded by one model call),
>   the summary records `cost_capped: true`, and the partial result is still
>   graded honestly. Ideal for the hard tier, where a failing run can otherwise
>   ruminate to the step cap — `--max-run-cost 2.50` turns "$8 DNF" into "$2.50
>   DNF" and loses no signal (works on single `--task` and suite/sweep runs).
> - Default `--sandbox docker` runs the agent's shell commands in an isolated
>   container. Use `--sandbox local` only for trusted dev loops (e.g.
>   `mock:synthetic`). Override prices any time with
>   `VULCANBENCH_PRICING=/path/to/prices.json`.

## Run on your Claude subscription instead of the API (`claude-code:`)

If you have a Claude Pro/Max subscription, `claude-code:<model>` specs run the
task with **Claude Code headless** (`claude -p`) instead of the VulcanBench
agent loop — billing your subscription, not API rates:

```bash
# Requires Claude Code installed and signed in with your subscription
# (run `claude` once interactively, or set CLAUDE_CODE_OAUTH_TOKEN).
vulcanbench run --task py-topo-sort-cycle \
  --model claude-code:claude-opus-4-8 \
  --judge-model claude-code:claude-opus-4-8 \
  --sandbox local
```

What to know before using it:

- **You're benchmarking model + vendor harness**, not the uniform VulcanBench
  loop. `claude-code:claude-opus-4-8` results are *not comparable* to
  `anthropic:claude-opus-4-8` columns — the summary records
  `cli_agent.harness` so they can't be silently mixed. Use it for cheap dev
  iterations, task authoring, and smoke tests; keep API runs for published
  cross-provider numbers.
- **`cost_usd` is hypothetical.** It's what the same tokens would have cost at
  API rates (`claude-code:` prices map to `anthropic:` prices), so you can see
  what a run *would* have cost. `cli_agent.billing: "subscription"` and the
  CLI's own `cli_reported_cost_usd` are recorded alongside.
- **`--sandbox local` is required.** Claude Code executes its own tools on
  your host (that's the harness being benchmarked); the docker sandbox would
  verify in a different environment than the agent ran in.
- **Subscription limits are run errors, not zeros.** If a Max 5-hour window or
  weekly cap is hit mid-suite, the run records an error instead of a 0 score;
  resume the gaps later with `--only-missing`.
- **Judges/graders can ride the subscription too**: pass
  `--judge-model claude-code:<model>` (single-shot `claude -p` calls). Or point
  `--judge-model` at a cheap API model (e.g. `anthropic:claude-haiku-4-5`) —
  judge calls are a small fraction of run cost.
- `ANTHROPIC_API_KEY` is stripped from the CLI subprocess so a set key can
  never silently flip the run onto API billing. `--max-run-cost` still works
  (enforced against the hypothetical cost, mid-run); `--effort` is recorded
  but not sent (headless Claude Code has no effort control).

**Re-grade for free after a task changes.** Grading is deterministic, so when you
edit a task's hidden tests or thresholds you don't need to re-run the model —
just re-grade the existing runs. `regrade` rebuilds each run's workspace from the
task base plus the captured agent patch, overlays the *current* tests, and
re-verifies in the sandbox at zero API cost:

```bash
# one run, or an entire directory of runs
vulcanbench regrade runs/<run-id> --sandbox docker
vulcanbench regrade runs/ --sandbox docker            # regrades every run under it
```

It prints old → new functional per run and writes `regrade.json` into each run
dir. Use it to re-score historical runs against a calibrated task without paying
for a single new model call.

**Add a model to a comparison without re-running the baselines.** Because grading
is deterministic and every run records the `task_hash` it was scored against, a
model×effort comparison is a *query over cached runs*, not a re-run. `compare`
builds the matrix for a frozen suite from `./runs`, excluding any run scored
against an older task definition (so results stay apples-to-apples):

```bash
vulcanbench compare --suite v2                     # complete cells only
vulcanbench compare --suite v2 --incomplete        # show gaps + how to fill them
```

The suite is "frozen" implicitly by its task hashes (`compare` prints a short
version id); change a task and its cached runs go stale and drop out. To add a
model, run just that one model against the suite, then re-run `compare` — the
baselines come from cache. That turns a new-model report from a full-matrix
re-run into a single new column, and pairs naturally with `--max-run-cost` to
bound the cost of that one column.

`--only-missing` closes the loop: on a suite run it skips tasks that already have
enough fresh cached runs for that model+effort and launches only the gaps, so an
interrupted or partial column is *resumed* rather than re-run. Stale cached runs
(scored against an older task definition) are ignored and re-run.

```bash
# run a column; if it's interrupted or you add tasks, re-run to fill only the gaps
vulcanbench run --suite v2 -m anthropic:claude-opus-4-8 --effort high \
  --only-missing --max-run-cost 2.50 --sandbox docker
```

> **Keep runs in one place.** Both the cache reuse (`--only-missing`) and the
> comparison (`compare`) only see runs under the directory they scan — the
> `--output-dir` of the run for `--only-missing`, and `--runs-dir` for `compare`
> (both default to `./runs`, scanned recursively). Runs written to *other*
> directories are invisible to that lookup, so `--only-missing` will re-run a
> cell whose cached result lives elsewhere, and `compare` will show it as
> missing. Point every run at the same `--output-dir` (or consolidate run dirs
> under one root before comparing) so the cache lookup sees the full history.
> Nesting is fine — sub-directories are discovered — the runs just have to be
> under the one root you scan.

See the full example in the README and `docs/ARCHITECTURE.md`. To add your own
task: `docs/TASK_CONTRIBUTION.md`.
