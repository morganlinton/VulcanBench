"""Every run's tool calls in one normalized log, ``tool-calls.jsonl``.

Each harness keeps its own record of tool use: the native loop writes trace
events, a CLI agent streams JSON events, and some leave a session log behind.
Every one has a different shape. This module reads whichever record the run's
harness produced and writes one line per tool call in a shared schema, so
every run directory answers the same questions: which tools were called, in
what order, when, and how each ended.

The loop writes the log when a run finishes and puts a count block in
``summary.json`` (``tool_calls``). ``vulcanbench tool-calls`` builds the log for
runs that predate it; it never edits their ``summary.json``, whose hash frozen
judging rounds pin.

The log is private like the rest of the run directory: ``input`` keeps the
call's arguments as the harness recorded them (sanitized). Public traces take
only :func:`public_events`, which drops inputs and outputs.

One line per call, in session order::

    seq          1-based position
    call_id      the harness's id for the call, or null
    tool         the harness's own tool name (Codex: the item type)
    kind         shell | edit | read | search | web | mcp | plan | agent | other
    agent        null for the main agent, else the sub-agent that made the call
    input        the call's arguments (private)
    status       ok | error | rejected | unknown
    exit_code    process exit code where the harness reports one
    output_chars length of the tool's output where recorded
    started_at   UTC ISO time the call started, where recorded
    ended_at     UTC ISO time the result arrived, where recorded
    duration_ms  ended minus started, or the harness's own duration
"""

from __future__ import annotations

import json
import re
from collections import Counter
from collections.abc import Callable, Iterator
from datetime import UTC, datetime, timedelta
from pathlib import Path
from typing import Any

from harness.agent.muse_code import session_records
from harness.redaction import sanitize

SCHEMA = "vulcanbench-tool-calls/1"
FILENAME = "tool-calls.jsonl"
STREAM = "cli-agent-stream.jsonl"
TRACE = "trace.jsonl"
STATUSES = ("ok", "error", "rejected", "unknown")

# Matched on the name's words ("TodoWrite" is todo + write, "run_terminal_cmd" is
# run + terminal + cmd). First match wins, so the narrow kinds come first: TodoWrite
# is a plan, not an edit; WebFetch is web, not read.
_KINDS: tuple[tuple[str, frozenset[str]], ...] = (
    ("web", frozenset({"web", "fetch", "browse", "url"})),
    ("mcp", frozenset({"mcp"})),
    ("plan", frozenset({"todo", "todos", "plan"})),
    ("agent", frozenset({"task", "agent", "subagent", "delegate"})),
    (
        "shell",
        frozenset(
            {
                "bash",
                "shell",
                "command",
                "exec",
                "execute",
                "execution",
                "terminal",
                "run",
                "cmd",
                "sh",
            }
        ),
    ),
    (
        "edit",
        frozenset(
            {
                "edit",
                "editor",
                "multiedit",
                "write",
                "patch",
                "change",
                "replace",
                "create",
                "delete",
                "move",
                "rename",
                "insert",
                "notebook",
            }
        ),
    ),
    ("read", frozenset({"read", "view", "open", "cat"})),
    ("search", frozenset({"grep", "glob", "search", "find", "list", "ls", "dir", "locate", "rg"})),
)
_WORDS = re.compile(r"[a-z]+|[0-9]+")
_PUBLIC_NAME = re.compile(r"^[A-Za-z0-9_.:/-]{1,64}$")


class CaptureUnavailable(Exception):
    """The run's record exists but holds no tool calls to normalize (missing file, older format)."""


def tool_kind(name: str) -> str:
    words = set(_WORDS.findall(re.sub(r"([a-z0-9])([A-Z])", r"\1 \2", name).lower()))
    for kind, vocabulary in _KINDS:
        if words & vocabulary:
            return kind
    return "other"


def _jsonl(path: Path) -> Iterator[dict[str, Any]]:
    if not path.exists():
        raise CaptureUnavailable(f"{path.name} is missing")
    with path.open(encoding="utf-8", errors="replace") as source:
        for raw_line in source:
            line = raw_line.strip()
            if not line:
                continue
            try:
                record = json.loads(line)
            except json.JSONDecodeError:
                continue
            if isinstance(record, dict):
                yield record


