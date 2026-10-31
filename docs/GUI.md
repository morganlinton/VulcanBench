# The local run GUI

A localhost page for watching and launching benchmark runs, so the operator
stops driving them by hand. It lives in the existing backend
(`backend/runner/`) and dashboard (`dashboard/app/runs`,
`dashboard/app/launch`), wraps the existing CLI and scripts as subprocesses,
and reads what they write to disk. It never reimplements run logic.

## Runs page (milestone 1)

The Runs page at `http://127.0.0.1:3000/runs` lists every run under the
configured roots, running or finished, whether the GUI launched it or not:

- Frontier v5 gate jobs: `<gate_out>/jobs/<job>/` as written by `harbor run`
  through `scripts/frontier-v5/run_gate.sh`. One row per job with the task,
  model, harness, effort, bench tag and commit of the checkout the task came
  from, attempt k of n, solves, elapsed time, and a reward dot per trial.
- Harness runs: `<root>/<run_id>/` as written by `vulcanbench run`
  (`trace.jsonl` while running, `summary.json` at the end), from the GUI's own
  output root and any `extra_run_roots`.
- Queued jobs that have no job directory yet.

A run detail page shows each attempt with its full `reward.json`, the
containers still attached to it (`docker ps`, matched by compose project), the
first-attempt check, and a live tail of the trial log, the agent stream, the
verifier output and, for GUI-launched runs, the console log. Log text passes
through the redactor before it leaves the backend, and nothing under
`agent/sessions` or `agent/setup` is reachable.

### How a run's state is decided

Harbor never stamps `finished_at` on a job it cancelled, so the page never
infers "running" from the absence of a finish time. For a job the GUI
launched, the queue's record decides. For any other job, `<job>.status.json`
beside it (written by `run_gate.sh` since 2026-10-29) decides when present;
otherwise the job is **running** only while a `harbor run ... --job-name
<job>` process is alive, **finished** when `result.json` has `finished_at`,
and **stopped** with a note saying why in every other case. A harness run
with `summary.json` is finished; one whose trace is still growing is running;
one whose trace went quiet for 15 minutes without a summary is stopped.

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

## Benchmark a model (milestone 3)

The Launch page's main tab. A new model comes out; the operator types its id,
picks the harness, the billing mode, the suite (VulcanBench Frontier v4 by
default), the effort levels and the bench tag, and gets one queued job per
level:

    <worktree>/.venv/bin/vulcanbench run --suite <suite> --model <m> --harness <h>
        --billing <b> --effort <level> --repeat <k> --sandbox <s> --no-judges
        --max-concurrency 1 --max-infra-retries 0 --only-missing -o <runner home>/runs/<label>/<level>

That is the command the hand-run sweep scripts under `scripts/cii-v4-board/`
use, minus their retry loops. What happens before confirm, none of it a
model call:

1. The worktree for the tag must already exist and be clean. When it does
   not, the page offers **Prepare the worktree**, a queue step that runs
   `scripts/run_worktree.sh <tag>` and nothing else (make setup takes
   minutes; watch it on the Runs page).
2. **Build plan** runs `vulcanbench harness doctor <harness> --json` from the
   worktree (CLI installed, logged in) and `vulcanbench estimate --json` for
   the suite and model (list price, bundled priors). "No built-in pricing"
   is the signal that the model is not onboarded at that tag: the plan warns,
   because onboarding (the price entry in `harness/pricing.py`, a pinned CLI
   when the vendor requires one, a decision entry) is a PR and a new tag,
   not something the GUI does.
3. The plan shows every level's command, the estimate per level and in all,
   the longest possible wall time (runs times the 3-hour task timeout), the
   sandbox default for the harness, and the quota rule chosen. Confirm is
   disabled while the doctor says the harness is not ready.
4. **Confirm** queues the levels. The supervisor runs them one at a time.

Progress lives on the Runs page under **Model sweeps**: per level, runs done
of total, passed and the pass rate, cost so far from the run summaries, the
current task with how far into its 3-hour budget it is (and a warning when
its trace has been quiet for 15 minutes), an ETA from the average run so far,
and Stop. Each run is also a row of its own, owned by its level, with the
first-attempt check and the log tail.

A level that ends with tasks still missing is **incomplete** and stays that
way; **Run the missing tasks** on the sweep card prefills a new plan for the
same output directory with `--only-missing`, which needs its own confirm. The
one continuation that happens without a click is the quota rule set on the
plan: after a subscription window or rate limit the level **waits** 30
minutes and runs `--only-missing` again, at most the number of times chosen
(default zero). An expired login, a provider error, a sandbox error or a
crash never continues (docs/DECISIONS.md 2026-10-30).

