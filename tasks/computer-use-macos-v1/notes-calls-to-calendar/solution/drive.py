"""notes-calls-to-calendar gold: Terminal asks Calendar (AppleScript,
goldlib.calendar_events) to make the five events on the Work calendar at
their New York times. The gold knows the conversions; an agent has to read
the note and convert.

Cheat variants: VB_LONDON_HOUR=9 puts Oliver's call at 9:00 (five hours);
VB_ANA_DAY=10 keeps Ana's sync on Tuesday; VB_ADD_LENA=1 adds the tentative
call too.
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import goldlib as G  # noqa: E402

lh = int(os.environ.get("VB_LONDON_HOUR", "10"))
ana = int(os.environ.get("VB_ANA_DAY", "12"))
EVENTS = [
    ("Intro call with Oliver Grant", (2026, 3, 11, lh, 0), (2026, 3, 11, lh, 30)),
    ("Budget sync with Ana Ribeiro", (2026, 3, ana, 12, 0), (2026, 3, ana, 12, 45)),
    ("Design review with Kenji Watanabe", (2026, 3, 12, 20, 0), (2026, 3, 12, 21, 0)),
    ("Vendor demo with Priya Nair", (2026, 3, 11, 11, 0), (2026, 3, 11, 12, 0)),
    ("Catch-up with Marcus Reed", (2026, 3, 16, 12, 0), (2026, 3, 16, 12, 30)),
]
if os.environ.get("VB_ADD_LENA") == "1":
    EVENTS.append(("Quarterly check-in with Lena Vogel", (2026, 3, 19, 6, 0), (2026, 3, 19, 6, 30)))

G.start()
G.open_terminal()
G.calendar_events("Work", EVENTS)
G.run("osascript -e 'tell application \"Calendar\" to count events of calendar \"Work\"'", wait=3)
G.shot("final")
print("gold done")
