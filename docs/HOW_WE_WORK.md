# How we work: building suites and running benchmarks

This is the working agreement for VulcanBench. It covers how we build eval
suites, how we run benchmarks against them, and how results get into the repo,
without the two kinds of work stepping on each other.

If you only read one section, read [The short version](#the-short-version).

## Why this exists

Two very different kinds of work happen in this repo:

- **Building** suites and the harness: tasks, verifiers, scoring, judging
  protocols, sandbox images. This is code, and it changes all the time.
- **Running** benchmarks: sweeps that take hours to days (one task at a time,
  3-hour task timeouts; see [DECISIONS.md](DECISIONS.md)), then judging
  rounds, cards and writeups.

When both happen in the same checkout, things drift. A sweep that starts on
Monday is still running on Thursday, and `vulcanbench` is an editable install,
so every `git pull` or local edit changes the code the next task runs with.
Half a column can end up scored by one version of the harness and half by
another, and nothing records which. Meanwhile results PRs carry harness edits,
raw traces bloat the clone (`traces/` alone is about 550 MB), and nobody can
say for sure which commit produced a published number.

The fix is to keep the two kinds of work on different clocks: building moves
forward on `main`, and running happens from a pinned snapshot of `main`.

## The short version

1. **Build on branches, merge to `main` through PRs.** Engine changes
   (harness, tasks, sandbox, scoring) and published results go in separate
   PRs. CI enforces this.
2. **Published suites are frozen.** Each one has a `suite.lock.json` with the
   hash of every task. CI fails if a frozen task changes. To change one on
   purpose, bump the suite's version and re-freeze.
3. **Run from a tag, in its own worktree.** Before a sweep, cut a bench tag
   from clean `main` (`make bench-tag LABEL=...`), then
   `make run-worktree TAG=...`. That gives you a separate checkout with its own
   venv that nobody edits. Keep working in your normal checkout.
4. **Every run records its commit.** Run summaries carry a `source` block
   (commit, tag, dirty flag). Suite runs refuse to start from uncommitted
   changes to scoring-relevant paths.
5. **Raw output stays out of git.** Run dirs live in gitignored `runs-*/`
   folders and get archived with `scripts/archive_runs.sh`. Git gets
   summaries, cards and writeups, each tied to a run and a commit.

## The layout

```
~/dev/VulcanBench/                      main checkout: where you build
  harness/ tasks/ sandbox/ ...          engine (changes through PRs)
  docs/results/                         published results (small files only)
  runs-effort-<model>/                  raw run output (gitignored)

~/dev/VulcanBench-runs/                 run worktrees: where sweeps run
  bench-2026-10-05-grok47-safety/       detached at tag bench/2026-10-05-grok47-safety
    .venv/                              its own venv, imports ITS harness/

$VULCANBENCH_ARCHIVE/                   bucket or disk: canonical raw output
  grok47-cursor-frontier-v4/            one folder per archived run dir
```

Three places, three jobs. The main checkout is for editing. Run worktrees are
for running and are never edited. The archive is the long-term home for raw
output.

## Building eval suites

### Branches and PRs

Work on a short-lived branch and open a PR to `main`. Suggested names:

| Kind of work | Branch | Touches |
| --- | --- | --- |
| New or changed suite tasks | `suite/<suite>-<topic>` | `tasks/`, maybe `harness/` |
| Harness, scoring, judging protocol | `harness/<topic>` | `harness/`, `sandbox/`, config |
| Published results, cards, writeups | `results/<model>-<suite>` | `docs/results/`, card scripts |

Names are a convention. What CI actually checks is the content (see
[PR checks](#pr-checks)).

### What counts as "engine"

Anything that can change what a run is asked to do or how it is scored:

- `harness/`, `tasks/`, `sandbox/`, `backend/`, `alembic/`
- `vulcanbench.toml`, `pyproject.toml`, `uv.lock`, `alembic.ini`

"Results" means `docs/results/` and `traces/`. Everything else (scripts,
tests, other docs, the dashboard, CI config) is neutral and can ride along
with either.

### Freezing a suite

Once a suite is published, its scoring-relevant content is locked:

```
make freeze-suite SUITE=coding-intelligence-index-v4
```

This writes `tasks/<suite>/suite.lock.json` with the suite's `version` (from
its `suite.json`) and the `task_hash` of every task in it. `task_hash` covers
the starting repo, the prompt (`issue.md`), hidden tests, the declarative tests
spec, the verifier and the gold patch. Cosmetic metadata (difficulty, notes,
budget calibration text) is deliberately outside it, so you can still fix a
typo in a note.

`make check-suites` (also part of `make ci`, and a CI step) fails if:

- a task in a frozen suite changed in a scoring-relevant way,
- a task was added or removed, or
- the version in `suite.json` no longer matches the lock.

`task_hash` skips local byproducts (`__pycache__`, `.pyc`, `.pytest_cache`,
`.mypy_cache`, `.ruff_cache`, `.DS_Store`), and the agent workspace copy skips
them too. Any other untracked file under a task's `repo/` or `tests/` still
counts, so if the check fails and you changed nothing, run
`git status --ignored tasks/<suite>` and look for leftovers. CI and fresh run
worktrees are always clean.

Frozen today: **VulcanBench Frontier v4** (`coding-intelligence-index-v4`,
version 2.0.0) and **VulcanBench Frontier v5** release 1 (`frontier-v5`,
version 1.0.0). Retired suites in this repo (cii-v1, v1 to v4, hard-1,
python-1, voice-v1, vulcancyber-v1) are not swept and are not locked. Lock one
if it ever comes back into use.

### Changing a frozen suite

Sometimes a published task really does need a fix. That is a breaking change:
results are comparable only within a version (see the Frontier v4
[CHARTER](../tasks/coding-intelligence-index-v4/CHARTER.md)). In one PR:

1. Make the fix.
2. Bump `version` in the suite's `suite.json` (semver: a task fix, addition or
   removal is a bump; follow the charter).
3. Run `make freeze-suite SUITE=<suite>` and commit the new lock.
4. Say in the PR which published columns this invalidates and whether they
   get re-run.

The diff then shows the version bump and the lock change side by side, which
is the point: nobody changes a published task by accident.

### Building a brand new suite

Develop it under `tasks/<new-suite>/` with no lock while it is in flux.
Validate tasks the usual way (`scripts/validate_tasks.py`, the Task PR
workflow). Freeze it the day it is first published, in the PR that declares
it published.

Harbor-native tasks (`task.toml` plus `instruction.md`, as in Frontier v5) are
hashed differently. The hash covers the prompt, `task.toml` without its
`[metadata]` table, and the `environment/`, `tests/` and `solution/` trees.
The lock also records each task's version and determinism checklist. Status
notes in `[metadata]` and `DESIGN.md` stay editable.

A suite can name private tasks in its `suite.json` (`private_tasks`: repo and
commit). Those directories live only in the private companion repo
(decision (f), [DECISIONS.md](DECISIONS.md) 2026-10-31). The lock records their
hash. Freezing needs the private checkout: pass `--private-root` or set
`VULCANBENCH_PRIVATE_TASKS` to its `tasks/` directory. `check` verifies a
private task only where that checkout is present, and CI reports it as not
checked. A `suite.json` `pins` object (reference models, CLI versions, image
ids) is copied into the lock and cannot change without a version bump.

## Running benchmarks

### The lifecycle

```
build on a branch ─▶ PR ─▶ main ─▶ bench tag ─▶ run worktree ─▶ sweep
                                                                   │
            results PR ◀─ cards + writeup ◀─ archive raw output ◀──┘
```

Step by step:

**1. Get the engine change merged.** Whatever the sweep or judging round
needs (a new harness adapter, a judging runner like
`harness/maintenance_review_v3NN.py`, a pinned CLI version) lands on `main`
first, through its own PR.

**2. Cut a bench tag.**

```
git checkout main && git pull
make bench-tag LABEL=grok47-safety PUSH=1
# -> bench/2026-10-05-grok47-safety
```

The script refuses unless you are on `main`, the tree is clean and `main`
matches `origin/main`. Suite locks are already enforced by CI on `main`, and
are checked again in the fresh run worktree. Bench tags are
`bench/<date>-<label>`, so they never trigger the `v*` release workflow.

**3. Create the run worktree.**

```
make run-worktree TAG=bench/2026-10-05-grok47-safety
# -> ../VulcanBench-runs/bench-2026-10-05-grok47-safety
```

This is a `git worktree`: a second checkout that shares the repo's history
but has its own files, detached at the tag. The script pulls LFS objects,
builds the worktree's own `.venv` (an editable install of *its* `harness/`,
not your main checkout's) and checks the suite locks.

**4. Launch the sweep from the worktree**, writing output to an absolute path
outside it. The main checkout's `runs-*/` folders are gitignored and are where
the card and population scripts already look, so that is the natural target:

```
cd ../VulcanBench-runs/bench-2026-10-05-grok47-safety
OUTROOT=$HOME/dev/VulcanBench/runs-effort-grok47 \
  MODEL=... PROBE=... \
  nohup bash scripts/cii-v4-board/run_effort_sweep.sh > $HOME/dev/VulcanBench/logs/grok47.log 2>&1 &
```

`run_effort_sweep.sh` `cd`s to the checkout it lives in, so running the
worktree's copy runs the worktree's code. Chained launchers that hardcode the
checkout (`VB=/Users/.../VulcanBench` in `run_opus55_all_suites.sh` and
`run_grok47_all_suites.sh`) need that variable pointed at the worktree. Now go back to your main checkout
and keep building. Nothing you do there touches the sweep.

**5. Archive the raw output** when the sweep finishes:

```
export VULCANBENCH_ARCHIVE=gs://<bucket>/vulcanbench   # or s3://..., or a directory
bash scripts/archive_runs.sh runs-effort-grok47 grok47-frontier-v4
```

It syncs the folder to the archive and drops an `ARCHIVED.json` pointer into
the local copy. Re-running is safe (it is a sync).

**6. Open the results PR**: cards, `docs/results/...`, the writeup, any
`docs/DECISIONS.md` entry. Name the bench tag in the PR description. No engine
files in this PR.

**7. Clean up**: `git worktree remove ../VulcanBench-runs/<name>` once nothing
needs it. The tag stays forever, so the exact code is always recoverable.

### Judging rounds work the same way

A Code quality round is a run too. The runner module and its protocol doc go
in an engine PR. Cut a bench tag, judge from a run worktree, then open a
results PR with the cards. Today these often land as one PR (runner, cards and
results together); under this workflow they are two. The per-round
`maintenance_review_v3NN.py` modules stay as they are: they are thin
configurations over the shared v3 core, and the tag is what pins the core
they ran against.

### Provenance: what every run records

Each run's `summary.json`, and each suite's `suite.json`, now carries a
`source` block:

```json
"source": {
  "harness": {
    "root": "/Users/.../VulcanBench-runs/bench-2026-10-05-grok47-safety",
    "commit": "b44558e3...",
    "describe": "bench/2026-10-05-grok47-safety",
    "dirty": false,
    "dirty_paths": []
  },
  "tasks": null,
  "dirty": false,
  "allow_dirty": false
}
```

- `harness` is the checkout the imported `harness` package came from.
- `tasks` is the checkout of the working directory, recorded only when it is a
  different checkout (for example a sibling suite repo, or a worktree driven
  by another checkout's venv). Seeing a non-null `tasks` on a VulcanBench
  suite run usually means the wrong venv was used.
- `describe` is `git describe --tags`, so a run from a bench tag names the
  tag, and a dirty one ends in `-dirty`.
- `dirty_paths` lists uncommitted changes under the scoring-relevant paths
  (`harness/`, `tasks/`, `sandbox/`, `vulcanbench.toml`, `pyproject.toml`,
  `uv.lock`), capped at 50.

A suite run also records `source.suite`: the suite id, display name, `kind`
(`in-tree` or `external`), the suites home and directory it was loaded from,
and, for a frozen suite, the lock version, `frozen_at` and `lock_digest`. The
digest is a sha256 over the lock's version and task hashes, so two summaries
with the same digest were scored against the same task set.

Provenance is read once per process, when the run starts. This sits alongside
the existing `task_hash` and `manifest` fields
([REPRODUCIBILITY.md](REPRODUCIBILITY.md)).

### The dirty-checkout guard

`vulcanbench run --suite ...` and `vulcanbench effort-sweep ...` refuse to
start when the checkout has uncommitted changes under the scoring-relevant
paths. Single-task runs (`--task`) only warn, so quick local experiments stay
easy.

For a deliberate experiment, pass `--allow-dirty` or set
`VULCANBENCH_ALLOW_DIRTY=1`. The run still records `dirty: true` and
`allow_dirty: true`, and a dirty run is never a publishable number.

A checkout that is not a git repo (an installed package) has nothing to stamp
and is not refused.

### Suites in other repos

Routine v1 and v2 live in the VulcanRoutine repo, and VulcanBench Safety v1
lives in VulcanConduct. The same rules apply: tag the suite repo, run from a
clean checkout of it, and the run's `source.tasks` block records that repo's
commit next to VulcanBench's `source.harness`. Locks and the PR hygiene
workflow are VulcanBench-only for now; port them when those repos need them.

### Suites outside the checkout

A suite built from your own repository lives in a suites home, never in this
repo, so a slice of a private codebase stays private. The default home is
`~/vulcanbench-suites` (`[runner].suites_home` in `vulcanbench.toml`,
`VULCANBENCH_RUNNER_SUITES_HOME`, or `--suites-home` on any command). Each
suite is one directory, `<suites_home>/<suite-id>/`, holding `suite.json`,
`suite.lock.json` once frozen, and one directory per task in the Frontier v4
task format. A task's `build/` folder holds the builder's working files,
answer key included; it is outside `task_hash` and never reaches the agent's
workspace.

```
vulcanbench suite init acme-slice --display-name "Acme slice" --repo acme/api
vulcanbench suite validate acme-slice --task <task>
vulcanbench suite add-task acme-slice --task <task>
vulcanbench suite freeze acme-slice
vulcanbench run --suite acme-slice --suites-home ~/vulcanbench-suites --model ...
```

Every `vulcanbench suite` command takes `--json` and then prints exactly one
JSON document. `add-task` only admits a task whose last validation was PASS
or WARN. A suite id that names a suite in `tasks/` is refused, and when a name
exists in both places the in-tree suite wins.

The dirty-checkout guard cannot see a directory outside the checkout, so an
external suite is held to its lock instead. `vulcanbench run` refuses an
external suite that has no `suite.lock.json`, or whose tasks no longer match
it, before any model call. No flag bypasses that check. To change a frozen
external suite, fix the task, bump `version` in its `suite.json`, and run
`vulcanbench suite freeze` again.

### Building a suite from your own repository

The suite builder turns merged pull requests from one of your own GitHub
repositories into tasks. It is for subscribers, so every command that creates
or changes a suite (`init`, `prs list`, `task scaffold`, `task author`,
`add-task`, `remove-task`, `validate`, `freeze`) needs the builder unlocked on
this machine. Reading suites (`list`, `show`, `check`, `access`) and running a
frozen one stay open.

```
vulcanbench suite unlock                       # asks for the subscriber password once
vulcanbench suite init acme-api --display-name "Acme API" --repo acme/api
vulcanbench suite prs list --suite acme-api --since 2026-08-01
vulcanbench suite task scaffold acme-api --pr 412
vulcanbench suite task author acme-api --task pr-412-fix-etag --harness codex --model gpt-6.1-terra
vulcanbench suite validate acme-api --task pr-412-fix-etag --sandbox docker
vulcanbench suite add-task acme-api --task pr-412-fix-etag
vulcanbench suite freeze acme-api
```

1. **`prs list`** asks `gh` (your own login) for merged pull requests and
   scores each from 0 to 100: it changes tests, its source change is a
   task-sized number of lines in a few files, it is not a dependency bump, and
   it has a description or a linked issue. Flags name what makes one a poor
   task. No model call.
2. **`task scaffold`** clones the repository into the suites home's cache,
   finds the pull request's base commit (the first parent of a merge commit,
   the commit before a rebase, the parent of a squash) and exports `repo/` at
   that commit with `git archive`, so there is no `.git` and no history that
   holds the fix. The pull request's own test files are removed from `repo/`.
   `gold_patch.diff` is the non-test part of the change. `issue.md` is a
   marked DRAFT. Everything the builder works from (the full diff, the
   upstream tests after the fix, the linked issues, a self-check script) goes
   in the task's `build/` folder, which is outside `task_hash` and never
   reaches an agent's workspace. A private repository has no open source
   license, so pass `--license-note` to record the terms in `repo/NOTICE`.
   No model call.
3. **`task author`** is the one step that spends quota. One session in the
   chosen CLI harness, on this machine, in a scratch copy of the task with
   network off, rewrites the issue so it describes the problem and not the
   fix, writes hidden `vb_` tests with the canary, fills the `tests` block,
   and checks its work with `build/selfcheck.py`. Afterwards only `issue.md`,
   `tests/`, `gold_patch.diff`, `build/author_notes.md`, deletions of test
   files under `repo/` and a short list of `metadata.json` keys are copied
   back; provenance keys and every other change are discarded and listed in
   `build/author.json`. It never re-runs and never validates itself.
4. **`validate`** runs the task validator (gold solves, base fails, three
   deterministic rounds) and two builder checks: the fix is not already in
   `repo/` and the gold patch applies. Review the issue, the tests and the
   verdict, then `add-task` the ones that pass and `freeze` the suite.

The password is a placeholder for subscriber accounts. `[builder].password_hash`
in `vulcanbench.toml` holds its scrypt hash (made with
`vulcanbench suite password-hash`), the password is read from a prompt or one
line on stdin and is never stored, logged or passed as an argument, and
`vulcanbench suite unlock` records a fingerprint of the hash under
`~/.config/vulcanbench`. Changing the password locks every machine again. The
code is public and runs locally, so this keeps the builder behind a
deliberate step but does not stop someone who edits it.

### Watching runs from the GUI

`make gui` starts a localhost page (`http://127.0.0.1:3000/runs`) that lists
every gate job and harness run under the configured roots with its bench tag,
attempts, rewards, containers and a live log tail, plus a first-attempt check
that flags sessions that never used the requested model or made no tool calls.
Its Launch page queues a model sweep (one `vulcanbench run` per effort
level, `--max-infra-retries 0`) through the `run_worktree.sh` path described
above: the worktree must be at the bench tag and clean, the exact commands
and the estimate are shown before a confirm click, the queue runs one job at
a time, nothing retries on its own (a quota window may continue a level only
when the plan says so, docs/DECISIONS.md 2026-10-30), and a stop goes
through the harness's own shutdown. Frontier v5 gate runs are launched by
hand with `run_gate.sh` and show up on the Runs page read-only
(docs/DECISIONS.md 2026-11-01). Cutting a bench tag from the GUI goes
through `cut_bench_tag.sh` with the same refusals. See [GUI.md](GUI.md).

## Results: what goes in git and what does not

| Goes in git | Stays out of git |
| --- | --- |
| Cards (PNG, SVG) and their CSV/JSON sources | Run directories (`runs/`, `runs-*/`) |
| Summary tables, comparison JSON, writeups | Agent session logs, raw traces |
| Protocol docs, `docs/DECISIONS.md` entries | Workspaces, sandboxes, caches |
| A README pointing at an archive or release | Anything over 5 MiB outside `tasks/` |

Every published result should name the bench tag (or commit) and the run
folder or archive path it came from.

**Publishing raw traces publicly.** When we want traces public (as with the
Muse Spark 1.3 traces), attach a redacted bundle to a GitHub Release (assets
can be up to 2 GB each, are versioned, and do not bloat the clone) and commit
a short README under `traces/<name>/` pointing at it.

**Public traces include tool calls.** Every redacted trace carries one
`tool_call` event per call, built from the run's `tool-calls.jsonl` with
`public_events()` in `harness/agent/tool_calls.py`: tool name, kind, status,
exit code, output length and timing. Inputs and outputs stay withheld. For a
sweep made before the log existed, run `vulcanbench tool-calls <sweep dir>
--write` first ([REPRODUCIBILITY.md](REPRODUCIBILITY.md#tool-calls)).

**The existing `traces/` folder** (about 550 MB, mostly the Muse Spark 1.3
Frontier v4 traces) stays where it is so existing links keep working. Removing
it from the tree would not shrink history anyway; only a history rewrite
would, and that is a separate decision. The rule is simply that nothing new
lands there except READMEs.

## PR checks

| Check | Where | Fails when |
| --- | --- | --- |
| Frozen suites match their locks | `ci.yml`, `make ci` | A locked suite's task hashes, task set or version drifted |
| Engine vs results scope | `pr-hygiene.yml` | A PR touches both engine and results paths |
| No new raw traces | `pr-hygiene.yml` | A PR adds non-Markdown files under `traces/` |
| No large blobs | `pr-hygiene.yml` | A PR adds or modifies a file over 5 MiB outside `tasks/` |

The scope rule has one escape hatch: the `mixed-scope` PR label, for a
genuine exception (for example a one-line harness fix that has to ship with a
correction to a published table). Say why in the PR description. The trace
and size rules have no label; if one of them is wrong for a real case, change
[scripts/check_pr_scope.py](../scripts/check_pr_scope.py) in its own PR.

## Cheat sheet

```
make check-suites                              # are frozen suites intact?
make freeze-suite SUITE=<dir>                  # lock a suite (after a version bump)
make bench-tag LABEL=<label> [PUSH=1]          # tag clean main for a sweep
make run-worktree TAG=bench/<date>-<label>     # pinned checkout + own venv
git worktree list                              # what run worktrees exist
bash scripts/archive_runs.sh <run-dir> [name]  # copy raw output to the archive
vulcanbench run --suite ... --allow-dirty      # experiments only, never published
```

## Rolling this out

- **Sweeps already in flight** in the main checkout keep running, but once
  this change is pulled there, each `--only-missing` retry checks the tree.
  If you have uncommitted harness or task edits in that checkout, the retry
  refuses (the launcher keeps looping and logging the refusal). Either commit
  or stash those edits, or finish the in-flight sweep with
  `VULCANBENCH_ALLOW_DIRTY=1` exported in the launcher's shell. Start every
  new sweep from a run worktree.
- **Pick an archive location** and set `VULCANBENCH_ARCHIVE` in your shell
  profile. A GCS or S3 bucket is best; an external disk works too.
- **Older results.** Results recorded before this change have no `source`
  block. Their `task_hash` fields still show which task definitions they ran
  against, and `vulcanbench leaderboard` flags runs whose hash no longer
  matches the current tasks.