Stop on a suite level: SIGINT to `vulcanbench` (finish the in-flight task),
the agent CLI's own session is killed two seconds later, the harness grades
what it has and tears down, and the job's process group is torn down if it is
still there after ten minutes. The sweep afterwards touches only containers
and scratch directories of this level's unfinished runs.

The **Pinned CLI directory** field (under "advanced") prepends a directory to
the child's PATH, for a model that needs a newer or pinned harness CLI
(`CODEX_BIN_DIR` in the scripts). Login homes the harness reads
(`CODEX_HOME`, `CLAUDE_CONFIG_DIR`, `GROK_HOME`, `ZCODE_*`) pass through
when set; keys never do. Metered API keys reach the harness the way they do
by hand: `load_dotenv()` on the worktree's `.env`.

## Housekeeping and the live tail (milestone 4)

- **Orphaned sandboxes** on the Runs page: containers that look like
  benchmark sandboxes (harbor's `<task>__<trial>__env` compose projects, the
  harness's `vb-<run id>` projects, `vb-agent-*` containers, `vulcanbench/*`
  images) that no active run owns, each with the run it belonged to and why
  it counts. **Remove the listed containers** re-checks the list at that
  moment and removes only those ids (and their compose networks). Nothing is
  removed by image or prefix while a run that could own it is alive.
- **Cut a bench tag** on the Launch page wraps `scripts/cut_bench_tag.sh
  <label> [--push]` in the backend's own checkout. The script's guards decide
  (on main, clean, equal to origin/main); the card shows what they will say
  before the click and the script's output after it, and the new tag becomes
  the selected tag.
- The log tail on a live run page streams server-sent events
  (`GET /runs/{id}/log/stream`): the tail once, then only the bytes
  appended, through the redactor. If the stream cannot be opened the page
  polls every 3 seconds as before.

## Frontier v5 gate runs and the queue (milestone 2)

The Launch page's second tab plans a Frontier v5 gate run:

1. Pick the reference (`astra`, `fable`, `opus`, read from `run_gate.sh
   --refs`), the bench tag, the task at that tag, attempts (1 to 3),
   concurrency (the task's default, or an override) and an optional job
   suffix. Effort is `medium`, fixed by the script; blocked levels are never
   offered anywhere.
2. **Build plan** asks the backend for the exact command. The backend
   validates the worktree (see the rules below), refuses an existing job
   directory, and returns the argv, the working directory, the environment
   variables the child gets, an estimate (attempts, task timeout, longest
   wall time, which subscription pays, early exit, no retries), warnings, and
   a hash of all of that. A plan expires after 10 minutes.
3. **Check usage** (Claude references only) runs `GATE_PREFLIGHT_ONLY=1
   run_gate.sh <ref>`: one one-turn call that reads the subscription windows.
   It runs only on this click, because it spends quota. Confirm stays
   disabled until a check from the last 10 minutes says the script would
   start.
4. **Confirm and queue this run** sends the plan's hash back. The backend
   re-validates the worktree, then queues the job. Nothing else starts a run.

The queue, shown at the top of the Runs page, runs one job at a time. The
supervisor thread in the backend (one per machine, guarded by
`<runner home>/supervisor.lock`) starts the oldest queued job when no owned
job is active and no `harbor run` or `vulcanbench run` launched outside the
GUI is visible, re-validating the plan right before the child starts. If the
plan's worktree does not exist yet, the job first runs
`scripts/run_worktree.sh <tag>` (shown as a separate step in the plan and
as the **preparing** status), then the gate command. The worktree's
`make setup` output lands in `<worktree>/.venv-setup.log`; when it fails, the
job's console log shows the tail and the job ends as **failed**. A half-made
worktree has to be removed by hand (`git worktree remove <dir>`) before the
same tag can be planned again.

**Stop** on a queue row or a run page persists a stop request. The supervisor
sends SIGINT to the script; the script's trap cancels harbor (which runs
`docker compose down`), waits up to 90 s, escalates to SIGTERM then SIGKILL,
removes the task's sandbox containers and writes the status file. If the
script is still there 150 s after the request, the supervisor sends SIGTERM
and then SIGKILL to the job's process group and removes any containers whose
compose project belongs to this job. **Remove** takes a queued job out of the
queue before it starts. There is no retry button and no automatic retry: a
failed, stopped or lost job stays that way until the operator plans a new
run.

The queue survives a backend restart. Jobs are recorded in
`<runner home>/runner.db` (stdlib sqlite, outside every checkout) with the
child's pid, start time, boot time and an argv token; on start the supervisor
adopts jobs whose process is still the same one, and gives the others their
final state from the script's status file.

### Rules the GUI enforces (not just displays)

