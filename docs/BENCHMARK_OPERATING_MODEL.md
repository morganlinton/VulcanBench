# Authoring and benchmark execution

Use one GitHub source repository with separate local authoring and execution
checkouts. Every benchmark identifies an exact committed revision. Authoring can
advance while that experiment continues on its original revision.

## Directory ownership

| Area | Purpose | Writer |
| --- | --- | --- |
| Authoring checkout or worktree | Harness, task definitions, tests and reviewed reports | One authoring chat per checkout |
| Dedicated runner clone | Committed source and an independent Python environment | Runner preparation only |
| External results directory | Experiment manifests, console logs, traces, patches and scores | One unique directory per experiment |

On the current Mac, the authoring checkout is
`/Users/vulcanbench/dev/VulcanBench`, the execution clone is
`/Users/vulcanbench/dev/VulcanBench-runner`, and results go to
`/Users/vulcanbench/vulcanbench-results`. Configuration lives in
`~/.config/vulcanbench/runner.json`. Linked authoring worktrees are useful for
parallel development. The runner is a separate clone without shared Git objects,
indexes, branches or an authoring editable install. Its origin push URL is disabled.
This prevents ordinary accidental pushes; it is not an access-control boundary.

Do not switch branches, pull, install dependencies, edit task definitions or
publish reports from an active execution checkout. Give each new authoring chat
its own worktree if another chat owns the current directory. Commit and push
only that chat's changes. Review changes through a PR before integrating them.

## Standard launcher

On the current Mac, `~/.local/bin/vulcanbench-runner` is the installed shortcut.
It uses the runner Python environment and a versioned copy of this launcher
outside both checkouts. Thus preparing an older commit cannot remove the command.
The installation receipt records its source commit and SHA-256; updating the
installed launcher is an explicit step after reviewing launcher changes.
The examples below use the equivalent source-script entry point.

Run `scripts/benchmark_runner.py` from a maintained authoring checkout using
Python 3.12 or later. It requires Git LFS and uv for preparation. It does not
provision language toolchains or choose new experiment conditions.

Initialize once on a new machine:

```sh
python scripts/benchmark_runner.py init \
  --source /absolute/path/VulcanBench \
  --runner /absolute/path/VulcanBench-runner \
  --results /absolute/path/vulcanbench-results
```

Commit and push the candidate definition, then prepare its full commit SHA:

```sh
python scripts/benchmark_runner.py prepare --revision <full-40-character-commit>
```

Preparation refuses local runner changes, checks out a detached commit, retrieves
LFS objects and synchronizes `uv.lock` with `uv sync --locked`. It records the
Python version and installed package inventory. It never pulls a moving branch
during execution. A changed dependency inventory requires preparation again.
Existing benchmark processes using this runner prevent preparation.

Record a preflight without making a model call:

```sh
python scripts/benchmark_runner.py run \
  --revision <full-40-character-commit> \
  --suite coding-intelligence-index-v4 \
  --model claude-sonnet-5-5 --harness claude-code --billing subscription \
  --effort low --sandbox local --no-judges --check-only
```

Remove `--check-only` to execute that explicitly authorized experiment. For one
task, replace `--suite` with `--task TASK --tasks-root tasks/POOL`. Set `--repeat`
explicitly when needed. Concurrency is one. Task budgets remain their committed
harness defaults; this interface does not add budget overrides. Existing harness
policy, including blocked effort levels, still applies. `--check-only` inventories
inputs; it does not prove provider access, task admission, or toolchain readiness.

Each launch creates a unique directory containing `experiment.json`,
`console.log` and a `runs/` subtree. The manifest includes the exact Git commit
and tree, task hashes and metadata, Python package inventory, available native
tool versions/paths, operator-settings hash, launcher hash and complete command.
Secrets and the general environment are not dumped. Model/effort choices remain
requests, subject to the provider identity limits recorded by the harness.

A nonzero command exit or source drift marks the experiment failed while retaining
its evidence. A completed command can contain functional task failures; inspect
the run summaries before reporting capability. No automatic commit, push, reset,
cleanup, results promotion or launcher-level retry occurs.

## Concurrency and native evidence

All standard launcher operations use the same OS advisory lock at
`~/.local/state/vulcanbench/execution.lock`, regardless of config or results path.
The lock coordinates processes running as this account on this host. It remains
held throughout the benchmark child process. Interrupt and termination signals
are forwarded to that process group and observed descendants, including provider
CLIs in separate sessions. The final receipt retains the exit. If signaled
descendants remain alive, an execution.review.json marker blocks subsequent
operations until the operator resolves the processes and removes that marker.

Execution also checks for existing `vulcanbench run`, `effort-sweep` and
`python -m harness.cli run` processes. This catches the current legacy CLI runs.
Other programs and custom Python callers that bypass the launcher do not
participate in its lock and may not be detected. Route future runs through this
launcher. Preflight is allowed while a legacy run is active because it makes no
model or verifier calls. Do not interrupt or relocate an active legacy run.

Separate Git state does not provide OS read/network isolation or resource caps.
Native solver files in global temporary directories still require the applicable
protocol's attribution, quarantine and exposure review. The OCaml calibration
protocol's admission gates, CLI/toolchain pins, artifact cleanup and no-retry
conditions remain mandatory. This generic launcher does not replace that
protocol or make an arbitrary multi-attempt native calibration eligible.

Historical calibration launchers and frozen task definitions remain unchanged.
Adapt future protocol drivers prospectively to use pinned runner code and
external evidence. Never run a legacy audit that writes directly to task
folders inside the runner. Generate reports externally, then deliberately copy
reviewed reports or receipt references into an authoring branch and commit them.

## Results retention and migration

The external results directory is local storage, not a backup. Use durable
artifact storage or a separate backup destination for irreplaceable evidence.
Keep raw traces and solver artifacts out of ordinary source commits. A published
report should identify its experiment directory/archive, commit and task hashes.

Do not move or delete existing ignored `runs-*` directories during this change.
They contain historical absolute references and may be in use. Migrate them only
through a separate copy-and-hash-verification step, preserving an inventory and
all original evidence until the copy is verified and consumers are updated.

Recommended next step: after the existing native sweep finishes, start the next
authorized benchmark through the prepared runner and verify its external receipt
before retiring the legacy launch path.
