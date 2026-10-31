# Reproducibility (v1)

Every run is designed to be inspectable and replayable without hidden state.

## Artifacts

Each run directory under `./runs/<run_id>/` contains:

| File | Purpose |
|------|---------|
| `trace.jsonl` | Append-only event log (LLM turns, tool calls, diffs, metrics) |
| `tool-calls.jsonl` | Every tool call in one schema, whatever the harness (see below) |
| `summary.json` | Final scores, manifest, cost, duration, task hash |
| `final.patch` | Git diff of agent edits (redacted + size-capped) |
| `replay.html` | Self-contained browser replay |

## Manifest

`summary.json` includes a `manifest` recording:

- Python/platform versions
- Sandbox mode, image name, network setting
- Model and judge model
- Toolchain versions (git, ruff, bandit, go, node, etc.)
- Task metadata snapshot (scale, complexity, languages)

## Source provenance

`summary.json` (and each suite's `suite.json`) also records a `source` block:
the commit, `git describe` (a bench tag when run from one) and dirty state of
the checkout the harness was imported from, plus the working directory's
checkout when it differs. Suite runs refuse to start with uncommitted changes
under `harness/`, `tasks/`, `sandbox/` or the run config unless
`--allow-dirty` is passed, and then the run records it. See
[HOW_WE_WORK.md](HOW_WE_WORK.md).

## Tool calls

Every run logs its tool calls. Each harness records tool use in its own shape
(the native loop's trace, a CLI's event stream, a session log the CLI leaves
behind), so when a run finishes, `harness/agent/tool_calls.py` reads whichever
one the run's harness wrote and writes `tool-calls.jsonl`: one line per call,
in session order.

| Field | Meaning |
|-------|---------|
| `seq` | 1-based position in the session |
| `call_id` | The harness's id for the call |
| `tool` | The harness's own tool name (`Bash`, `read_file`; for Codex, the item type) |
| `kind` | `shell`, `edit`, `read`, `search`, `web`, `mcp`, `plan`, `agent` or `other` |
| `agent` | Null for the main agent, else the sub-agent that made the call |
| `input` | The call's arguments as recorded (redacted, never published) |
| `status` | `ok`, `error`, `rejected` or `unknown` |
| `exit_code` | Process exit code, where the harness reports one |
| `output_chars` | Length of the tool's output, where recorded |
| `started_at`, `ended_at`, `duration_ms` | Timing, where recorded |

`summary.json` carries a `tool_calls` block: the schema id, call count, errors,
counts by tool and by kind, and how many calls have start and end times. Its
`capture` is `recorded`, or `unavailable` with a reason when the harness's
record holds no tool calls (Mirror runs before 2026-10-27 kept no tool names).
Harnesses differ in what they expose: Claude Code reports no exit codes, and
Devin's times are whole seconds.

Every adapter has an extractor; `tests/test_tool_calls.py` fails for a harness
without one. For runs made before the log existed, build it from the same
sources:

```bash
vulcanbench tool-calls runs-effort-<model>            # report only
vulcanbench tool-calls runs-effort-<model> --write    # add tool-calls.jsonl
```

It adds `tool-calls.jsonl` and nothing else. `summary.json` is left alone,
because judging rounds pin its hash.

Public traces publish tool calls through `public_events()`: tool name, kind,
status, exit code, output length and timing for each call. Inputs and outputs
are never published, and a tool name that is not a plain identifier is
published as its kind.

## Task content hashing

`task_hash` in the summary is a deterministic SHA-256 of scoring-relevant task
files (repo, tests, issue, metadata tests, gold patch). The leaderboard and
report flag runs scored against a stale task definition.

## Replay

```bash
vulcanbench replay <run_id>
```

Opens or regenerates the HTML trace viewer. The summary's `replay_command` field
records the exact command.

## Sandbox isolation

Default `--sandbox docker` runs command execution and the functional verifier
inside a non-root, network-off container. File reads/edits use a host bind mount
over the same workspace path.

## Publishing artifacts

Traces, summaries, and patches pass through secret redaction before write
(`harness/redaction.py`). Safe to share for evaluation; do not use outputs to
train competing models (see README provider terms).