def _epoch(value: Any) -> str | None:
    """UTC ISO time from an epoch number in seconds, ms, µs or ns (inferred from its size)."""
    if isinstance(value, bool) or not isinstance(value, (int, float)) or value <= 0:
        return None
    for scale in (1e9, 1e6, 1e3):
        if value > 1e8 * scale:
            value = value / scale
            break
    return datetime.fromtimestamp(value, UTC).isoformat()


def _iso(value: Any) -> str | None:
    if not isinstance(value, str) or not value:
        return None
    try:
        return datetime.fromisoformat(value.replace("Z", "+00:00")).astimezone(UTC).isoformat()
    except ValueError:
        return None


def _chars(content: Any) -> int | None:
    if content is None:
        return None
    if isinstance(content, str):
        return len(content)
    if isinstance(content, list) and all(isinstance(b, dict) for b in content):
        return sum(len(str(b.get("text") or "")) for b in content)
    return len(json.dumps(content, default=str))


def _dict(value: Any) -> dict[str, Any]:
    return value if isinstance(value, dict) else {}


def _args(value: Any) -> Any:
    """Arguments a harness stored as a JSON string, decoded where they parse."""
    if isinstance(value, str):
        try:
            return json.loads(value)
        except json.JSONDecodeError:
            return value
    return value


def _exit_status(exit_code: Any, failed: bool = False) -> str:
    if failed:
        return "error"
    if isinstance(exit_code, int) and not isinstance(exit_code, bool):
        return "ok" if exit_code == 0 else "error"
    return "ok"


class _Calls:
    """Calls in first-seen order, addressable by id."""

    def __init__(self) -> None:
        self.order: list[dict[str, Any]] = []
        self.by_id: dict[str, dict[str, Any]] = {}

    def get(self, call_id: Any, tool: Any = None, **fields: Any) -> dict[str, Any]:
        key = str(call_id) if call_id not in (None, "") else None
        if key is not None and key in self.by_id:
            call = self.by_id[key]
        else:
            call = {
                "call_id": key,
                "tool": None,
                "agent": None,
                "input": None,
                "status": "unknown",
                "exit_code": None,
                "output_chars": None,
                "started_at": None,
                "ended_at": None,
                "duration_ms": None,
            }
            self.order.append(call)
            if key is not None:
                self.by_id[key] = call
        if tool and not call["tool"]:
            call["tool"] = str(tool)
        for name, value in fields.items():
            if value is not None:
                call[name] = value
        return call


# --- one extractor per harness ------------------------------------------------------


def _native(run_dir: Path) -> list[dict[str, Any]]:
    """The loop's own trace: tool_call then tool_observation for each call.

    Since the tool-call log, ``tool_call`` is written before the tool runs and the
    observation carries the call id and duration. Older traces wrote both after
    the tool ran and the observation had no id, so they pair in order and only
    the end time is known.
    """
    calls = _Calls()
    pending: list[dict[str, Any]] = []
    for event in _jsonl(run_dir / TRACE):
        data = event.get("data") or {}
        if event.get("type") == "tool_call":
            call = calls.get(data.get("id"), data.get("tool"), input=data.get("args"))
            call["_ts"] = event.get("ts")
            pending.append(call)
        elif event.get("type") == "tool_observation" and pending:
            done = calls.by_id.get(str(data.get("id"))) if data.get("id") else pending[0]
            if done is None or done not in pending:
                continue
            pending.remove(done)
            if "duration_ms" in data:  # new format: the tool_call event marks the start
                done["started_at"] = _iso(done["_ts"])
                done["duration_ms"] = data["duration_ms"]
            done["ended_at"] = _iso(event.get("ts"))
            result = data.get("result")
            exit_code = _dict(result).get("exit_code")
            done["exit_code"] = exit_code if isinstance(exit_code, int) else None
            done["status"] = _exit_status(done["exit_code"], failed=bool(data.get("error")))
            done["output_chars"] = _chars(result)
    for call in calls.order:
        call.pop("_ts", None)
    return calls.order


