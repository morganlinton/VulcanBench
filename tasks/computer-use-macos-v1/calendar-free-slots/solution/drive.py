"""calendar-free-slots gold: Terminal asks Calendar (AppleScript, goldlib)
to make the three meetings in their only free slots. The gold knows the
slots; an agent has to read the week.

Cheat variants: VB_WORKSHOP=standup starts the workshop at 9:30, over the
standup; VB_BUDGET=wed puts the review on the workshop's day (Wednesday 1 to
2, free but not a different day); VB_PRIYA=lunch puts the 1:1 at noon.
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import goldlib as G  # noqa: E402

ws = ((2026, 3, 18, 9, 30), (2026, 3, 18, 11, 0)) if os.environ.get("VB_WORKSHOP") == "standup" else (
    (2026, 3, 18, 9, 45), (2026, 3, 18, 11, 15))
bud = ((2026, 3, 18, 13, 0), (2026, 3, 18, 14, 0)) if os.environ.get("VB_BUDGET") == "wed" else (
    (2026, 3, 19, 14, 0), (2026, 3, 19, 15, 0))
pri = ((2026, 3, 20, 12, 0), (2026, 3, 20, 12, 30)) if os.environ.get("VB_PRIYA") == "lunch" else (
    (2026, 3, 20, 16, 30), (2026, 3, 20, 17, 0))

G.start()
G.open_terminal()
G.calendar_events("Work", [("Design workshop", *ws), ("Budget review", *bud), ("1:1 with Priya", *pri)])
G.shot("final")
print("gold done")
