"""First-attempt check: did the session really use the requested model and work?

Infrastructure failures (bad OAuth token, a model the account cannot use, a
usage limit) can look like an ordinary failed attempt with reward 0. This
module reads the agent stream a trial writes while it runs and reports, as
early as possible, whether the requested model answered and made tool calls.
It only reads; it never stops anything.

Stream formats:
- Claude Code ``--output-format stream-json`` (``agent/claude-code.txt`` in a
  harbor trial, ``cli-agent-stream.jsonl`` in a harness run): ``system/init``
  carries ``model``; each ``assistant`` event carries ``message.model`` and
  ``tool_use`` content blocks; ``rate_limit_event`` carries
  ``rate_limit_info.status``; a refusal fallback shows up as an event whose
  type or subtype mentions ``fallback`` and as assistant turns on another model.
- Codex ``exec --json`` (``agent/codex.txt``): ``item.completed`` events with
  ``item.type == "command_execution"`` are the tool calls. Codex does not put
  the model in the stream, so it comes from the ``codex exec ... --model X``
  line in ``trial.log`` or from ``agent/trajectory.json``.
"""

from __future__ import annotations

import json
import os
import re
from collections import Counter
from dataclasses import dataclass, field
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

from backend.runner.models import FirstCheck

# Same patterns scripts/frontier-v5/run_gate.sh greps for (INFRA_RE).
INFRA_PATTERNS: tuple[str, ...] = (
    '"error":"authentication_failed"',
    "OAuth access token is invalid",
    "issue with the selected model",
    "requires usage credits",
    "hit your usage limit",
)

_CODEX_MODEL = re.compile(r"codex exec .*?--model[ =](\S+)")


@dataclass
class StreamEvidence:
    path: str
    lines: int = 0
    init_model: str | None = None
    models: Counter[str] = field(default_factory=Counter)
    tool_calls: int = 0
    infra_hits: list[str] = field(default_factory=list)
    rate_limit_status: str | None = None
    fallback_events: int = 0
    finished: bool = False  # a final "result" / "turn.completed" event was seen


# path -> (size, mtime_ns, evidence). Streams only grow, so size+mtime is enough.
_cache: dict[str, tuple[int, int, StreamEvidence]] = {}


def _cached(path: Path) -> StreamEvidence | None:
    try:
        st = path.stat()
    except OSError:
        return None
    hit = _cache.get(str(path))
    if hit and hit[0] == st.st_size and hit[1] == st.st_mtime_ns:
        return hit[2]
    return None


def _store(path: Path, ev: StreamEvidence) -> StreamEvidence:
    try:
        st = path.stat()
        _cache[str(path)] = (st.st_size, st.st_mtime_ns, ev)
    except OSError:
        pass
    return ev


def _iter_json_lines(path: Path) -> list[tuple[str, dict[str, Any] | None]]:
    out: list[tuple[str, dict[str, Any] | None]] = []
    with path.open("r", encoding="utf-8", errors="replace") as fh:
        for raw in fh:
            line = raw.strip()
            if not line:
                continue
            obj: Any = None
            if line.startswith("{"):
                try:
                    obj = json.loads(line)
                except json.JSONDecodeError:
                    obj = None  # a partial last line while the file grows
            out.append((line, obj if isinstance(obj, dict) else None))
    return out


def _note_infra(ev: StreamEvidence, line: str) -> None:
    for pat in INFRA_PATTERNS:
        if pat in line and pat not in ev.infra_hits:
            ev.infra_hits.append(pat)


def scan_claude_stream(path: Path) -> StreamEvidence | None:  # noqa: PLR0912
    cached = _cached(path)
    if cached is not None:
        return cached
    if not path.exists():
        return None
    ev = StreamEvidence(path=str(path))
    for line, obj in _iter_json_lines(path):
        ev.lines += 1
        _note_infra(ev, line)
        if obj is None:
            continue
        etype = str(obj.get("type", ""))
        subtype = str(obj.get("subtype", "") or "")
        if "fallback" in etype or "fallback" in subtype:
            ev.fallback_events += 1
        if etype == "system" and subtype == "init":
            model = obj.get("model")
            if isinstance(model, str) and ev.init_model is None:
                ev.init_model = model
        elif etype == "assistant":
            msg = obj.get("message")
            if isinstance(msg, dict):
                model = msg.get("model")
                if isinstance(model, str):
                    ev.models[model] += 1
                content = msg.get("content")
                if isinstance(content, list):
                    for block in content:
                        if isinstance(block, dict) and block.get("type") == "tool_use":
                            ev.tool_calls += 1
        elif etype == "rate_limit_event":
            info = obj.get("rate_limit_info")
            if isinstance(info, dict) and isinstance(info.get("status"), str):
                ev.rate_limit_status = str(info["status"])
        elif etype == "result":
            ev.finished = True
    return _store(path, ev)


def scan_codex_stream(path: Path) -> StreamEvidence | None:
    cached = _cached(path)
    if cached is not None:
        return cached
    if not path.exists():
        return None
    ev = StreamEvidence(path=str(path))
    for line, obj in _iter_json_lines(path):
        ev.lines += 1
        _note_infra(ev, line)
        if obj is None:
            continue
        etype = str(obj.get("type", ""))
        item = obj.get("item")
        if etype == "item.completed" and isinstance(item, dict):
            if item.get("type") == "command_execution":
                ev.tool_calls += 1
        elif etype == "turn.completed":
            ev.finished = True
    return _store(path, ev)