def _claude_code(run_dir: Path) -> list[dict[str, Any]]:
    """Claude Code's stream-json: tool_use blocks in assistant events, tool_result blocks in user events."""
    calls = _Calls()
    for event in _jsonl(run_dir / STREAM):
        kind = event.get("type")
        if kind not in ("assistant", "user"):
            continue
        blocks = [
            b for b in (event.get("message") or {}).get("content") or [] if isinstance(b, dict)
        ]
        when = _iso(event.get("timestamp"))
        for block in blocks:
            if kind == "assistant" and block.get("type") == "tool_use":
                calls.get(
                    block.get("id"),
                    block.get("name"),
                    input=block.get("input"),
                    started_at=when,
                    agent="subagent" if event.get("parent_tool_use_id") else None,
                )
            elif kind == "user" and block.get("type") == "tool_result":
                call = calls.get(block.get("tool_use_id"))
                call.update(
                    ended_at=when,
                    status="error" if block.get("is_error") else "ok",
                    output_chars=_chars(block.get("content")),
                )
    return calls.order


_CODEX_TOOLS = ("command_execution", "file_change", "mcp_tool_call", "web_search")


def _codex_name(item: dict[str, Any]) -> str:
    if item.get("type") == "mcp_tool_call":
        return f"mcp:{item.get('server')}.{item.get('tool')}"
    return str(item.get("type"))


def _codex_input(item: dict[str, Any]) -> dict[str, Any]:
    kind = item.get("type")
    if kind == "command_execution":
        return {"command": item.get("command")}
    if kind == "file_change":
        return {"changes": item.get("changes")}
    if kind == "mcp_tool_call":
        return {"arguments": item.get("arguments")}
    return {"query": item.get("query")}


def _codex(run_dir: Path) -> list[dict[str, Any]]:
    """Codex's exec --json items, timed from the harness trace.

    The stream has no times. The trace stamps each completed tool item
    (``tool_observation``) and, since the tool-call log, each started one
    (``tool_call``) with the item id. Older traces carry no id; their
    observations are in the same order as the completed tool items.
    """
    calls = _Calls()
    completed: list[dict[str, Any]] = []
    for event in _jsonl(run_dir / STREAM):
        item = event.get("item") or {}
        if (
            event.get("type") not in ("item.started", "item.completed")
            or item.get("type") not in _CODEX_TOOLS
        ):
            continue
        kind = item["type"]
        call = calls.get(item.get("id"), _codex_name(item), input=_codex_input(item), _type=kind)
        if event["type"] == "item.completed":
            status = item.get("status")
            call["exit_code"] = (
                item.get("exit_code") if isinstance(item.get("exit_code"), int) else None
            )
            call["status"] = _exit_status(
                call["exit_code"], failed=status in ("failed", "declined")
            )
            output = (
                item.get("aggregated_output") if kind == "command_execution" else item.get("result")
            )
            call["output_chars"] = _chars(output)
            completed.append(call)
    trace = list(_jsonl(run_dir / TRACE))
    for event in trace:
        data = event.get("data") or {}
        if event.get("type") == "tool_call" and data.get("id") in calls.by_id:
            calls.by_id[data["id"]]["started_at"] = _iso(event.get("ts"))
    observed = [e for e in trace if e.get("type") == "tool_observation"]
    if observed and all((e.get("data") or {}).get("id") for e in observed):
        for event in observed:
            done = calls.by_id.get(event["data"]["id"])
            if done is not None:
                done["ended_at"] = _iso(event.get("ts"))
    elif len(observed) == len(completed) and all(
        (e.get("data") or {}).get("tool") == c["_type"]
        for e, c in zip(observed, completed, strict=True)
    ):
        for event, call in zip(observed, completed, strict=True):
            call["ended_at"] = _iso(event.get("ts"))
    for call in calls.order:
        call.pop("_type", None)
    return calls.order


