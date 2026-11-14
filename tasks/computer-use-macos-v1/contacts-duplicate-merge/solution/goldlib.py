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

# Golds wait fixed times for the guest to react. With a sweep running in the
# other VM the guest is slower and those waits were too short (a Settings
# pane not yet open when the gold clicked into it), so every sleep in a gold,
# drive.py's own included, is stretched by VB_GOLD_PACE (default 1.8).
_PACE = float(os.environ.get("VB_GOLD_PACE", "1.8"))
_sleep = time.sleep


def _paced_sleep(seconds: float) -> None:
    _sleep(seconds * _PACE)


time.sleep = _paced_sleep  # the time module is shared, so drive.py's sleeps stretch too


def shot(name: str) -> None:
    """Keep a screenshot of a step with the oracle's logs, for diagnosis."""
    try:
        with open(f"/logs/agent/gold-{name}.webp", "wb") as f:
            f.write(B.grab())
    except OSError:
        pass


def start() -> None:
    B.start(None)


def reveal_dock() -> None:
    """Bring up an auto-hidden Dock (settings-it-checklist seeds auto-hide on):
    touch the screen's bottom edge, then rest on the Dock row."""
    B.move(TERMINAL_DOCK[0], 799)
    time.sleep(1.5)
    B.move(*TERMINAL_DOCK)
    time.sleep(0.5)


def open_terminal() -> None:
    reveal_dock()
    B.click(*TERMINAL_DOCK, "left", 1)
    time.sleep(6)
    # Click again once it is running: a slow first launch left Finder in
    # front, and the Command-N meant for Terminal opened a Finder window.
    reveal_dock()
    B.click(*TERMINAL_DOCK, "left", 1)
    time.sleep(3)
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
# The Automation prompt's body wraps by the app's name, which moves Allow:
# Contacts' five-line prompt has it at y=366, Notes' four-line one at 347.
AUTOMATION_ALLOW_SHORT = (698, 347)
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


def authorize(app: str, data_prompt: bool, seconds: float = 150.0) -> None:
    """Raise and answer the prompts for Terminal controlling `app` before the
    gold types anything long (a prompt that appears while a long command is
    being typed is missed, and later keystrokes would go to it). Contacts,
    Calendar and Reminders ask a second time, for their data.

    An app's first launch in a trial can be slow enough that the first Apple
    Event times out with no prompt shown, so Terminal retries a short,
    data-touching command until it succeeds, while the gold clicks the
    Automation prompt's two Allow spots and the data prompt's in turn for
    `seconds`. With no prompt up a click lands harmlessly in the Terminal
    window."""
    probe = {
        "Contacts": "count people",
        "Notes": "count notes",
        "Calendar": "count calendars",
        "Reminders": "count lists",
    }.get(app, "count windows")
    run(
        f"until osascript -e 'tell application \"{app}\" to {probe}' >/dev/null 2>&1; "
        f"do sleep 2; done; echo authorized-{app}",
        wait=4,
    )
    spots = [AUTOMATION_ALLOW, AUTOMATION_ALLOW_SHORT] + ([DATA_ALLOW] if data_prompt else [])
    end = time.time() + seconds
    k = 0
    while time.time() < end:
        spot = spots[k % len(spots)]
        allow_prompt(*spot, name=f"authorize-{app}-{k}" if k < 3 else "authorize-later")
        k += 1
    time.sleep(2)


def allow_folder(name: str = "folder-prompt", seconds: float = 45.0) -> None:
    """Answer a folder privacy prompt ("Terminal would like to access files
    in your Documents folder") that may appear several seconds after the
    command asked for the folder: click Allow's spot every 2 s for `seconds`.
    With no prompt up the click lands in the Terminal window."""
    end = time.time() + seconds
    k = 0
    while time.time() < end:
        allow_prompt(*FOLDER_ALLOW, name=f"{name}-{k}" if k < 2 else f"{name}-later")
        k += 1
