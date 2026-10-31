"""Server-sent events for a growing log file: the tail once, then only what was appended."""

from __future__ import annotations

import json
import time
from collections.abc import Iterator
from pathlib import Path
from typing import Any

from backend.runner.discovery import tail_text
from backend.runner.redact import redact

POLL_S = 1.0
MAX_CHUNK = 256_000


def _event(name: str, data: dict[str, Any]) -> str:
    return f"event: {name}\ndata: {json.dumps(data)}\n\n"


def log_events(
    path: Path, *, lines: int = 300, max_s: float = 600.0, poll_s: float = POLL_S
) -> Iterator[str]:
    """Yield ``init`` with the current tail, then ``append`` events as the file grows.

    A file that shrinks (rotated or rewritten) sends a fresh ``init``. Ends with
    ``end`` after ``max_s`` seconds; the page reconnects if it still wants more.
    """
    text, truncated = tail_text(path, max_lines=lines)
    try:
        offset = path.stat().st_size
    except OSError:
        offset = 0
    yield _event("init", {"text": redact(text), "truncated": truncated, "size": offset})
    deadline = time.monotonic() + max_s
    while time.monotonic() < deadline:
        time.sleep(poll_s)
        try:
            size = path.stat().st_size
        except OSError:
            yield _event("end", {"reason": "file gone"})
            return
        if size < offset:
            text, truncated = tail_text(path, max_lines=lines)
            offset = size
            yield _event("init", {"text": redact(text), "truncated": truncated, "size": size})
            continue
        if size == offset:
            yield ": keep-alive\n\n"
            continue
        try:
            with path.open("rb") as fh:
                fh.seek(offset)
                chunk = fh.read(min(size - offset, MAX_CHUNK))
        except OSError:
            continue
        offset += len(chunk)
        yield _event(
            "append", {"text": redact(chunk.decode("utf-8", errors="replace")), "size": offset}
        )
    yield _event("end", {"reason": "time limit"})