def _cursor(run_dir: Path) -> list[dict[str, Any]]:
    """Cursor's stream-json tool_call events (started and completed), timed from the trace by call id."""
    calls = _Calls()
    for event in _jsonl(run_dir / STREAM):
        if event.get("type") != "tool_call" or not isinstance(event.get("tool_call"), dict):
            continue
        name, body = next(iter(event["tool_call"].items()), (None, None))
        body = body if isinstance(body, dict) else {}
        args = body.get("args") if isinstance(body.get("args"), dict) else None
        call_id = event.get("call_id") or (args or {}).get("toolCallId")
        call = calls.get(call_id, str(name or "unknown").removesuffix("ToolCall"), input=args)
        result = body.get("result")
        if event.get("subtype") == "completed" and isinstance(result, dict):
            success = result.get("success") if isinstance(result.get("success"), dict) else None
            exit_code = (success or {}).get("exitCode")
            call["exit_code"] = exit_code if isinstance(exit_code, int) else None
            if "rejected" in result:
                call["status"] = "rejected"
            elif "error" in result or "failure" in result:
                call["status"] = "error"
            else:
                call["status"] = _exit_status(call["exit_code"])
            call["output_chars"] = _chars(success if success is not None else result)
    for event in _jsonl(run_dir / TRACE):
        timed = calls.by_id.get(str((event.get("data") or {}).get("tool")))
        if timed is None:
            continue
        if event.get("type") == "tool_call":
            timed["started_at"] = _iso(event.get("ts"))
        elif event.get("type") == "tool_observation":
            timed["ended_at"] = _iso(event.get("ts"))
    return calls.order


_ACP_STATUS = {"completed": "ok", "failed": "error", "cancelled": "error", "rejected": "rejected"}


def _grok_build(run_dir: Path) -> list[dict[str, Any]]:
    """Grok Build's session updates (Agent Client Protocol tool_call and tool_call_update), folded into the stream."""
    calls = _Calls()
    for event in _jsonl(run_dir / STREAM):
        update = (event.get("params") or {}).get("update") or event.get("update") or event
        if not isinstance(update, dict) or update.get("sessionUpdate") not in (
            "tool_call",
            "tool_call_update",
        ):
            continue
        title = update.get("title")
        name = title if isinstance(title, str) and _PUBLIC_NAME.match(title) else update.get("kind")
        call = calls.get(update.get("toolCallId"), name, input=update.get("rawInput"))
        status = _ACP_STATUS.get(str(update.get("status")))
        if status:
            call["status"] = status
        if "rawOutput" in update or "content" in update:
            call["output_chars"] = _chars(update.get("rawOutput", update.get("content")))
    return calls.order


def _zcode(run_dir: Path) -> list[dict[str, Any]]:
    """Z Code's harvested session parts: a tool part carries its state, with start and end times."""
    calls = _Calls()
    for event in _jsonl(run_dir / STREAM):
        tool_call = event.get("tool_call")
        if event.get("type") != "part" or not isinstance(tool_call, dict):
            continue
        state = _dict(_dict(event.get("data")).get("state"))
        times = _dict(state.get("time"))
        output = state.get("output")
        calls.get(
            tool_call.get("toolCallId"),
            tool_call.get("name"),
            input=tool_call.get("input"),
            status={"completed": "ok", "error": "error"}.get(
                str(tool_call.get("status")), "unknown"
            ),
            started_at=_epoch(times.get("start")),
            ended_at=_epoch(times.get("end")),
            output_chars=_chars(output) if output is not None else None,
        )
    return calls.order


