"""Subscription usage-window snapshots for Claude Code runs.

Claude Code streams a ``rate_limit_event`` on every request whose
``rate_limit_info.unifiedWindows`` carries the subscription's usage windows
(five-hour, seven-day) as utilization fractions.  A flat-rate seat has no
per-task price, but the change in a window across one task is the seat's
real per-task cost, so the harness records it instead of leaving
``economics.quota`` at "unavailable".

Two sources, in order of quality:

* ``probe``: a one-turn ``claude -p`` call before and after the task (the
  same call ``scripts/frontier-v5/run_gate.sh`` makes for its usage
  preflight).  Opt-in with ``VULCANBENCH_QUOTA_PROBE=1`` because it spends a
  few hundred tokens of the seat per task and adds a few seconds.
* ``stream``: the first and last ``rate_limit_event`` inside the task's own
  stream.  Free, always recorded, but the last event precedes the final
  request's accounting, so it understates the delta slightly.
"""

from __future__ import annotations

import json
import subprocess
from typing import Any

PROBE_PROMPT = "Reply with the single word ok."


def parse_rate_limit_info(info: Any) -> dict[str, Any] | None:
    """Normalize one ``rate_limit_info`` payload to ``{status, overage, windows}``.

    ``windows`` maps each unified window name to its utilization (0 to 1) plus
    any reset timestamp the CLI reported.  Returns ``None`` when the payload
    carries no windows.
    """
    if not isinstance(info, dict):
        return None
    raw_windows = info.get("unifiedWindows")
    if not isinstance(raw_windows, dict) or not raw_windows:
        return None
    windows: dict[str, dict[str, Any]] = {}
    for name, window in raw_windows.items():
        if not isinstance(window, dict):
            continue
        utilization = window.get("utilization")
        if not isinstance(utilization, (int, float)):
            continue
        entry: dict[str, Any] = {"utilization": float(utilization)}
        for key in ("resetsAt", "resets_at"):
            if window.get(key) is not None:
                entry["resets_at"] = window[key]
                break
        windows[str(name)] = entry
    if not windows:
        return None
    return {
        "status": info.get("status"),
        "overage": info.get("isUsingOverage"),
        "windows": windows,
    }


def parse_stream_lines(lines: Any) -> dict[str, Any] | None:
    """The last ``rate_limit_event`` in a stream-json transcript, normalized."""
    last: dict[str, Any] | None = None
    for raw in lines:
        line = raw.strip() if isinstance(raw, str) else ""
        if not line or '"rate_limit_event"' not in line:
            continue
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            continue
        if event.get("type") != "rate_limit_event":
            continue
        parsed = parse_rate_limit_info(event.get("rate_limit_info"))
        if parsed is not None:
            last = parsed
    return last


def utilizations(snapshot: dict[str, Any] | None) -> dict[str, float]:
    """``{window: utilization}`` for a normalized snapshot (empty when None)."""
    if not snapshot:
        return {}
    return {name: w["utilization"] for name, w in snapshot["windows"].items()}


def window_delta(
    before: dict[str, Any] | None, after: dict[str, Any] | None
) -> dict[str, float] | None:
    """Per-window utilization change, or ``None`` when either side is missing.

    A window that reset between the two snapshots (after < before) is reported
    as ``None`` for that window rather than a negative spend.
    """
    if not before or not after:
        return None
    b, a = utilizations(before), utilizations(after)
    delta: dict[str, Any] = {}
    for name in sorted(set(b) & set(a)):
        change = round(a[name] - b[name], 6)
        delta[name] = change if change >= 0 else None
    return delta or None


def probe_claude_usage(
    claude_bin: str,
    model: str,
    env: dict[str, str] | None = None,
    timeout_s: float = 180.0,
) -> dict[str, Any] | None:
    """One-turn ``claude -p`` call; returns the normalized usage snapshot.

    Any failure (no CLI, timeout, no rate-limit event) returns ``None`` so a
    probe never fails the task it brackets.
    """
    cmd = [
        claude_bin,
        "-p",
        PROBE_PROMPT,
        "--model",
        model,
        "--output-format",
        "stream-json",
        "--verbose",
        "--max-turns",
        "1",
        "--no-session-persistence",
        "--setting-sources",
        "project",
    ]
    try:
        proc = subprocess.run(
            cmd,
            env=env,
            text=True,
            encoding="utf-8",
            errors="replace",
            capture_output=True,
            timeout=timeout_s,
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired):
        return None
    return parse_stream_lines(proc.stdout.splitlines())


def quota_block(
    *,
    stream_first: dict[str, Any] | None,
    stream_last: dict[str, Any] | None,
    probe_before: dict[str, Any] | None = None,
    probe_after: dict[str, Any] | None = None,
) -> dict[str, Any] | None:
    """Assemble ``economics.quota`` from whichever snapshots exist.

    ``before``/``after``/``delta`` come from the probe when both probe sides
    exist, else from the stream's first and last events; ``source`` says
    which.  Both raw pairs are kept so a reader can see the understatement.
    """
    if probe_before and probe_after:
        source, before, after = "probe", probe_before, probe_after
    elif stream_first and stream_last:
        source, before, after = "stream", stream_first, stream_last
    else:
        return None
    return {
        "source": source,
        "before": utilizations(before),
        "after": utilizations(after),
        "delta": window_delta(before, after),
        "status": after.get("status"),
        "overage": after.get("overage"),
        "resets_at": {
            name: w.get("resets_at") for name, w in after["windows"].items() if w.get("resets_at")
        },
        "stream": {"first": utilizations(stream_first), "last": utilizations(stream_last)},
        "probe": {"before": utilizations(probe_before), "after": utilizations(probe_after)},
    }
