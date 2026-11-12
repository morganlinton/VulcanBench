"""Helpers for the macOS suite's gold solutions (solution/drive.py), which run
in the agent container and drive the VM through the same VNC backend as the
agent's MCP tools, so the input log is written the same way.

Golds copy this file next to drive.py (solution/goldlib.py is a copy kept in
sync by tests/test_computer_use_macos.py) because harbor uploads only the
task's solution/ directory.

Terminal golds: some golds type commands into Terminal (AppleScript to an
app, or file moves) where the end state is the same as the app's own UI
would leave and the point of the gold is to prove the checker accepts a
correct end state. The first Apple Event to an app raises macOS's
Automation prompt and the first access to a protected folder raises a
privacy prompt; `allow_prompt` clicks Allow.
"""

from __future__ import annotations

import os
import sys
import time

os.environ.setdefault("VB_BACKEND", "vnc")
sys.argv = [sys.argv[0], "--backend", "vnc"]
sys.path.insert(0, "/opt/vbdesk")
from vb_computer_mcp import BACKEND as B  # noqa: E402

TERMINAL_DOCK = (1080, 762)


def shot(name: str) -> None:
    """Keep a screenshot of a step with the oracle's logs, for diagnosis."""
    try:
        with open(f"/logs/agent/gold-{name}.webp", "wb") as f:
            f.write(B.grab())
    except OSError:
        pass


def start() -> None:
    B.start(None)


def open_terminal() -> None:
    B.click(*TERMINAL_DOCK, "left", 1)
    time.sleep(4)
    B.key("cmd+n")  # a fresh window even if Terminal restored one
    time.sleep(2)


def run(command: str, wait: float = 3.0) -> None:
    """Type one shell command into the focused Terminal window and run it."""
    B.type_text(command)
    time.sleep(0.3)
    B.key("return")
    time.sleep(wait)


def allow_prompt(x: int, y: int, name: str = "prompt") -> None:
    """Click a privacy or Automation prompt's Allow button at (x, y)."""
    shot(name)
    B.click(x, y, "left", 1)
    time.sleep(2)


DAYS = ("Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday")
AUTOMATION_ALLOW = (698, 366)  # "Terminal wants access to control <app>": Allow
FOLDER_ALLOW = (698, 290)  # "Terminal would like to access files in ...": Allow
DATA_ALLOW = (698, 290)  # "Allow Terminal to add events to your calendar?" and the like


def as_date(y: int, mo: int, d: int, h: int, mi: int) -> str:
    """An AppleScript date literal in the guest's US English format."""
    import datetime as _dt  # noqa: PLC0415

    day = DAYS[_dt.date(y, mo, d).weekday()]
    month = _dt.date(y, mo, d).strftime("%B")
    ampm = "AM" if h < 12 else "PM"
    return f'date "{day}, {month} {d}, {y} at {h % 12 or 12}:{mi:02d}:00 {ampm}"'


def calendar_events(calendar: str, events: list[tuple[str, tuple, tuple]]) -> None:
    """Make one-off events through Terminal and AppleScript: (title,
    (y, mo, d, h, mi) start, (y, mo, d, h, mi) end). Allows the Automation
    prompt the first Apple Event to Calendar raises."""
    authorize("Calendar", data_prompt=True)
    for title, s, e in events:
        script = (
            f'tell application "Calendar" to make new event at end of events of calendar "{calendar}" '
            f'with properties {{summary:"{title}", start date:{as_date(*s)}, end date:{as_date(*e)}}}'
        )
        run(f"osascript -e '{script}'", wait=2.5)


def authorize(app: str, data_prompt: bool) -> None:
    """Raise and answer the prompts for Terminal controlling `app` with one
    short Apple Event, before the gold types anything long (a prompt that
    appears while a long command is still being typed is missed, and later
    keystrokes would go to it). Contacts, Calendar and Reminders ask a
    second time for access to their data."""
    probe = {
        "Contacts": "count people",
        "Notes": "count notes",
        "Calendar": "count calendars",
        "Reminders": "count lists",
    }.get(app, "count windows")  # touches the app's data
    run(f"osascript -e 'tell application \"{app}\" to {probe}'", wait=6)
    # An app's first launch in a trial can take most of a minute before its
    # prompt appears. Click Allow's spot every few seconds for 40 s: with no
    # prompt up the click lands harmlessly in the Terminal window.
    for k in range(10):
        allow_prompt(
            *AUTOMATION_ALLOW, name=f"automation-{app}-{k}" if k < 2 else "automation-later"
        )
        time.sleep(2)
    if data_prompt:
        for k in range(4):
            allow_prompt(*DATA_ALLOW, name=f"data-{app}-{k}" if k < 2 else "data-later")
            time.sleep(2)


def allow_folder(name: str = "folder-prompt", seconds: float = 14.0) -> None:
    """Answer a folder privacy prompt ("Terminal would like to access files
    in your Documents folder") that may appear several seconds after the
    command asked for the folder: click Allow's spot every 2 s for `seconds`.
    With no prompt up the click lands in the Terminal window."""
    end = time.time() + seconds
    k = 0
    while time.time() < end:
        allow_prompt(*FOLDER_ALLOW, name=f"{name}-{k}" if k < 2 else f"{name}-later")
        k += 1