def _devin(run_dir: Path) -> list[dict[str, Any]]:
    """Devin's harvested session: tool_call and tool_result lines, each after the message that holds it.

    Times are the holding message's ``created_at`` (whole seconds). The stream's
    result status is always "completed", so status comes from the session's
    tool_call_state rows (``devin-session/tool_calls.jsonl``) where present.
    """
    calls = _Calls()
    when = None
    for event in _jsonl(run_dir / STREAM):
        kind = event.get("type")
        tool_call = _dict(event.get("tool_call"))
        if kind == "message":
            when = _epoch(event.get("created_at"))
        elif kind == "tool_call":
            calls.get(
                tool_call.get("toolCallId"),
                tool_call.get("name"),
                input=tool_call.get("input"),
                started_at=when,
            )
        elif kind == "tool_result":
            calls.get(
                tool_call.get("toolCallId"),
                ended_at=when,
                output_chars=_chars(event.get("content")),
            )
    states = run_dir / "devin-session" / "tool_calls.jsonl"
    if states.exists():
        for row in _jsonl(states):
            update = _args(row.get("tool_call_update_json"))
            call = calls.by_id.get(str(row.get("tool_call_id")))
            if call is not None and isinstance(update, dict):
                call["status"] = _ACP_STATUS.get(str(update.get("status")), call["status"])
    return calls.order


def _muse_result(text: str) -> tuple[str, int | None]:
    """(status, exit code) for a Muse tool result, read from its status fields only."""
    try:
        body = json.loads(text)
    except (json.JSONDecodeError, TypeError):
        return ("error" if (text or "").startswith("tool failed") else "ok"), None
    if not isinstance(body, dict):
        return "ok", None
    exit_code = body.get("exit_code") if isinstance(body.get("exit_code"), int) else None
    if exit_code is not None or "terminal_status" in body:
        failed = body.get("terminal_status") in ("failed", "timed_out", "cancelled")
        return _exit_status(exit_code, failed=failed), exit_code
    if "ok" in body:
        return ("ok" if body["ok"] is True else "error"), None
    return "ok", None


def _muse_code(run_dir: Path) -> list[dict[str, Any]]:
    """Muse Code's archived session logs: committed tool calls and committed result batches, with recorded_at times."""
    logs = sorted(
        run_dir.glob("muse-session-logs/**/session.jsonl"),
        key=lambda p: ("/subagent/" in p.as_posix(), p),
    )
    if not logs:
        raise CaptureUnavailable("no Muse session log in muse-session-logs/")
    calls = _Calls()
    for path in logs:
        agent = path.parent.name if "/subagent/" in path.as_posix() else None
        for row in _jsonl(path):
            for record in session_records(row):
                payload = record.get("payload") or {}
                event = payload.get("event", payload)
                if not isinstance(event, dict):
                    continue
                when = _epoch(record.get("recorded_at"))
                if event.get("kind") == "assistant_tool_calls_committed":
                    for item in event.get("tool_calls") or []:
                        call = calls.get(
                            item.get("call_id"),
                            item.get("name"),
                            input=_args(item.get("args")),
                            agent=agent,
                        )
                        call["started_at"] = call["started_at"] or when
                elif event.get("kind") == "tool_result_batch_committed":
                    for result in event.get("results") or []:
                        text = str(result.get("text") or "")
                        status, exit_code = _muse_result(text)
                        call = calls.get(result.get("tool_call_id"))
                        if call["ended_at"] is None:
                            call.update(
                                ended_at=when,
                                status=status,
                                exit_code=exit_code,
                                output_chars=len(text),
                            )
    return calls.order


def _dsh(run_dir: Path) -> list[dict[str, Any]]:
    """The DeepSeek harness's session log: tool/call and tool/result events with times."""
    logs = sorted(run_dir.glob("dsh-session-logs/**/session.v4.jsonl"))
    if not logs:
        raise CaptureUnavailable("no dsh session log in dsh-session-logs/")
    calls = _Calls()
    for path in logs:
        for event in _jsonl(path):
            data = event.get("data") or {}
            if event.get("type") == "tool/call":
                calls.get(
                    data.get("callId"),
                    data.get("name"),
                    input=_args(data.get("arguments")),
                    started_at=_epoch(event.get("time")),
                )
            elif event.get("type") == "tool/result":
                message = data.get("message") or {}
                calls.get(
                    message.get("toolCallId"),
                    ended_at=_epoch(event.get("time")),
                    status="error" if message.get("isError") else "ok",
                    output_chars=_chars(message.get("content")),
                )
    return calls.order


