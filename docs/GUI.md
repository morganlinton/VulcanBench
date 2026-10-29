# The local run GUI

A localhost page for watching (and, in later milestones, launching) benchmark
runs, so the operator stops driving them by hand. It lives in the existing
backend (`backend/runner/`) and dashboard (`dashboard/app/runs`), wraps the
existing CLI and scripts as subprocesses, and reads what they write to disk.
It never reimplements run logic.

## What it shows today (milestone 1, read-only)

The Runs page at `http://127.0.0.1:3000/runs` lists every run under the
configured roots, running or finished, whether the GUI launched it or not:

- Frontier v5 gate jobs: `<gate_out>/jobs/<job>/` as written by `harbor run`
  through `scripts/frontier-v5/run_gate.sh`. One row per job with the task,
  model, harness, effort, bench tag and commit of the checkout the task came
  from, attempt k of n, solves, elapsed time, and a reward dot per trial.
- Harness runs: `<root>/<run_id>/` as written by `vulcanbench run`
  (`trace.jsonl` while running, `summary.json` at the end), from the GUI's own
  output root and any `extra_run_roots`.

A run detail page shows each attempt with its full `reward.json`, the
containers still attached to it (`docker ps`, matched by compose project), the
first-attempt check, and a live tail of the trial log, the agent stream and
the verifier output. Log text passes through the redactor before it leaves the
backend, and nothing under `agent/sessions` or `agent/setup` is reachable.

### How a run's state is decided

Harbor never stamps `finished_at` on a job it cancelled, and `run_gate.sh`
(before milestone 2) writes no status file, so the page never infers "running"
from the absence of a finish time. A gate job is **running** only while a
`harbor run ... --job-name <job>` process is alive. With no process it is
**finished** when `result.json` has `finished_at`, otherwise **stopped**, with
a note saying why. A `<job>.status.json` beside the job (written by the
milestone 2 version of `run_gate.sh`) takes precedence when present. A harness
run with `summary.json` is finished; one whose trace is still growing is
running; one whose trace went quiet for 15 minutes without a summary is stopped.

### The first-attempt check

Two of the three infrastructure failures in the Frontier v5 gate log produced
one synthetic message and reward 0, which looks like an ordinary failed
attempt. The check reads the agent stream of every attempt and reports:

- **session ok**: tool calls were made and every assistant turn is on the
  requested model.
- **checking**: no tool calls yet, and the agent started less than
  `first_check_after_s` (default 180 s) ago.
- **check failed**: an assistant turn on a different model (the refusal
  fallback), a fallback event, an infrastructure error pattern (the same
  patterns `run_gate.sh` greps for), or no tool calls after the window.
- **check warning**: a rate-limit status other than `allowed`, or missing
  evidence.

For Claude Code the model comes from the stream itself; for Codex it comes
from the `codex exec ... --model` line in `trial.log` and the command count
from the JSON stream. The check only flags; it never stops a run.

## Starting it

```bash
make gui            # backend on 127.0.0.1:8000 with VULCANBENCH_RUNNER=1, dashboard on 127.0.0.1:3000
```

or separately, `make gui-backend` and `make gui-dashboard` (the dashboard
needs `npm install` in `dashboard/` once). The runner routes answer only when
`VULCANBENCH_RUNNER=1` is set and only to loopback clients; without the flag
the backend behaves exactly as the public deployment does.

## Configuration

`[runner]` in `vulcanbench.toml`, every key overridable with
`VULCANBENCH_RUNNER_<KEY>`:

| key | default | meaning |
|---|---|---|
| `home` | `~/vulcanbench-runner` | runner state and the output root (`runs/`) for harness runs the GUI launches |
| `gate_out` | `~/vulcanbench-gate` | `GATE_OUT`; `<gate_out>/jobs` is scanned |
| `run_worktrees` | `../VulcanBench-runs` | where `scripts/run_worktree.sh` puts bench-tag worktrees |
| `extra_run_roots` | `[]` | more harness output roots to show read-only |
| `first_check_after_s` | `180` | window for the first-attempt check |

## Rules the GUI enforces (not just displays)

- Every launch (milestone 2 onward) runs from a worktree created by
  `scripts/run_worktree.sh`, detached at a `bench/...` tag, with a clean tree;
  never from the checkout being edited and never with `--allow-dirty`. The
  child environment is an allowlist, so `VULCANBENCH_ALLOW_DIRTY` cannot leak in.
- Effort levels under `[effort].blocked` never appear as options and are
  refused server-side.
- Nothing that spends model quota starts without an explicit confirm click
  that shows the estimate; there is no auto-start and no auto-retry.
- Gate runs go only through `scripts/frontier-v5/run_gate.sh`.
- Credentials are never read, displayed, logged or sent to the browser.
- Loopback only; no auth system, no remote exposure.

## Milestones

1. Read-only Runs page and first-attempt check (this document).
2. Queue and supervisor (one run at a time, survives a backend restart, clean
   stop), gate launches with the usage pre-flight and a confirm step, and the
   `run_gate.sh` hardening that makes a clean stop possible.
3. Harness launches with `vulcanbench estimate --json`, effort picker, and
   `--only-missing` as an explicit resume.
4. Polish: orphan-container cleanup, bench-tag cutting, streaming log tail.
