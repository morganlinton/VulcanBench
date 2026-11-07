#!/usr/bin/env python3
"""Record raw X input events on the task display as JSON lines.

Started by ensure-display.sh once the window manager is up, so it runs under
computer-1, under the MCP computer tools and under the oracle alike. Every
RawKeyPress and RawButtonPress the X server delivers (xdotool's XTEST input
included) becomes one line in /var/lib/vbdesk/xinput.log:

    {"ts": 1762500000.123, "type": "RawButtonPress", "detail": 1}

This is the UI-path evidence for apps without their own audit log (family B):
a cheat that rewrites files directly produces no input events in the window
before the files changed. RawMotion and release events are dropped to keep the
log small. Timestamps are the container's wall clock, the same clock file
mtimes use.
"""

from __future__ import annotations

import contextlib
import json
import os
import subprocess
import sys
import time

LOG = os.environ.get("VB_XINPUT_LOG", "/var/lib/vbdesk/xinput.log")
KEEP = {"RawKeyPress", "RawButtonPress"}


def main() -> int:
    os.makedirs(os.path.dirname(LOG), exist_ok=True)
    env = dict(os.environ, DISPLAY=os.environ.get("DISPLAY", ":1"))
    # stdbuf: xinput fully buffers stdout when it is a pipe, which would shift
    # every timestamp to the moment the buffer flushed.
    proc = subprocess.Popen(
        ["stdbuf", "-oL", "xinput", "test-xi2", "--root"],
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        env=env,
        text=True,
        bufsize=1,
    )
    assert proc.stdout is not None
    cur: dict | None = None
    with open(LOG, "a", buffering=1) as out:
        out.write(json.dumps({"ts": round(time.time(), 3), "type": "recorder.start"}) + "\n")
        for raw in proc.stdout:
            line = raw.strip()
            if line.startswith("EVENT type"):
                if cur is not None and cur["type"] in KEEP:
                    out.write(json.dumps(cur) + "\n")
                name = line[line.find("(") + 1 : line.rfind(")")]
                cur = {"ts": round(time.time(), 3), "type": name}
            elif cur is not None and line.startswith("detail:"):
                with contextlib.suppress(ValueError):
                    cur["detail"] = int(line.split(":", 1)[1])
        if cur is not None and cur["type"] in KEEP:
            out.write(json.dumps(cur) + "\n")
        out.write(json.dumps({"ts": round(time.time(), 3), "type": "recorder.stop"}) + "\n")
    return proc.wait()


if __name__ == "__main__":
    sys.exit(main())