- Every launch runs from a worktree under `[runner].run_worktrees` created by
  `scripts/run_worktree.sh`, detached at the plan's `bench/...` tag, with no
  uncommitted changes under `harness`, `tasks`, `sandbox`, `vulcanbench.toml`,
  `pyproject.toml` or `uv.lock`; never the backend's own checkout and never
  with `--allow-dirty` (the argv is checked for it at confirm and launch).
- The child environment is an allowlist: `HOME`, `PATH`, `USER`, `LANG`,
  `TMPDIR`, `CLAUDE_GATE_ENV`, `FABLE_AUTH` pass through; `GATE_OUT`,
  `GATE_ATTEMPTS`, `GATE_CONCURRENCY`, `GATE_JOB_SUFFIX` come from the plan.
  `VULCANBENCH_*` (so `VULCANBENCH_ALLOW_DIRTY` cannot leak in),
  `DATABASE_URL`, `*_API_KEY`, `*_TOKEN` and `*SECRET*` never pass.
- Effort levels under `[effort].blocked` never appear as options and are
  refused server-side, at plan time and again at launch. Gate runs are fixed
  at `medium` by the script.
- A suite run never re-queues a unit on its own (`--max-infra-retries 0`),
  and the GUI never retries a failed, incomplete, stopped or lost job. The
  only continuation is the quota rule on the plan, off by default.
- Nothing that spends model quota starts without the confirm click that
  shows the estimate and the exact command; there is no auto-start and no
  auto-retry.
- Gate runs go only through `scripts/frontier-v5/run_gate.sh`.
- Credentials are never read, displayed, logged or sent to the browser. The
  backend checks only whether `~/.codex/auth.json` and the `CLAUDE_GATE_ENV`
  file exist; the script and harbor read them.
- Loopback only; no auth system, no remote exposure. The runner routes exist
  only with `VULCANBENCH_RUNNER=1` and answer 403 to any other client address.

### What run_gate.sh gained for this

`scripts/frontier-v5/run_gate.sh` keeps every stdout line the GATE-LOG
workflow reads and adds: harbor as a direct child with SIGINT at its default
disposition, traps on INT and TERM that stop the run cleanly (exit 130), an
atomic `$GATE_OUT/<job>.status.json` with the phase (`preflight`, `running`,
`stopping`, `done`, `infra_failed`, `stopped`), pids, solves, harbor's exit
code and the stop reason, `--refs`, `GATE_PREFLIGHT_ONLY=1`, and
`GATE_POLL_S` for its tests. Older bench tags carry the older script; the
plan warns when that is the case, and a stop then signals harbor directly.

## Dry run without spending quota

The whole launch path can be exercised with a stub harbor. Start the backend
with a scratch `HOME` whose `.local/bin/harbor` is a small script that writes
the job layout and sleeps until SIGINT (`run_gate.sh` puts `$HOME/.local/bin`
first on PATH), empty `~/.codex/auth.json` and `claude-gate.env` placeholders
(they are only checked for existence), and `VULCANBENCH_RUNNER_HOME`,
`VULCANBENCH_RUNNER_GATE_OUT` and `VULCANBENCH_RUNNER_RUN_WORKTREES` pointing
at scratch directories; tag the commit under test as a local `bench/...` tag
and plan a run from it. Do not click **Check usage** in that setup: it would
call the real `claude` CLI. `tests/test_run_gate_script.py` and
`tests/test_runner_supervisor.py` do the same with fakes, automatically.

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
| `home` | `~/vulcanbench-runner` | `runner.db`, `supervisor.lock`, `jobs/<id>/console.log`, and the output root (`runs/`) for harness runs the GUI launches |
| `gate_out` | `~/vulcanbench-gate` | `GATE_OUT`; `<gate_out>/jobs` is scanned |
| `run_worktrees` | `../VulcanBench-runs` | where `scripts/run_worktree.sh` puts bench-tag worktrees |
| `extra_run_roots` | `[]` | more harness output roots to show read-only |
| `first_check_after_s` | `180` | window for the first-attempt check |

## Milestones

1. Read-only Runs page and first-attempt check.
2. Queue and supervisor (one run at a time, survives a backend restart, clean
   stop), gate launches with the usage pre-flight and a confirm step, and the
   `run_gate.sh` hardening that makes a clean stop possible (this document).
3. Benchmark a model: suite runs through `vulcanbench run`, one job per
   effort level, the quota-free doctor and estimate, per-level progress, the
   capped quota continuation, `--only-missing` as an explicit resume, and
   `--max-infra-retries` on the CLI (this document).
4. Orphaned-sandbox cleanup, bench-tag cutting through the script, and the
   streaming log tail (this document).
