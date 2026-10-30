"""Tests for the normalized tool-call log (tool-calls.jsonl), one fixture per harness format."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

import pytest

from harness.agent import tool_calls as tc
from harness.agent.cli_agents import list_cli_agent_adapters
from harness.agent.loop import run_agent

T0 = "2026-10-29T10:00:00+00:00"
T1 = "2026-10-29T10:00:02+00:00"
T2 = "2026-10-29T10:00:05+00:00"
EPOCH_T0 = 1_793_268_000  # 2026-10-29T10:00:00Z


def _write(path: Path, records: list[dict[str, Any]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("\n".join(json.dumps(r) for r in records) + "\n", encoding="utf-8")


def _trace(run: Path, events: list[tuple[str, str, dict[str, Any]]]) -> None:
    _write(
        run / "trace.jsonl", [{"ts": ts, "type": kind, "data": data} for ts, kind, data in events]
    )


def test_every_adapter_has_an_extractor() -> None:
    """A harness added without a tool-call extractor fails here, not silently in a sweep."""
    ids = {adapter.harness_id for adapter in list_cli_agent_adapters()}
    assert ids, "no adapters registered"
    missing = sorted((ids | {"native"}) - set(tc.EXTRACTORS))
    assert missing == [], f"add an extractor to harness/agent/tool_calls.py for: {missing}"


@pytest.mark.parametrize(
    ("name", "kind"),
    [
        ("Bash", "shell"),
        ("command_execution", "shell"),
        ("run_terminal_cmd", "shell"),
        ("KillShell", "shell"),
        ("Edit", "edit"),
        ("MultiEdit", "edit"),
        ("apply_patch", "edit"),
        ("file_change", "edit"),
        ("write_file", "edit"),
        ("Read", "read"),
        ("read_file", "read"),
        ("Grep", "search"),
        ("Glob", "search"),
        ("LS", "search"),
        ("list_dir", "search"),
        ("WebFetch", "web"),
        ("web_search", "web"),
        ("webSearchToolCall", "web"),
        ("mcp:docs.lookup", "mcp"),
        ("TodoWrite", "plan"),
        ("write_todos", "plan"),
        ("Task", "agent"),
        ("truncate", "other"),
        ("think", "other"),
    ],
)
def test_tool_kind(name: str, kind: str) -> None:
    assert tc.tool_kind(name) == kind


def test_native_new_format_times_both_ends(tmp_path: Path) -> None:
    _trace(
        tmp_path,
        [
            (T0, "tool_call", {"tool": "run_command", "args": {"command": "pytest"}, "id": "c1"}),
            (
                T1,
                "tool_observation",
                {
                    "tool": "run_command",
                    "id": "c1",
                    "result": {"stdout": "x", "exit_code": 1},
                    "error": None,
                    "duration_ms": 2000,
                },
            ),
        ],
    )
    [call] = tc.extract(tmp_path, "native")
    assert (call["call_id"], call["tool"], call["kind"], call["status"], call["exit_code"]) == (
        "c1",
        "run_command",
        "shell",
        "error",
        1,
    )
    assert call["started_at"] == T0 and call["ended_at"] == T1 and call["duration_ms"] == 2000
    assert call["input"] == {"command": "pytest"}


def test_native_old_format_pairs_in_order_with_end_time_only(tmp_path: Path) -> None:
    _trace(
        tmp_path,
        [
            (T1, "tool_call", {"tool": "read_file", "args": {"path": "a"}, "id": "c1"}),
            (T1, "tool_observation", {"tool": "read_file", "result": "text", "error": None}),
            (T2, "tool_call", {"tool": "edit_file", "args": {}, "id": "c2"}),
            (
                T2,
                "tool_observation",
                {"tool": "edit_file", "result": None, "error": "no such file"},
            ),
        ],
    )
    calls = tc.extract(tmp_path, "native")
    assert [(c["tool"], c["status"], c["started_at"], c["ended_at"]) for c in calls] == [
        ("read_file", "ok", None, T1),
        ("edit_file", "error", None, T2),
    ]


def test_claude_code_pairs_tool_use_and_result(tmp_path: Path) -> None:
    _write(
        tmp_path / tc.STREAM,
        [
            {
                "type": "assistant",
                "timestamp": "2026-10-29T10:00:00.000Z",
                "parent_tool_use_id": None,
                "message": {
                    "content": [
                        {"type": "text", "text": "hi"},
                        {
                            "type": "tool_use",
                            "id": "toolu_1",
                            "name": "Bash",
                            "input": {"command": "ls"},
                        },
                    ]
                },
            },
            {
                "type": "user",
                "timestamp": "2026-10-29T10:00:02.000Z",
                "message": {
                    "content": [
                        {
                            "type": "tool_result",
                            "tool_use_id": "toolu_1",
                            "content": "abc",
                            "is_error": True,
                        }
                    ]
                },
            },
        ],
    )
    [call] = tc.extract(tmp_path, "claude-code")
    assert (call["tool"], call["status"], call["output_chars"], call["duration_ms"]) == (
        "Bash",
        "error",
        3,
        2000,
    )
    assert call["started_at"] == T0 and call["ended_at"] == T1 and call["agent"] is None


def _codex_stream(run: Path) -> None:
    _write(
        run / tc.STREAM,
        [
            {
                "type": "item.started",
                "item": {"id": "item_1", "type": "command_execution", "command": "ls"},
            },
            {
                "type": "item.completed",
                "item": {
                    "id": "item_1",
                    "type": "command_execution",
                    "command": "ls",
                    "aggregated_output": "a\nb",
                    "exit_code": 0,
                    "status": "completed",
                },
            },
            {
                "type": "item.completed",
                "item": {"id": "item_2", "type": "agent_message", "text": "done"},
            },
            {
                "type": "item.completed",
                "item": {
                    "id": "item_3",
                    "type": "file_change",
                    "status": "completed",
                    "changes": [{"path": "x.py", "kind": "update"}],
                },
            },
        ],
    )


def test_codex_with_started_events_and_ids(tmp_path: Path) -> None:
    _codex_stream(tmp_path)
    _trace(
        tmp_path,
        [
            (T0, "tool_call", {"tool": "command_execution", "id": "item_1"}),
            (T1, "tool_observation", {"tool": "command_execution", "id": "item_1"}),
            (T1, "llm_response", {"content": "done"}),
            (T2, "tool_observation", {"tool": "file_change", "id": "item_3"}),
        ],
    )
    calls = tc.extract(tmp_path, "codex")
    assert [
        (c["tool"], c["kind"], c["status"], c["exit_code"], c["output_chars"]) for c in calls
    ] == [("command_execution", "shell", "ok", 0, 3), ("file_change", "edit", "ok", None, None)]
    assert (
        calls[0]["duration_ms"] == 2000
        and calls[1]["started_at"] is None
        and calls[1]["ended_at"] == T2
    )


def test_codex_older_trace_aligns_by_order(tmp_path: Path) -> None:
    _codex_stream(tmp_path)
    _trace(
        tmp_path,
        [
            (T1, "tool_observation", {"tool": "command_execution"}),
            (T2, "tool_observation", {"tool": "file_change"}),
        ],
    )
    calls = tc.extract(tmp_path, "codex")
    assert [c["ended_at"] for c in calls] == [T1, T2] and all(
        c["started_at"] is None for c in calls
    )


def test_codex_older_trace_that_disagrees_leaves_times_empty(tmp_path: Path) -> None:
    _codex_stream(tmp_path)
    _trace(tmp_path, [(T1, "tool_observation", {"tool": "run_command"})])
    assert all(c["ended_at"] is None for c in tc.extract(tmp_path, "codex"))


def test_cursor_nested_tool_call(tmp_path: Path) -> None:
    _write(
        tmp_path / tc.STREAM,
        [
            {
                "type": "tool_call",
                "subtype": "started",
                "call_id": "c1",
                "tool_call": {"shellToolCall": {"args": {"command": "ls"}}},
            },
            {
                "type": "tool_call",
                "subtype": "completed",
                "call_id": "c1",
                "tool_call": {
                    "shellToolCall": {
                        "args": {"command": "ls"},
                        "result": {"success": {"exitCode": 2, "stdout": "no"}},
                    }
                },
            },
            {
                "type": "tool_call",
                "subtype": "completed",
                "call_id": "c2",
                "tool_call": {
                    "webSearchToolCall": {"result": {"rejected": {"reason": "User Rejected"}}}
                },
            },
        ],
    )
    _trace(tmp_path, [(T0, "tool_call", {"tool": "c1"}), (T1, "tool_observation", {"tool": "c1"})])
    calls = tc.extract(tmp_path, "cursor")
    assert [(c["tool"], c["kind"], c["status"], c["exit_code"]) for c in calls] == [
        ("shell", "shell", "error", 2),
        ("webSearch", "web", "rejected", None),
    ]
    assert calls[0]["duration_ms"] == 2000


def test_grok_build_session_updates(tmp_path: Path) -> None:
    _write(
        tmp_path / tc.STREAM,
        [
            {
                "method": "session/update",
                "params": {
                    "update": {
                        "sessionUpdate": "tool_call",
                        "toolCallId": "t1",
                        "title": "run_command",
                        "kind": "execute",
                        "status": "pending",
                        "rawInput": {"command": "ls"},
                    }
                },
            },
            {
                "method": "session/update",
                "params": {
                    "update": {
                        "sessionUpdate": "tool_call_update",
                        "toolCallId": "t1",
                        "status": "completed",
                        "rawOutput": "abc",
                    }
                },
            },
            {
                "method": "session/update",
                "params": {
                    "update": {
                        "sessionUpdate": "tool_call",
                        "toolCallId": "t2",
                        "title": "Read secret plan.md",
                        "kind": "read",
                        "status": "failed",
                    }
                },
            },
        ],
    )
    calls = tc.extract(tmp_path, "grok-build")
    assert [(c["tool"], c["status"], c["output_chars"]) for c in calls] == [
        ("run_command", "ok", 3),
        ("read", "error", None),
    ]


def test_zcode_part_state_times(tmp_path: Path) -> None:
    _write(
        tmp_path / tc.STREAM,
        [
            {"type": "message", "message_id": "m1"},
            {
                "type": "part",
                "data": {
                    "type": "tool",
                    "state": {
                        "status": "completed",
                        "output": "abcd",
                        "time": {"start": EPOCH_T0 * 1000, "end": EPOCH_T0 * 1000 + 2000},
                    },
                },
                "tool_call": {
                    "toolCallId": "p1",
                    "name": "bash",
                    "status": "completed",
                    "input": {"command": "ls"},
                },
            },
        ],
    )
    [call] = tc.extract(tmp_path, "zcode")
    assert (
        call["tool"],
        call["status"],
        call["output_chars"],
        call["started_at"],
        call["duration_ms"],
    ) == ("bash", "ok", 4, T0, 2000)


def test_devin_message_times_and_session_status(tmp_path: Path) -> None:
    _write(
        tmp_path / tc.STREAM,
        [
            {"type": "message", "created_at": EPOCH_T0, "role": "assistant"},
            {
                "type": "tool_call",
                "tool_call": {"toolCallId": "w0", "name": "edit", "input": {"path": "a"}},
            },
            {"type": "message", "created_at": EPOCH_T0 + 5, "role": "tool"},
            {
                "type": "tool_result",
                "tool_call": {"toolCallId": "w0", "status": "completed"},
                "content": "ok!",
            },
        ],
    )
    _write(
        tmp_path / "devin-session" / "tool_calls.jsonl",
        [
            {
                "tool_call_id": "w0",
                "tool_call_json": json.dumps({"toolCallId": "w0"}),
                "tool_call_update_json": json.dumps({"toolCallId": "w0", "status": "failed"}),
            },
        ],
    )
    [call] = tc.extract(tmp_path, "devin")
    assert (call["tool"], call["status"], call["output_chars"], call["duration_ms"]) == (
        "edit",
        "error",
        3,
        5000,
    )


def test_muse_code_session_log(tmp_path: Path) -> None:
    log = tmp_path / "muse-session-logs/2026/10/29/s1/session.jsonl"

    def record(us: int, event: dict[str, Any]) -> dict[str, Any]:
        return {"recorded_at": us, "payload": {"event": event}}

    _write(
        log,
        [
            record(
                EPOCH_T0 * 1_000_000,
                {
                    "kind": "assistant_tool_calls_committed",
                    "tool_calls": [
                        {
                            "id": "x",
                            "call_id": "k1",
                            "name": "bash",
                            "args": json.dumps({"command": "ls"}),
                        }
                    ],
                },
            ),
            record(
                EPOCH_T0 * 1_000_000 + 2_000_000,
                {
                    "kind": "tool_result_batch_committed",
                    "results": [
                        {"tool_call_id": "k1", "text": json.dumps({"exit_code": 3, "stdout": ""})}
                    ],
                },
            ),
        ],
    )
    _write(
        log.parent / "subagent/c1/session.jsonl",
        [
            record(
                EPOCH_T0 * 1_000_000,
                {
                    "kind": "assistant_tool_calls_committed",
                    "tool_calls": [
                        {"call_id": "k2", "name": "submit_reminder_decision", "args": "{}"}
                    ],
                },
            ),
        ],
    )
    calls = tc.extract(tmp_path, "muse-code")
    assert [(c["tool"], c["agent"], c["status"], c["exit_code"]) for c in calls] == [
        ("bash", None, "error", 3),
        ("submit_reminder_decision", "c1", "unknown", None),
    ]
    assert calls[0]["input"] == {"command": "ls"} and calls[0]["duration_ms"] == 2000


def test_muse_code_without_session_log_is_unavailable(tmp_path: Path) -> None:
    with pytest.raises(tc.CaptureUnavailable):
        tc.extract(tmp_path, "muse-code")


def test_dsh_session_log(tmp_path: Path) -> None:
    _write(
        tmp_path / "dsh-session-logs/ws/session-1/session.v4.jsonl",
        [
            {
                "type": "tool/call",
                "time": EPOCH_T0 * 1000,
                "data": {
                    "callId": "d1",
                    "name": "bash",
                    "arguments": json.dumps({"command": "ls"}),
                },
            },
            {
                "type": "tool/result",
                "time": EPOCH_T0 * 1000 + 2000,
                "data": {
                    "message": {
                        "toolCallId": "d1",
                        "isError": False,
                        "content": [{"type": "text", "text": "abc"}],
                    }
                },
            },
        ],
    )
    [call] = tc.extract(tmp_path, "dsh")
    assert (
        call["tool"],
        call["status"],
        call["output_chars"],
        call["started_at"],
        call["duration_ms"],
    ) == ("bash", "ok", 3, T0, 2000)


def test_pi_events(tmp_path: Path) -> None:
    _write(
        tmp_path / tc.STREAM,
        [
            {
                "type": "tool_execution_start",
                "toolCallId": "p1",
                "toolName": "bash",
                "args": {"command": "ls"},
            },
            {
                "type": "tool_execution_end",
                "toolCallId": "p1",
                "toolName": "bash",
                "isError": False,
                "durationMs": 2000,
                "result": {"structuredContent": {"output": "abcde", "exit_code": 0}},
            },
            {
                "type": "message_end",
                "message": {
                    "role": "toolResult",
                    "toolCallId": "p1",
                    "toolName": "bash",
                    "timestamp": EPOCH_T0 * 1000 + 2000,
                    "durationMs": 2000,
                },
            },
        ],
    )
    [call] = tc.extract(tmp_path, "pi")
    assert (
        call["status"],
        call["exit_code"],
        call["output_chars"],
        call["started_at"],
        call["ended_at"],
    ) == ("ok", 0, 5, T0, T1)


def test_mirror_decoded_rows(tmp_path: Path) -> None:
    _write(
        tmp_path / tc.STREAM,
        [
            {
                "sequence": 1,
                "role": "assistant",
                "time": EPOCH_T0,
                "tool_calls": [
                    {"id": "m1", "name": "shell", "arguments": json.dumps({"command": "ls"})}
                ],
            },
            {
                "sequence": 2,
                "role": "tool",
                "time": EPOCH_T0 + 2.5,
                "tool_call_id": "m1",
                "tool_name": "shell",
                "result": "abc",
                "success": False,
                "metadata": {"exit_code": 127, "tool_duration_seconds": 2.0},
            },
        ],
    )
    [call] = tc.extract(tmp_path, "mirror")
    assert (call["status"], call["exit_code"], call["duration_ms"], call["started_at"]) == (
        "error",
        127,
        2000,
        T0,
    )


def test_mirror_rows_without_tool_fields_are_unavailable(tmp_path: Path) -> None:
    _write(
        tmp_path / tc.STREAM,
        [
            {"sequence": 1, "role": "assistant", "time": EPOCH_T0},
            {"sequence": 2, "role": "tool", "time": EPOCH_T0 + 1},
        ],
    )
    summary = tc.write(tmp_path, "mirror")
    assert summary["capture"] == "unavailable" and not (tmp_path / tc.FILENAME).exists()


def test_write_never_raises_on_bad_run_data(tmp_path: Path) -> None:
    _write(
        tmp_path / tc.STREAM,
        [
            {
                "type": "tool_execution_end",
                "toolCallId": "p1",
                "durationMs": "soon",
                "result": "not a dict",
            }
        ],
    )
    summary = tc.write(tmp_path, "pi")
    assert summary["capture"] in ("recorded", "error")


def test_public_events_drop_inputs_and_outputs(tmp_path: Path) -> None:
    _trace(
        tmp_path,
        [
            (
                T0,
                "tool_call",
                {"tool": "run_command", "args": {"command": "cat secret"}, "id": "c1"},
            ),
            (
                T1,
                "tool_observation",
                {
                    "tool": "run_command",
                    "id": "c1",
                    "result": {"stdout": "hidden", "exit_code": 0},
                    "error": None,
                    "duration_ms": 2000,
                },
            ),
        ],
    )
    calls = tc.extract(tmp_path, "native")
    calls.append(
        {
            **calls[0],
            "seq": 2,
            "tool": "Read the plan file",
            "kind": "read",
            "agent": "goal-reminder",
        }
    )
    events = tc.public_events(calls, T0)
    text = json.dumps(events)
    assert "secret" not in text and "hidden" not in text and "plan file" not in text
    assert events[0] == {
        "event": "tool_call",
        "seq": 1,
        "tool": "run_command",
        "kind": "shell",
        "status": "ok",
        "exit_code": 0,
        "output_chars": len(json.dumps({"stdout": "hidden", "exit_code": 0})),
        "t_start_ms": 0,
        "t_end_ms": 2000,
        "duration_ms": 2000,
    }
    assert events[1]["tool"] == "read" and events[1]["agent"] == "subagent"


def test_loop_writes_the_log_and_summary_block(tmp_path: Path) -> None:
    res = run_agent(
        task_id="hello-world",
        model="mock:synthetic",
        output_dir=tmp_path,
        tasks_root=Path("tasks/v1"),
        judges=False,
    )
    run_dir = tmp_path / res["run_id"]
    block = res["summary"]["tool_calls"]
    calls = tc.read(run_dir)
    assert block["capture"] == "recorded" and block["count"] == len(calls) > 0
    assert block["timed"] == {"start": len(calls), "end": len(calls)}
    assert all(c["call_id"] and c["duration_ms"] is not None for c in calls)
