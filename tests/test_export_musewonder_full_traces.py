"""Muse Wonder Full redacted-traces exporter: redaction, observer attribution and the canary."""

from __future__ import annotations

import csv
import json
from pathlib import Path

from scripts import export_musewonder_full_redacted_traces as exporter

RUN_ID = "legacy-democore-binary-parity-0123abcd"
TASK = "legacy-democore-binary-parity"
SECRETS = (
    "/Users/morganlinton/dev/VulcanBench",
    "morganlinton",
    "def parse_ledger(",
    "SECRET-PROMPT",
    "SECRET-PROSE",
    "SECRET-REASON",
    "SECRET-ADVISORY",
    "SECRET-OUTPUT",
    "/private/var/folders/xy/T/scratch",
)


def _record(seq: int, run_id: str, event: dict, kind: str = "run") -> dict:
    return {
        "schema_version": 1,
        "id": f"rec-{run_id}-{seq}",
        "sequence": seq,
        "record_type": "event",
        "payload_type": "runtime.session",
        "payload": {
            "kind": kind,
            "run_id": run_id,
            "event": event,
            "source_run_record_id": f"src-{seq}",
        },
    }


def _model(model: str, input_tokens: int, output_tokens: int) -> dict:
    return {
        "kind": "model_completed",
        "model": model,
        "duration_ms": 1200,
        "finish_reason": "tool_calls",
        "usage": {
            "input_tokens": input_tokens,
            "output_tokens": output_tokens,
            "cached_tokens": input_tokens // 2,
            "reasoning_tokens": output_tokens // 2,
        },
    }


