# Frontier v5 grid engine

How a grid run (PLAN-v6 sections 4 and 5) goes through Harbor: one budgeted
agent session per environment, four environments at a time in fixed batches,
per-item scoring, and the board. Draft, built 2026-11-21 before the pilot
cell exists; the pilot fixes the numbers.

| Piece | File |
| --- | --- |
| Grid file check and batch plan | `scripts/frontier-v5/grid_plan.py` |
| Budgeted, batched runs | `scripts/frontier-v5/run_gate.sh --grid` |
| Per-item result reader | `scripts/frontier-v5/grid_items.py` |
| Harbor job importer | `scripts/frontier-v5/import_harbor.py` |
| Board aggregation | `scripts/frontier-v5/grid_board.py` |

## What Harbor 0.24.0 does at an agent timeout

Read from the installed Harbor 0.24.0 source (`harbor/trial/single_step.py`,
`trial.py`, `environments/docker/docker.py`), and confirmed on 2026-11-21
on the CLI11 pilot with Harbor's oracle agent: points 2, 3 and 5 below
were seen directly (a 3-second budget on an oracle that sleeps first;
the exec'd process surviving a killed `docker exec` client).

1. The agent phase runs under `asyncio.wait_for(timeout=agent timeout)`. The
   agent timeout is the task's `[agent] timeout_sec`, or the job's
   `override_timeout_sec`, times `--agent-timeout-multiplier` (or
   `--timeout-multiplier`). It covers the agent's run phase only, not its
   setup.
2. At the timeout Harbor raises `AgentTimeoutError`, catches it in
   `SingleStepTrial._run_agent` and records it: `result.json`
   `exception_info` = `{"exception_type": "AgentTimeoutError",
   "exception_message": "Agent execution timed out after N seconds", ...}`,
   plus `exception.txt`. `agent_execution.finished_at` is set.
3. The trial then continues as if the agent had finished: it downloads the
   agent logs and calls the agent's `populate_context_post_run` (so tokens
   are read), runs the task's collect hooks, collects the artifacts, stops
   the agent container (separate verifier mode) and **runs the verifier**.
   `verifier_result` and `reward.json` are written as usual.
4. `AgentTimeoutError` is in Harbor's default no-retry list, so a budget stop
   is never retried.
5. **Gap:** cancelling the agent kills the host's `docker compose exec`
   client (`_terminate_process`), not the agent's process inside the
   container (Docker does not forward the client's death to an exec'd
   process). The agent keeps running, and writing, until the container stops,
   which in separate mode is after the artifacts are collected. The tree
   could be collected mid-write, and the agent gets a few unbudgeted
   seconds.
6. Tokens after a budget stop: Codex totals come from the rollout's last
   `token_count` event and survive the stop. Claude Code writes its
   cumulative `modelUsage` only in the final `result` event, which a stopped
   session never writes, so `import_harbor.py` sums the per-message usage in
   the stream instead (each message id once) and says so in `cost.basis`.

## The freeze hook (closes the gap)

Every grid environment declares this in its `task.toml`. Harbor runs
`[[verifier.collect]]` hooks for the main service after the agent phase
(timed out or not) and before the artifacts are collected:

```toml
[[verifier.collect]]
command = "kill -KILL -1 2>/dev/null; true"
user = "root"
```

`kill -KILL -1` as root signals every process in the container except PID 1
and the calling shell, so the agent and anything it started stop before the
tree is read. It also runs after a normal finish, where it is harmless.
`grid_plan.py` refuses a grid whose environments lack it. A verifier in
shared mode must not depend on a process started before the agent phase,
since the hook stops it too; separate mode (CLI11's) is unaffected.

## The grid file

One file per release, next to the environments (default
`tasks/frontier-v5/grid.json`, or `GATE_GRID_FILE`), pinned with the suite
lock when the release is frozen:

```json
{"schema": 1,
 "budget_min": 20,
 "budget_instruction": "You have {minutes} minutes of wall-clock time. When it is up the session is stopped and the repository is graded as it stands.",
 "environments": {"cli11-grid": {"language": "cpp", "tier": "smaller"}},
 "batches": [["cli11-grid"]]}
```

- `language`: c, cpp, java, javascript (TypeScript repos too), python, rust.
  `tier`: smaller or larger. These place each environment on the board.
- `batches`: every environment in exactly one batch of 1 to 4. The order
  and grouping are the same for every model, so contention is equal.
- `budget_min`: the budget. Applied as `--agent-timeout-multiplier
  budget / [agent] timeout_sec`, so every environment in a batch must have
  the same `[agent] timeout_sec`.
- `budget_instruction`: appended to every environment's instruction with
  `--extra-instruction`, with `{minutes}` filled in; null when the
  environment's own `instruction.md` states the budget. One of the two must
  tell the agent; the pilot chooses which.

## Running

```bash
GATE_EFFORT=max scripts/frontier-v5/run_gate.sh --grid sol62
```

```bash
GATE_EFFORT=max GATE_BUDGET_MIN=30 scripts/frontier-v5/run_gate.sh --grid sol62 1
```

The first runs every batch in order; the second is a calibration run of
batch 1 at a 30-minute budget (the status file records `budget_source:
override`). Each batch is one Harbor job, `<ref>-<effort>-grid-b<n>`, with
one attempt per environment and all of them concurrent. Authentication, the
web-tools-off settings, the Claude usage pre-flight, signals and the
infrastructure guard are the gate runner's; an infrastructure failure in
any environment stops the whole batch, which is then rerun whole.

The status file (`$GATE_OUT/<job>.status.json`) adds `mode: grid`, `batch`,
`environments`, `budget_sec`, `budget_source`, `agent_timeout_multiplier`,
`items_done` and `items_total` (summed from each `reward.json`) and
`budget_reached` (trials Harbor stopped at the budget).

## Scoring and the board

```bash
scripts/frontier-v5/import_harbor.py --out runs-grid ~/vulcanbench-gate/jobs/sol62-max-grid-b*
```

```bash
scripts/frontier-v5/grid_board.py --grid tasks/frontier-v5/grid.json --out board runs-grid
```

`grid_items.py` checks each trial's `items.json` against the grid brief's
schema and against `reward.json` (reward = items done / items total;
`items_done` and `items_total` match; a failed tamper scan scores 0 items;
every `reward.json` value is a finite number, since Harbor fails the trial
with `VerifierOutputParseError` on anything else, and after a budget stop
that error is hidden behind `AgentTimeoutError` in `result.json`).
A trial that disagrees is not scored and the import exits 1. How a broken
guard is charged is the verifier's to apply (by clearing `done`); the
engine reports the failed guards beside the count.

The board gives, per model and effort: items done over items with `n=` and
±1 binomial stderr; the same per size tier, per language and per item kind;
the six-by-two cells as counts; tokens and API-equivalent cost. No speed
column (R13). Unscored runs are excluded, never zeros, and mark the row
partial. Two scored runs of one model on one environment are refused.