def _pi(run_dir: Path) -> list[dict[str, Any]]:
    """Pi's JSON events: tool_execution_start/end, and the toolResult message's absolute time."""
    calls = _Calls()
    for event in _jsonl(run_dir / STREAM):
        kind = event.get("type")
        if kind == "tool_execution_start":
            calls.get(event.get("toolCallId"), event.get("toolName"), input=event.get("args"))
        elif kind == "tool_execution_end":
            result = _dict(event.get("result"))
            structured = _dict(result.get("structuredContent"))
            exit_code = structured.get("exit_code")
            exit_code = exit_code if isinstance(exit_code, int) else None
            output = structured.get("output") if "output" in structured else result.get("content")
            calls.get(
                event.get("toolCallId"),
                event.get("toolName"),
                exit_code=exit_code,
                status=_exit_status(exit_code, failed=bool(event.get("isError"))),
                duration_ms=event.get("durationMs"),
                output_chars=_chars(output),
            )
        elif kind == "message_end" and (event.get("message") or {}).get("role") == "toolResult":
            message = event["message"]
            call = calls.get(message.get("toolCallId"), message.get("toolName"))
            ended = message.get("timestamp")
            call["ended_at"] = _epoch(ended)
            duration = message.get("durationMs", call["duration_ms"])
            if isinstance(ended, (int, float)) and isinstance(duration, (int, float)):
                call["started_at"] = _epoch(ended - duration)
    return calls.order


def _mirror(run_dir: Path) -> list[dict[str, Any]]:
    """Mirror's decoded conversation rows: assistant rows list tool calls, tool rows carry results.

    Rows decoded before the adapter archived full traces (a11353ed5) keep no tool
    names, arguments or results, so those runs have no tool calls to log.
    """
    rows = list(_jsonl(run_dir / STREAM))
    if any(r.get("role") == "tool" for r in rows) and not any(
        "tool_call_id" in r or "tool_calls" in r for r in rows
    ):
        raise CaptureUnavailable(
            "Mirror rows predate tool-call decoding; no tool names or results were kept"
        )
    calls = _Calls()
    for row in rows:
        when = _epoch(row.get("time"))
        if row.get("role") == "assistant":
            for item in row.get("tool_calls") or []:
                calls.get(
                    item.get("id"),
                    item.get("name"),
                    input=_args(item.get("arguments")),
                    started_at=when,
                )
        elif row.get("role") == "tool":
            metadata = row.get("metadata") or {}
            exit_code = (
                metadata.get("exit_code") if isinstance(metadata.get("exit_code"), int) else None
            )
            seconds = metadata.get("tool_duration_seconds")
            call = calls.get(
                row.get("tool_call_id"),
                row.get("tool_name"),
                ended_at=when,
                exit_code=exit_code,
                status=_exit_status(exit_code, failed=row.get("success") is False),
                output_chars=_chars(row.get("result")),
            )
            if isinstance(seconds, (int, float)) and seconds > 0:
                call["duration_ms"] = round(seconds * 1000)
    return calls.order


EXTRACTORS: dict[str, Callable[[Path], list[dict[str, Any]]]] = {
    "native": _native,
    "claude-code": _claude_code,
    "codex": _codex,
    "cursor": _cursor,
    "grok-build": _grok_build,
    "zcode": _zcode,
    "devin": _devin,
    "muse-code": _muse_code,
    "dsh": _dsh,
    "pi": _pi,
    "mirror": _mirror,
}


# --- the log and its summary block ------------------------------------------------


