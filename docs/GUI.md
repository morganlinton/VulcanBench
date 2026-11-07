# The local run GUI

A localhost page for watching and launching benchmark runs, so the operator
stops driving them by hand. It lives in the existing backend
(`backend/runner/`) and dashboard (`dashboard/app/runs`,
`dashboard/app/launch`), wraps the existing CLI and scripts as subprocesses,
and reads what they write to disk. It never reimplements run logic.

## First run

1. Start it: `make app` and open the app, or `make gui` and go to
   `http://127.0.0.1:3000`.
2. Open Launch. The readiness card at the top checks, without starting
   anything: the queue supervisor, a prepared bench tag, Docker, the `gh`
   login and the suite builder. It stays collapsed to one green line when all
   pass; anything amber or red says how to fix it. Each harness's own login is
   checked by the harness doctor when a plan is built.
3. Benchmark a model: on Launch, type the model id, pick the harness, the
   suite (VulcanBench Frontier v4 or a custom suite), the bench tag and the
   effort levels, then Build plan. Read the exact commands, the list-price
   estimate and the warnings, then confirm. Nothing is queued before the
   confirm click, and the plan expires after 10 minutes.
4. Watch it on Runs: each sweep is a card with one row per effort level
   (progress, the task running now against the 3-hour cap, cost, ETA, Stop).
   A level that ends short says why (quota window, expired login, exit code)
   and the card offers "Run the missing tasks", which opens Launch prefilled
   with `--only-missing`.
5. Build your own suite: on Suites, unlock with the subscriber password,
   create a suite for `owner/name`, load its merged pull requests, scaffold
   the good ones (fill in the terms note first if the repository has no
   LICENSE), author them (one model session each, behind a plan and a confirm
   click), validate them in Docker (no model calls), read each task's issue,
   tests and gold patch, accept the ones that passed, and freeze. "Benchmark
   this suite" opens Launch with the suite picked.

Every step that spends model quota goes through a plan you read and a
confirm click; nothing retries on its own.

### The readiness check

`GET /api/runner/readiness` runs five local, quota-free checks:

| check | ok when | otherwise |
|---|---|---|
| Queue supervisor | it is active with no hold | red: restart the backend with `VULCANBENCH_RUNNER=1` |
| Bench tag | some `bench/*` tag has a run worktree at its commit, with clean scoring paths | amber when none is prepared, red when the prepared one is dirty or off its tag |
| Docker | the daemon answers | amber: local-sandbox runs still work; validation does not |
| GitHub CLI | `gh auth status --hostname github.com` exits 0 | amber: run `gh auth login` |
| Suite builder | a tooling worktree exists and a password is configured (locked counts as ok) | amber, with the step that is missing |

`gh auth status` runs with only `HOME`, `PATH`, `USER` and `TMPDIR` in its
environment, stdin closed, and its output discarded unread, because it can
name the account and the token's scopes. Nothing in the check reads a
credential file.

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

### Custom suites

The suite picker has two groups. **Built-in at this tag** lists the frozen
suites under `tasks/` at the chosen bench tag. **Custom (suites home)** lists
every frozen suite in `[runner].suites_home` (default `~/vulcanbench-suites`),
read from its `suite.lock.json` and `suite.json`; a draft suite with no lock
is not offered, and a custom id that names a built-in suite at the tag is
left out because the engine resolves in-tree names first. Picking one shows
its directory, its repository, when it was frozen and its lock digest.