def _write_log(path: Path, rows: list[dict]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("".join(json.dumps(r) + "\n" for r in rows))


def _synthetic_run(tmp_path: Path) -> tuple[Path, dict]:
    run = tmp_path / "high" / RUN_ID
    session = run / "muse-session-logs/2026/10/16/sess-main"
    observer_id = "child-goal"
    bash_args = json.dumps(
        {"command": "cat /Users/morganlinton/dev/VulcanBench/x.py", "description": "SECRET-PROMPT"}
    )
    bash_out = json.dumps(
        {"exit_code": 1, "terminal_status": "failed", "output": "def parse_ledger(): SECRET-OUTPUT"}
    )
    main = [
        _record(1, "run-main", {"kind": "run_model", "record": {"model_id": exporter.WONDER}}),
        _record(2, "run-main", _model(exporter.WONDER, 1000, 50)),
        _record(
            3,
            "run-main",
            {
                "kind": "assistant_tool_calls_committed",
                "tool_calls": [{"call_id": "call_1", "name": "bash", "args": bash_args}],
            },
        ),
        _record(
            4,
            "run-main",
            {
                "kind": "tool_result_batch_committed",
                "results": [{"tool_call_id": "call_1", "text": bash_out}],
            },
        ),
        _record(
            5,
            "run-main",
            {
                "kind": "assistant_message_committed",
                "text": "SECRET-PROSE ```def parse_ledger(): pass``` /Users/morganlinton",
            },
        ),
        _record(
            6,
            "run-main",
            {
                "kind": "memory_reminder_child_session_linked",
                "reminder_agent_id": "goal-reminder",
                "child_session_id": observer_id,
                "child_session_log_path": f"subagent/{observer_id}/session.jsonl",
            },
        ),
        _record(
            7,
            "run-main",
            {
                "kind": "reminder_proposal",
                "reminder_agent_id": "goal-reminder",
                "reminder_kind": "goal",
                "advisory_text": "SECRET-ADVISORY",
                "decision_reason": "SECRET-REASON",
                "decision_call_id": "call_obs",
            },
        ),
        _record(
            8,
            "run-main",
            {"kind": "reminder_reconciler_outcome", "outcome": "rejected", "reason": "no_reminder"},
        ),
        _record(
            9,
            "run-main",
            {"kind": "terminal", "terminal": "completed", "reason": None, "turn_duration_ms": 9000},
        ),
    ]
    # A second Wonder call arrives wrapped in a retained frame, as Muse Code writes some records.
    main.insert(
        5,
        {
            "retained_frame": "session_permission_transaction",
            "children": [
                {
                    "record_json": json.dumps(
                        _record(10, "run-main", _model(exporter.WONDER, 400, 20))
                    )
                }
            ],
        },
    )
    _write_log(session / "session.jsonl", main)
    observer_args = json.dumps({"decision": "none", "reason": "SECRET-REASON"})
    _write_log(
        session / "subagent" / observer_id / "session.jsonl",
        [
            _record(1, "run-obs", {"kind": "run_model", "record": {"model_id": exporter.SPARK}}),
            _record(2, "run-obs", _model(exporter.SPARK, 300, 30)),
            _record(
                3,
                "run-obs",
                {
                    "kind": "assistant_tool_calls_committed",
                    "tool_calls": [
                        {
                            "call_id": "call_obs",
                            "name": "submit_reminder_decision",
                            "args": observer_args,
                        }
                    ],
                },
            ),
            _record(
                4,
                "run-obs",
                {
                    "kind": "tool_result_batch_committed",
                    "results": [{"tool_call_id": "call_obs", "text": "reminder decision recorded"}],
                },
            ),
            _record(5, "run-obs", {"kind": "terminal", "terminal": "completed", "reason": None}),
        ],
    )
    usage = {
        "input_tokens": 1400,
        "output_tokens": 70,
        "cached_tokens": 700,
        "reasoning_tokens": 35,
        "calls": 2,
        "auxiliary": {
            exporter.SPARK: {
                "input_tokens": 300,
                "output_tokens": 30,
                "cached_tokens": 150,
                "reasoning_tokens": 15,
                "calls": 1,
            }
        },
    }
    (run / "trace.jsonl").write_text(json.dumps({"type": "muse_usage", "data": usage}) + "\n")
    summary = {
        "task_id": TASK,
        "run_id": RUN_ID,
        "model": exporter.MODEL,
        "duration_s": 600.0,
        "total_tokens": 1470,
        "scores": {"functional": 1.0},
        "cost_detail": {
            "agent_input_tokens": 1400,
            "agent_cached_input_tokens": 700,
            "agent_uncached_input_tokens": 700,
        },
        "cli_agent": {"harness_version": "Muse Code 1.4.9 (1.4.9-R6297.1)"},
    }
    (run / "summary.json").write_text(json.dumps(summary))
    return run, summary


def _export(tmp_path: Path) -> tuple[Path, list[dict]]:
    run, summary = _synthetic_run(tmp_path)
    events, stats = exporter.redact_run(run, summary)
    out = tmp_path / "out"
    exporter.write_trace(
        out / "traces/high" / f"{TASK}.jsonl", exporter.header(summary, "high", RUN_ID), events
    )
    row = {
        "effort": "high",
        "task": TASK,
        "run_id": RUN_ID,
        "judged": True,
        "functional": 1.0,
        "combined_33": 90.0,
        "code_quality": 80.0,
        "timed_out": False,
        "minutes": 10.0,
        **stats,
        "file": f"traces/high/{TASK}.jsonl",
        "canary": exporter.CANARY,
    }
    with (out / "manifest.csv").open("w", newline="") as fh:
        writer = csv.DictWriter(fh, fieldnames=list(row), lineterminator="\n")
        writer.writeheader()
        writer.writerow(row)
    rows = [{**row, "effort": e} for e in exporter.EFFORTS]
    (out / "README.md").write_text(
        exporter.readme(
            rows,
            [
                {
                    **summary,
                    "started_at": "2026-10-16T00:00:00+00:00",
                    "finished_at": "2026-10-17T00:00:00+00:00",
                }
            ],
        )
    )
    return out, events


def test_no_prose_paths_or_code_leak(tmp_path: Path) -> None:
    out, _ = _export(tmp_path)
    text = (out / "traces/high" / f"{TASK}.jsonl").read_text()
    for secret in SECRETS:
        assert secret not in text
    assert exporter.self_check(out) == []


def test_header_carries_canary(tmp_path: Path) -> None:
    out, _ = _export(tmp_path)
    head = json.loads((out / "traces/high" / f"{TASK}.jsonl").read_text().splitlines()[0])
    assert head["canary"] == exporter.CANARY
    assert exporter.CANARY_GUID in head["canary"]
    assert head["suite"] == "VulcanBench Frontier v4 (coding-intelligence-index-v4)"
    assert head["harness_version"].startswith("Muse Code 1.4.9")
    assert (out / "README.md").read_text().startswith(f"<!-- {exporter.CANARY} -->")


def test_spark_observer_calls_are_attributed_to_spark(tmp_path: Path) -> None:
    _, events = _export(tmp_path)
    calls = [e for e in events if e["event"] == "model_call"]
    assert [(e.get("agent"), e["model"]) for e in calls] == [
        (None, exporter.WONDER),
        (None, exporter.WONDER),
        ("goal-reminder", exporter.SPARK),
    ]
    start = next(e for e in events if e["event"] == "subagent_start")
    assert start == {
        "event": "subagent_start",
        "agent": "goal-reminder",
        "role": "reminder_observer",
        "models": [exporter.SPARK],
    }
    decision = next(e for e in events if e["event"] == "reminder_decision")
    assert decision == {
        "event": "reminder_decision",
        "agent": "goal-reminder",
        "reminder_kind": "goal",
        "decision": "none",
    }
    result = events[-1]
    assert result["model_calls"] == {exporter.SPARK: 1, exporter.WONDER: 2}
    assert result["terminal"] == "completed" and result["timed_out"] is False
    # Wonder at $4.00 / $0.20 / $20.00 per million, Spark at $1.25 / $0.15 / $4.25.
    expected = (700 * 4.00 + 700 * 0.20 + 70 * 20.00 + 150 * 1.25 + 150 * 0.15 + 30 * 4.25) / 1e6
    assert abs(result["cost_usd_estimate"] - round(expected, 4)) < 1e-9


def test_tool_events_keep_name_error_and_size_only(tmp_path: Path) -> None:
    _, events = _export(tmp_path)
    call = next(e for e in events if e["event"] == "tool_call")
    assert call == {"event": "tool_call", "model": exporter.WONDER, "tool": "bash"}
    result = next(e for e in events if e["event"] == "tool_result")
    assert set(result) == {"event", "tool", "is_error", "output_chars"}
    assert result["is_error"] is True and result["output_chars"] > 0
    message = next(e for e in events if e["event"] == "assistant_message")
    assert set(message) == {"event", "model", "chars"}


def test_self_check_catches_a_leak(tmp_path: Path) -> None:
    out, _ = _export(tmp_path)
    trace = out / "traces/high" / f"{TASK}.jsonl"
    leaked = {"event": "tool_call", "tool": "bash", "note": "cat /Users/someone/src/main.py; echo"}
    trace.write_text(trace.read_text() + json.dumps(leaked) + "\n")
    problems = exporter.self_check(out)
    assert any("path or username" in p for p in problems)
    assert any("free text" in p for p in problems)