def codex_model_from_trial_log(path: Path) -> str | None:
    if not path.exists():
        return None
    try:
        text = path.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return None
    m = _CODEX_MODEL.search(text)
    return m.group(1) if m else None


def trajectory_model(path: Path) -> str | None:
    """``agent.model_name`` from an ATIF trajectory, read lazily (it can be MBs)."""
    if not path.exists():
        return None
    try:
        with path.open("r", encoding="utf-8") as fh:
            head = fh.read(4096)
    except OSError:
        return None
    m = re.search(r'"model_name"\s*:\s*"([^"]+)"', head)
    return m.group(1) if m else None


def evaluate(  # noqa: PLR0912
    ev: StreamEvidence | None,
    *,
    requested_model: str | None,
    agent_started_at: datetime | None,
    now: datetime,
    window_s: int,
    trial_finished: bool,
    reported_model: str | None = None,
) -> FirstCheck:
    """Turn stream evidence into a verdict.

    ``reported_model`` is a model name found outside the stream (Codex's
    ``trial.log`` invocation line); for Claude the stream itself carries it.
    """
    if ev is None:
        return FirstCheck(
            status="unknown",
            reasons=["no agent stream on disk yet"],
            requested_model=requested_model,
        )
    window_open = True
    if agent_started_at is not None:
        window_open = (now - agent_started_at).total_seconds() < window_s
    if trial_finished or ev.finished:
        window_open = False
    reasons: list[str] = []
    status = "ok"
    models = dict(ev.models)
    if reported_model and not models:
        models = {reported_model: 0}
    if ev.infra_hits:
        status = "fail"
        reasons.append("infrastructure error pattern in the agent stream: " + ev.infra_hits[0])
    if ev.fallback_events:
        status = "fail"
        reasons.append(f"{ev.fallback_events} model fallback event(s)")
    if requested_model:
        seen = {m for m in models if m}
        if ev.init_model and ev.init_model != requested_model:
            status = "fail"
            reasons.append(f"session initialised with {ev.init_model}, not {requested_model}")
        wrong = sorted(m for m in seen if m != requested_model)
        if wrong:
            status = "fail"
            reasons.append(
                "assistant turns on " + ", ".join(wrong) + f" instead of {requested_model}"
            )
        if not seen and not ev.init_model and not window_open and status == "ok":
            status = "warning"
            reasons.append("no model name recorded in the stream")
    elif status == "ok":
        status = "warning"
        reasons.append("requested model unknown")
    if ev.rate_limit_status and ev.rate_limit_status != "allowed" and status == "ok":
        status = "warning"
        reasons.append(f"rate limit status {ev.rate_limit_status}")
    if ev.tool_calls == 0:
        if window_open and status == "ok":
            status = "pending"
            reasons.append("no tool calls yet; check window still open")
        elif status in {"ok", "warning"}:
            status = "fail"
            reasons.append("no tool calls in the agent stream")
    if status == "ok":
        reasons.append(f"{ev.tool_calls} tool call(s) on {requested_model}")
    return FirstCheck(
        status=status,
        reasons=reasons,
        requested_model=requested_model,
        models=models,
        tool_calls=ev.tool_calls,
        infra_hits=list(ev.infra_hits),
        rate_limit_status=ev.rate_limit_status,
        evidence=os.path.basename(ev.path),
        window_open=window_open,
    )


def check_agent_dir(
    trial_dir: Path,
    *,
    harness: str | None,
    requested_model: str | None,
    agent_started_at: datetime | None,
    now: datetime | None,
    window_s: int,
    trial_finished: bool,
) -> FirstCheck:
    """Run the check for a harbor trial directory (``agent/`` beneath it)."""
    now = now or datetime.now(UTC)
    agent_dir = trial_dir / "agent"
    claude = agent_dir / "claude-code.txt"
    codex = agent_dir / "codex.txt"
    if harness == "codex" or (harness is None and codex.exists()):
        ev = scan_codex_stream(codex)
        reported = codex_model_from_trial_log(trial_dir / "trial.log") or trajectory_model(
            agent_dir / "trajectory.json"
        )
        return evaluate(
            ev,
            requested_model=requested_model,
            agent_started_at=agent_started_at,
            now=now,
            window_s=window_s,
            trial_finished=trial_finished,
            reported_model=reported,
        )
    ev = scan_claude_stream(claude)
    return evaluate(
        ev,
        requested_model=requested_model,
        agent_started_at=agent_started_at,
        now=now,
        window_s=window_s,
        trial_finished=trial_finished,
    )


def check_harness_run(
    run_dir: Path,
    *,
    harness: str | None,
    requested_model: str | None,
    agent_started_at: datetime | None,
    now: datetime | None,
    window_s: int,
    run_finished: bool,
) -> FirstCheck:
    """Run the check for a ``vulcanbench run`` directory (``cli-agent-stream.jsonl``)."""
    now = now or datetime.now(UTC)
    stream = run_dir / "cli-agent-stream.jsonl"
    ev = scan_codex_stream(stream) if harness == "codex" else scan_claude_stream(stream)
    return evaluate(
        ev,
        requested_model=requested_model,
        agent_started_at=agent_started_at,
        now=now,
        window_s=window_s,
        trial_finished=run_finished,
    )