A custom suite's plan adds `--suites-home <dir>` after `--suite <id>` in every
level's command and in the estimate, and records the suites home and the lock
digest on the plan. It needs a bench tag whose harness knows the flag; an
older tag is refused with "pick a newer bench tag". At confirm and again at
launch the GUI checks that the flag is there for a custom suite (and only
for one), that it names the configured suites home, and that the lock's
digest still equals the one the plan was built with; a suite re-frozen in
between needs a new plan. The run itself then verifies the tasks against the
lock before any model call (docs/HOW_WE_WORK.md, "Suites outside the
checkout").

The **Pinned CLI directory** field (under "advanced") prepends a directory to
the child's PATH, for a model that needs a newer or pinned harness CLI
(`CODEX_BIN_DIR` in the scripts). Login homes the harness reads
(`CODEX_HOME`, `CLAUDE_CONFIG_DIR`, `GROK_HOME`, `ZCODE_*`) pass through
when set; keys never do. Metered API keys reach the harness the way they do
by hand: `load_dotenv()` on the worktree's `.env`.

## Build your own eval suite (milestone 6)

The Suites pages (`/suites`, "Build your own suite" on the Launch page,
View > Suites in the Mac app) turn merged pull requests from one of the
operator's own GitHub repositories into a frozen suite that runs on the
Launch page under Custom. The flow: create a suite for `owner/name`, load its
merged pull requests (scored as task candidates), scaffold the ones worth
keeping, have a model write each task's issue and hidden tests, validate,
read and accept the tasks that pass, freeze.

Every step that changes a suite is a `vulcanbench suite` command run from the
*tooling worktree*: the newest prepared bench tag whose worktree has
`harness/suite_builder/` and passes the same checks as a benchmark launch (at
the tag, clean scoring paths, `.venv`). Without one the pages say to cut a tag
and prepare it on the Launch page. Reads (the suite list, a suite's tasks, a
task's files) come from disk as JSON and text.

| Page action | Command | Notes |
|---|---|---|
| Unlock, Lock | `suite unlock --password-stdin`, `suite lock` | see below |
| New suite | `suite init <id> --display-name N --repo owner/name` | 409 for an existing id or a built-in suite's id |
| Load pull requests | `suite prs list --suite <id> [--since] --limit` | through the operator's `gh` login; cached 10 minutes; 409 with "gh auth login" when gh is not logged in |
| Scaffold N selected | `suite task scaffold <id> --pr N [--license-note TEXT]` | one request per PR, one scaffold per suite at a time, up to 10 minutes each; no model call; the terms note (below) adds `--license-note` |
| Accept | `suite add-task <id> --task T` | only with a PASS or WARN validation newer than the last authoring |
| Drop | `suite remove-task` when accepted, then `build/dropped.json` | the task's folder stays |
| Freeze suite | `suite freeze <id>` | needs one accepted task, every accepted task validated, no active job |

Every command gets `--suites-home <[runner].suites_home>` and `--json`, and
its stdin is a closed pipe so `gh` or `git` can never wait on a prompt. Exit
7 (the builder is locked) is a 403 that brings back the unlock screen. A
frozen suite is read-only here. A task's stage is derived from its `build/`
files and the runner's jobs: scaffolded, authoring, authored, authoring
failed, validating, validated, validation failed; a validation older than the
latest authoring session no longer counts.

A repository with no LICENSE file, which is common for a private one, needs
a terms note before scaffolding. Validation requires a LICENSE or NOTICE file
in each task's `repo/` for provenance, so without one every task from that
repository fails validation. The optional "Terms note" field above the
Scaffold button is written to `repo/NOTICE` in each task scaffolded while it
is filled in. When a scaffold reports `no_license`, the pull request's row
says so right away; add a NOTICE file to that task's `repo/` folder by hand
(scaffolding the same pull request again is refused, because the dropped
task's folder stays).

### Authoring and validation jobs

Select tasks in the task table (or open one task) and pick **Author** or
**Validate**. Both build a plan first, through `POST /api/runner/plans`
like a benchmark sweep: the exact command per task, the harness doctor (for
authoring), an estimate, the warnings, and a confirm button that queues one
job per task. The supervisor runs them one at a time with everything else in
the queue; nothing starts without the confirm click and nothing retries.

- **Author** (`kind: author`) runs `vulcanbench suite task author <id> --task
  T --harness H --model M --billing B [--effort E] --timeout 3600 --max-turns
  200`. The harness picker offers the CLI harnesses only (not the `vulcan`
  API loop), billing `auto` or `subscription`, and never a blocked effort.
  The session runs on this Mac as the operator, in a scratch copy of the task
  with network off, and the engine copies back only the issue, hidden tests,
  gold patch and allowed metadata keys. It spends model quota once per task;
  cost is open-ended (no price table covers authoring), so the plan says to
  budget about one Frontier v4 task run per pull request. A doctor that says
  the harness is not ready disables confirm.
- **Validate** (`kind: validate`) runs `vulcanbench suite validate <id> --task
  T --sandbox docker`; it needs a successful authoring session and a running
  Docker, and makes no model call.

How a job ends:

| Kind | Exit | Job status | Note |
|---|---|---|---|
| author | 0 with a new `author.json` status ok | finished | accepted and discarded change counts |
| author | 2, 3, 5 | infra_failed | refused before the session, with the engine's message |
| author | 6 | failed | the session made no accepted change |
| author | 1 | failed | provider error; a quota window or rate limit, or an expired login, is named; nothing retries |
| validate | 0 or 1 | finished | `verdict: PASS` or `FAIL` with reasons (the verdict belongs to the task, not the job) |
| validate | 3 | infra_failed | Docker is not running |
| validate | 4 | failed | a toolchain is missing (SKIP) |
| both | 7 | infra_failed | the builder was locked between confirm and launch |
| both | 130, 143, a signal | stopped | an authoring session's agent CLI is killed like a suite run's |

At confirm and again right before launch the job is re-checked: the tooling
worktree is still at its tag and clean and has the builder, the argv is the
worktree's `vulcanbench suite task author` or `suite validate` for the
planned suite and task with the configured `--suites-home`, the suite is not
frozen, the effort is not blocked, there is no `--allow-dirty`, and the env
has no overrides. A job's console log streams on its task page while it runs
and stays on the Runs page as `job:<id>`.

### The unlock screen and the password

The builder is for subscribers and is password-gated in the engine
(docs/DECISIONS.md, 2026-11-03), so the pages show an unlock screen until
`suite access --json` says this Mac is unlocked. The password's path:

1. The field is an uncontrolled `type="password"` input; the submit handler
   reads it once and clears it, so the value never sits in React state or in
   browser storage.
2. The browser posts `{"password": ...}` to `POST /api/runner/suites/unlock`
   on the loopback backend. The runner routes refuse pages not served from
   localhost, so another website open in the browser cannot post there.
3. The route reads the body itself (FastAPI's 422 response echoes rejected
   input), refuses an empty password, one over 1024 characters or one with a
   line break without quoting it, and runs `vulcanbench suite unlock
   --password-stdin --json` with the password plus a newline on the child's
   stdin. It is never an argument (so never in `ps`), never in the child's
   environment, never in a plan, job, console log or sqlite row.
4. Anything the command prints is masked for the password before it reaches
   the browser, then redacted like every other log line.
5. Five wrong passwords in ten minutes make the next try wait a minute. The
   429 answer carries `retry_after_s`; the screen counts it down with Unlock
   disabled, so a password typed during the wait stays in the field instead
   of being sent and thrown away.

The builder's irreversible steps (accept, drop, freeze, lock) ask in the page
itself rather than through `window.confirm` or `window.prompt`: the desktop
app is Electron, which has no `window.prompt`, so a native prompt would make
Drop cancel itself there.

Uvicorn's access log records only the method, path and status. The password
does cross from the browser to the backend in the request body, which the
screen says, and Python cannot wipe a string, so it sits briefly in the
backend's memory. The engine keeps only a fingerprint of the configured hash
in `~/.config/vulcanbench/suite-builder-unlock.json`.

The hash is read from `vulcanbench.toml` in the tooling worktree (the child
environment drops `VULCANBENCH_*`), so a newly set password takes effect in
the GUI once it is committed and a bench tag containing it is prepared.

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

## The queue (milestone 2)

Every plan, whether a suite sweep or a worktree preparation, ends in the
queue shown at the top of the Runs page. The queue runs one job at a time.
The supervisor thread in the backend (one per machine, guarded by
`<runner home>/supervisor.lock`) starts the oldest queued job when no owned
job is active and no `harbor run` or `vulcanbench run` launched outside the
GUI is visible, re-validating the plan right before the child starts.
Preparing a worktree is its own queue step: it runs
`scripts/run_worktree.sh <tag>` (status **preparing**) and finishes when the
worktree exists. The worktree's `make setup` output lands in
`<worktree>/.venv-setup.log`; when it fails, the job's console log shows the
tail and the job ends as **failed**. A half-made worktree has to be removed
by hand (`git worktree remove <dir>`) before the same tag can be planned
again.

**Stop** on a queue row or a run page persists a stop request. The supervisor
sends SIGINT to the child. For a suite level it then kills the agent sessions
after two seconds (they run in their own process groups, so the harness's
watchdog would do the same), gives the verifier and teardown ten minutes, and
only then sends SIGTERM and SIGKILL to the job's process group and removes
the containers and scratch of that level's unfinished runs. A plain job gets
SIGTERM after 150 s and SIGKILL 15 s later. **Remove** takes a queued job out
of the queue before it starts. There is no retry button and no automatic
retry: a failed, stopped or lost job stays that way until the operator plans
a new run.

The queue survives a backend restart. Jobs are recorded in
`<runner home>/runner.db` (stdlib sqlite, outside every checkout) with the
child's pid, start time, boot time and an argv token; on start the supervisor
adopts jobs whose process is still the same one, and gives the others their
final state from what they left on disk (a suite level from its runs, a plain
job from nothing, so it is **lost**).

Frontier v5 gate runs are not planned from the GUI (they were, between
2026-10-29 and 2026-10-31; docs/DECISIONS.md has the entry). They are
launched by hand with `scripts/frontier-v5/run_gate.sh`, and the Runs page
keeps showing them read-only: a running one holds the queue like any outside
run, and its containers are attributed to it by the orphan check.

### Rules the GUI enforces (not just displays)

- Every launch runs from a worktree under `[runner].run_worktrees` created by
  `scripts/run_worktree.sh`, detached at the plan's `bench/...` tag, with no
  uncommitted changes under `harness`, `tasks`, `sandbox`, `vulcanbench.toml`,
  `pyproject.toml` or `uv.lock`; never the backend's own checkout and never
  with `--allow-dirty` (the argv is checked for it at confirm and launch).
- The child environment is an allowlist: `HOME`, `PATH`, `USER`, `LANG`,
  `TMPDIR` and the login homes of the subscription CLIs (`CODEX_HOME`,
  `CLAUDE_CONFIG_DIR` and the like) pass through; nothing else does.
  `VULCANBENCH_*` (so `VULCANBENCH_ALLOW_DIRTY` cannot leak in),
  `DATABASE_URL`, `*_API_KEY`, `*_TOKEN` and `*SECRET*` never pass, and a
  plan may not set them.
- Effort levels under `[effort].blocked` never appear as options and are
  refused server-side, at plan time and again at launch.
- A suite run never re-queues a unit on its own (`--max-infra-retries 0`),
  and the GUI never retries a failed, incomplete, stopped or lost job. The
  only continuation is the quota rule on the plan, off by default.
- Nothing that spends model quota starts without the confirm click that
  shows the estimate and the exact command; there is no auto-start and no
  auto-retry. The harness doctor, the cost estimate and the worktree checks
  that run before the confirm make no model call.
- Credentials are never read, displayed, logged or sent to the browser. The
  backend opens no credential file; the harness doctor reports whether a CLI
  is logged in, and the harness reads its own login.
- Loopback only; no auth system, no remote exposure. The runner routes exist
  only with `VULCANBENCH_RUNNER=1` and answer 403 to any other client address.
  They also answer 403 to a browser page that is not served from localhost
  (any `Origin` header whose host is not `localhost`, `127.0.0.1` or `::1`,
  and `Origin: null`). Every page open in the browser connects from
  127.0.0.1, and the public API's CORS policy allows any origin, so the
  client address alone would let any website read the runner and post to
  it. Requests with no `Origin` (curl, server-side fetches) still pass.

### Reading gate jobs

`scripts/frontier-v5/run_gate.sh` keeps every stdout line the GATE-LOG
workflow reads and, since 2026-10-29, runs harbor as a direct child with
SIGINT at its default disposition, traps INT and TERM to stop the run cleanly
(exit 130), and writes an atomic `$GATE_OUT/<job>.status.json` with the phase
(`preflight`, `running`, `stopping`, `done`, `infra_failed`, `stopped`), pids,
solves, harbor's exit code and the stop reason. The Runs page reads that
status file to decide a gate job's state, falling back to the process table
and the job's `result.json` for jobs from older tags. `--refs` and
`GATE_PREFLIGHT_ONLY=1` remain in the script for terminal use.

## Dry run without spending quota

The whole launch path runs against the fake `vulcanbench` CLI in
`tests/runner_fixtures.py`: it answers `harness doctor`, `estimate --json` and
`run` with scripted outcomes (finish, partial, quota window, auth failure,
crash, hang), so `tests/test_runner_routes.py` and
`tests/test_runner_supervisor.py` exercise plan, confirm, queue, stop,
continuation and restart adoption without a model call. For a dry run by
hand, prepare a worktree for a local `bench/...` tag, replace its
`.venv/bin/vulcanbench` with that fake, point `VULCANBENCH_RUNNER_HOME` and
`VULCANBENCH_RUNNER_RUN_WORKTREES` at scratch directories, and plan a sweep
from the Launch page.

## Starting it

```bash
make gui            # backend on 127.0.0.1:8000 with VULCANBENCH_RUNNER=1, dashboard on 127.0.0.1:3000
```

or separately, `make gui-backend` and `make gui-dashboard` (the dashboard
needs `npm install` in `dashboard/` once). The runner routes answer only when
`VULCANBENCH_RUNNER=1` is set and only to loopback clients; without the flag
the backend behaves exactly as the public deployment does.

## The Mac app

`desktop/` wraps the same two servers in an Electron window, so the GUI opens
from the Dock instead of two terminals.

```bash
make app                                        # needs node 22 (brew install node@22)
open desktop/dist/mac-arm64/VulcanBench.app
```

What the app does at launch, in order:

1. Finds the checkout: `VULCANBENCH_REPO` if set, else the path recorded in
   `build-info.json` when the app was built (the checkout `make app` ran in),
   else the repo above `desktop/` when run unpackaged with `npm start`. It
   refuses with a dialog if that checkout has no `.venv` or
   `scripts/gui_backend.sh`.
2. Backend: if something already answers on `127.0.0.1:8000/api/health`
   (`make gui`, or a copy of the app still shutting down) it is used as is.
   Otherwise the app runs `scripts/gui_backend.sh 8000` from the checkout, with
   the user's environment plus the directories a Finder launch lacks on PATH
   (`~/.local/bin`, `~/.docker/bin`, `/opt/homebrew/bin`, `/usr/local/bin`).
   The app reads no credential file and sets no secret; the backend and its
   subprocesses get exactly what a terminal launch would.
3. Dashboard: Next's standalone build, assembled into the app by
   `desktop/scripts/build-dashboard.sh`, run with the app's own node
   (`ELECTRON_RUN_AS_NODE`) on port 3000 or the next free port. The backend
   address is baked into the dashboard at build time (`NEXT_PUBLIC_API_BASE`,
   default `http://localhost:8000`), which is why the backend port is fixed.
4. Opens the Runs page once both answer. Menu: Runs, Launch, Leaderboard and
   Suites under View, Open in Browser, Show Logs in Finder.

Quitting stops the dashboard and a backend the app itself started, with
SIGTERM. That never signals a benchmark: the supervisor leaves running jobs
alone and adopts them again on the next start, as with any backend restart.
Logs land in `~/Library/Logs/VulcanBench/`. The `.app` is unsigned and built
per machine; it is a launcher for one checkout, not an installer, because the
Python venv, Docker, the run worktrees and the runner state all stay where
they are. `cd desktop && npm test` covers the helpers (PATH, checkout lookup,
port choice, server start and stop) without Electron.

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
| `suites_home` | `~/vulcanbench-suites` | custom suites built outside the checkout, offered in the suite picker and built on the Suites pages |

## Milestones

1. Read-only Runs page and first-attempt check.
2. Queue and supervisor (one run at a time, survives a backend restart, clean
   stop), and the `run_gate.sh` hardening that made a clean stop possible
   (this document). Gate launches from the GUI shipped with this milestone
   and were removed on 2026-11-01 (docs/DECISIONS.md); gate jobs stay
   visible read-only.
3. Benchmark a model: suite runs through `vulcanbench run`, one job per
   effort level, the quota-free doctor and estimate, per-level progress, the
   capped quota continuation, `--only-missing` as an explicit resume, and
   `--max-infra-retries` on the CLI (this document).
4. Orphaned-sandbox cleanup, bench-tag cutting through the script, and the
   streaming log tail (this document).
5. Custom suites in the picker: frozen suites from the suites home run with
   `--suites-home`, checked by lock digest at confirm and launch (this
   document).
6. Build your own eval suite: the Suites pages behind the builder's unlock
   screen (create, pull requests, scaffold, review, accept, drop, freeze),
   wrapping `vulcanbench suite` from the tooling worktree, and queued
   authoring and validation jobs behind the confirm click (this document).
