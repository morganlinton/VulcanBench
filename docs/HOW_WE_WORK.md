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
version 2.0.0). Retired suites in this repo (cii-v1, v1 to v4, hard-1,
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
