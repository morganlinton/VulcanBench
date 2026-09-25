# Muse Spark 1.3 (Contributor) on VulcanBench Frontier v4: agent traces

Raw agent traces from a five-level effort sweep of Meta's Muse Spark 1.3
model, driven by the Muse Code CLI, on the 23-task VulcanBench Frontier v4
suite (internally `coding-intelligence-index-v4`). The tasks, gold patches
and tests live in the public
[VulcanBench](https://github.com/morganlinton/VulcanBench) repository under
`tasks/coding-intelligence-index-v4/`.

These are the solver's own records, published so others can analyze how the
model spent its time. Nothing has been edited apart from the harness's
standard redaction pass, gzip compression of the two largest file types, and
removal of the per-workspace `.git` folders the harness creates for diffing.

## Configuration

| Item | Value |
| --- | --- |
| Model | `muse-spark-1.3-contributor` (Meta Contributor tier) |
| CLI | Muse Code 1.0.3 (1.0.3-R2198.1), macOS arm64, pinned by SHA256 |
| Effort | passed as `--reasoning-effort`; `extra-high` maps to `xhigh` |
| Billing | Meta account subscription; no API key in the child environment |
| Web tools | disabled |
| Isolation | macOS `sandbox-exec` outer profile; per-task workspace and scratch |
| Task timeout | 10 hours for minimal and the first 17 low tasks, 3 hours after (see below) |
| Concurrency | 1 (sequential) |
| Dates | 2026-09-06 to 2026-09-18 |

The full protocol, including task hashes and all seven amendments made during
the sweep, is in `protocol.json`. The protocol as originally written is
preserved in `protocol-original-2026-09-06.json`.

Levels not run: `max` is documented by Meta as Standard-tier only, and
`ultra` is blocked for every model on VulcanBench because it is a client-side
mode mapped onto the provider's highest tier rather than a distinct effort.

## Results

`results.jsonl` has one line per scored run (115 in all). Functional score is
the hidden-test pass fraction; a regression against a previously passing test
scores 0.

| Level | Mean functional | Full passes | Zeros | Tokens | Solve hours | Provider-aborted attempts |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| minimal | 0.717 | 10 | 2 | 1,215,371,999 | 51.3 | 9 |
| low | 0.666 | 10 | 6 | 588,674,265 | 26.9 | 1 |
| medium | 0.705 | 12 | 5 | 684,490,854 | 26.4 | 0 |
| high | 0.757 | 13 | 4 | 739,638,148 | 20.6 | 0 |
| extra-high | 0.769 | 14 | 3 | 444,006,388 | 13.8 | 1 |

Total 3,672,181,654 tokens and 139.1 hours of solving. Token counts are the
sum of Muse's own per-call receipts (input plus output, cached input included
in input), deduplicated by run and record id.

### Timeout change mid-sweep

The suite's task timeout was lowered from 10 hours to 3 hours on 2026-09-13,
while this sweep was paused on a quota window. Minimal and the first 17 low
tasks ran under 10 hours; everything after ran under 3. Six minimal runs and
one low run lasted past 3 hours. Rescoring those as zero gives minimal 0.620
and low 0.628; the other levels are unaffected.

## Layout

```
<level>/<task>-<run-id>/
  summary.json                 harness summary: scores, usage, effort, task hash (scored runs only)
  trace.jsonl                  harness event log: start/result events, verifier output
  cli-agent-stream.jsonl.gz    Muse Code's JSON event stream as received by the harness
  final.patch                  the diff the harness graded (scored runs only)
  replay.html                  harness replay page for the run
  workspace/                   the task workspace as the model left it
  muse-session-logs/
    <date>/<session-id>/session.jsonl.gz   Muse's own session log, with per-call token receipts
    stderr.txt                 Muse's stderr
    boundary.sb                the sandbox profile applied to the run
```

`<level>` is one of `minimal`, `low`, `medium`, `high`, `extra-high`.

Eleven directories have no `summary.json`. They are attempts the provider
aborted mid-run (a model stream idle timeout or transport timeout), which
produced no patch and were not scored. The task was then relaunched from a
clean workspace. Timeouts and failing patches were never rerun. The aborted
attempts are kept for completeness: at minimal, lodgecore (6), blendcore (2)
and stampcore (1); at low, stampcore (1); at extra-high, schedcore (1).

## Reading the traces

Decompress with `gunzip -k` or read with `gzip.open` in Python. Every file is
JSON Lines.

In `cli-agent-stream.jsonl`, each shell command and its result appear as a
`tool.result` record whose `payload.text` is itself a JSON string with
`command`, `description`, `exit_code` and `output`. The `description` field
is the model's own one-line label for the step, which makes it a quick way to
follow what it was doing. Model turns are `task.lifecycle.*` records with
`task_kind` of `model.meta.response`; file edits are `tool.edit_file`.

In `session.jsonl`, completed model calls are records whose
`payload.event.kind` is `model_completed`, with `usage` holding
`input_tokens`, `cached_tokens`, `output_tokens` and `reasoning_tokens`, a
`duration_ms`, and `recorded_at` in microseconds since the epoch. Some records
are wrapped in a `retained_frame` envelope whose `children[].record_json`
holds nested records; `harness/agent/muse_code.py` in the VulcanBench
repository has a reference parser (`session_records` and `collect_usage`).

## Caveats

- **Contributor tier.** Meta's Contributor pricing tier permits Meta to use
  submitted prompts and completions for training. That was accepted for this
  run. The task suite was already public.
- **Effort is asserted, not echoed.** The CLI accepted each level and the
  provider receipts confirm the model id, but the provider does not report the
  effective reasoning effort.
- **Protocol amendments.** Seven, all listed in `protocol.json`: a longer
  stream idle timeout (180 s to 900 s) after repeated provider stalls, the
  3-hour task timeout above, two driver fixes so standard harness outcomes
  (budget exceeded, no source file changed) are recorded as scored failures
  instead of pausing, two source re-pins after unrelated harness edits, and
  dropping `ultra`. None changed a scored result.
- **Timing.** Wall clock includes tool execution on the host, which was shared
  with other work at times. Treat durations as indicative.
- **Paths.** Logs contain local filesystem paths from the machine that ran the
  sweep. They carry no secrets; the harness redaction pass ran over every
  record and a separate credential scan found none.