def _finish(calls: list[dict[str, Any]]) -> list[dict[str, Any]]:
    out = []
    for seq, call in enumerate(calls, 1):
        tool = call["tool"] or "unknown"
        started, ended = call["started_at"], call["ended_at"]
        duration = call["duration_ms"]
        if isinstance(duration, bool) or not isinstance(duration, (int, float)):
            duration = None
        else:
            duration = round(duration)
        if duration is None and started and ended:
            duration = round(
                (datetime.fromisoformat(ended) - datetime.fromisoformat(started)).total_seconds()
                * 1000
            )
        if started is None and ended and isinstance(duration, (int, float)):
            started = (datetime.fromisoformat(ended) - timedelta(milliseconds=duration)).isoformat()
        out.append(
            {
                "seq": seq,
                "call_id": call["call_id"],
                "tool": tool,
                "kind": tool_kind(tool),
                "agent": call["agent"],
                "input": call["input"],
                "status": call["status"] if call["status"] in STATUSES else "unknown",
                "exit_code": call["exit_code"],
                "output_chars": call["output_chars"],
                "started_at": started,
                "ended_at": ended,
                "duration_ms": duration,
            }
        )
    return out


def extract(run_dir: Path, harness: str) -> list[dict[str, Any]]:
    """Normalized tool calls for one run. Raises CaptureUnavailable when the record holds none."""
    if harness not in EXTRACTORS:
        raise CaptureUnavailable(f"no tool-call extractor for harness {harness!r}")
    return _finish(EXTRACTORS[harness](run_dir))


def summary_block(calls: list[dict[str, Any]], harness: str) -> dict[str, Any]:
    return {
        "schema": SCHEMA,
        "file": FILENAME,
        "harness": harness,
        "capture": "recorded",
        "count": len(calls),
        "errors": sum(c["status"] == "error" for c in calls),
        "by_tool": dict(sorted(Counter(c["tool"] for c in calls).items())),
        "by_kind": dict(sorted(Counter(c["kind"] for c in calls).items())),
        "timed": {
            "start": sum(c["started_at"] is not None for c in calls),
            "end": sum(c["ended_at"] is not None for c in calls),
        },
    }


def save(run_dir: Path, calls: list[dict[str, Any]]) -> None:
    with (run_dir / FILENAME).open("w", encoding="utf-8") as sink:
        for call in calls:
            sink.write(json.dumps(sanitize(call), default=str) + "\n")


def write(run_dir: Path, harness: str) -> dict[str, Any]:
    """Write ``tool-calls.jsonl`` and return the summary block. Never raises on a run's own data."""
    try:
        calls = extract(run_dir, harness)
    except CaptureUnavailable as exc:
        return {"schema": SCHEMA, "harness": harness, "capture": "unavailable", "reason": str(exc)}
    except (OSError, ValueError, KeyError, TypeError, AttributeError) as exc:
        return {
            "schema": SCHEMA,
            "harness": harness,
            "capture": "error",
            "reason": f"{type(exc).__name__}: {exc}"[:300],
        }
    save(run_dir, calls)
    return summary_block(calls, harness)


def harness_of(summary: dict[str, Any]) -> str:
    return str((summary.get("cli_agent") or {}).get("harness") or "native")


# --- public traces ----------------------------------------------------------------


def public_events(calls: list[dict[str, Any]], origin: str | datetime) -> list[dict[str, Any]]:
    """Tool-call events for a public trace: names, kinds, timing and outcomes, never inputs or outputs.

    Times become milliseconds from ``origin`` (the agent's start). A tool name
    that is not a plain identifier (some harnesses title calls with their
    arguments) is published as its kind instead.
    """
    start = origin if isinstance(origin, datetime) else datetime.fromisoformat(origin)

    def offset(value: str | None) -> int | None:
        return (
            None
            if value is None
            else round((datetime.fromisoformat(value) - start).total_seconds() * 1000)
        )

    return [
        {
            "event": "tool_call",
            "seq": c["seq"],
            "tool": c["tool"] if _PUBLIC_NAME.match(c["tool"]) else c["kind"],
            "kind": c["kind"],
            **({"agent": "subagent"} if c["agent"] else {}),
            "status": c["status"],
            "exit_code": c["exit_code"],
            "output_chars": c["output_chars"],
            "t_start_ms": offset(c["started_at"]),
            "t_end_ms": offset(c["ended_at"]),
            "duration_ms": c["duration_ms"],
        }
        for c in calls
    ]


def read(run_dir: Path) -> list[dict[str, Any]]:
    """A run's tool-call log as written."""
    return list(_jsonl(run_dir / FILENAME))
