# QUICKSTART.md

One-command local setup and first run (v1).

## Prerequisites
- Python >=3.12
- Node.js >=20 (for dashboard)
- Docker Desktop (for real sandbox runs; optional for dry-run stubs)
- Git + Git LFS (`git lfs install`)

## Setup

```bash
git clone https://github.com/Zen-Open-Source/VulcanBench.git
cd vulcanbench
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
end-to-end before spending any tokens:

```bash
vulcanbench run --task hello-world --model mock:synthetic
vulcanbench run --suite v1 --model mock:synthetic --no-judges   # all 16 tasks
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
```

**3. Start small and cheap** — one task, in Docker, judges off, with a spend cap:

```bash
vulcanbench run --task py-topo-sort-cycle \
  --model openai:gpt-4o-mini \
  --sandbox docker --no-judges
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
> - `--judges` is **on by default** (a 3-model `human_like` ensemble reusing the
>   run model) — it roughly triples token cost/latency. Use `--no-judges` for
>   cheap functional-only runs.
> - `--max-cost` is a soft cap that stops launching new runs (suite runs only)
>   and requires a priced model; cost/latency are recorded per run regardless.
> - Default `--sandbox local` runs the agent's shell commands on **your host**.
>   Prefer `--sandbox docker` (or `auto`) for real models. Override prices any
>   time with `VULCANBENCH_PRICING=/path/to/prices.json`.

See the full example in the README and `docs/ARCHITECTURE.md`. To add your own
task: `docs/TASK_CONTRIBUTION.md`.
