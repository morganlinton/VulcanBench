"""calendar-trip-timezones gold: Terminal asks Calendar and Reminders
(AppleScript) for the two flights and the check-out in the guest's Boston
time and two timed check-in reminders. The gold knows the conversions; an
agent has to read the bookings and work them out (or turn on Calendar's time
zone support).

Cheat variants: VB_DST=winter converts with winter offsets (Lisbon 5 hours
ahead, Munich 6); VB_NAIVE=1 types every local time as Boston time;
VB_CANCELLED=1 also adds the cancelled AX 216; VB_CHECKIN=day makes the
reminders due at departure instead of a day before.
"""

import os
import sys
from datetime import UTC, datetime, timedelta
from zoneinfo import ZoneInfo

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import goldlib as G  # noqa: E402

BOS = ZoneInfo("America/New_York")
dst = os.environ.get("VB_DST")
naive = os.environ.get("VB_NAIVE") == "1"
MUC = 1


def at(y: int, mo: int, d: int, h: int, mi: int, offset: float) -> tuple[int, int, int, int, int]:
    """Local time at a place `offset` hours from UTC, as Boston wall time."""
    if naive:
        return (y, mo, d, h, mi)
    t = datetime(y, mo, d, h, mi, tzinfo=UTC) - timedelta(hours=offset)
    if dst == "winter" and offset != -4:
        t -= timedelta(hours=1)  # treats Europe as one more hour ahead of Boston than it is
    b = t.astimezone(BOS)
    return (b.year, b.month, b.day, b.hour, b.minute)


EVENTS = [
    ("Flight AX 218 Boston to Lisbon", (2026, 3, 16, 21, 30), at(2026, 3, 17, 9, 5, 0)),
    ("Hotel check-out: Casa Alfama Suites", at(2026, 3, 20, 11, 0, 0), at(2026, 3, 20, 11, 30, 0)),
    ("Flight AW 1173 Lisbon to Munich", at(2026, 3, 20, 12, 40, 0), at(2026, 3, 20, 16, 25, MUC)),
]
if os.environ.get("VB_CANCELLED") == "1":
    EVENTS.append(("Flight AX 216 Boston to Lisbon", (2026, 3, 9, 18, 10), at(2026, 3, 10, 5, 45, 0)))
day = os.environ.get("VB_CHECKIN") == "day"
REMS = [
    ("Check in for AX 218", (2026, 3, 16, 21, 30) if day else (2026, 3, 15, 21, 30)),
    ("Check in for AW 1173", EVENTS[2][1] if day else at(2026, 3, 19, 12, 40, 0)),
]

G.start()
G.open_terminal()
G.authorize("Calendar", data_prompt=True)
for title, s, e in EVENTS:
    G.run(
        f"osascript -e 'tell application \"Calendar\" to make new event at end of events of calendar \"Travel\" "
        f"with properties {{summary:\"{title}\", start date:{G.as_date(*s)}, end date:{G.as_date(*e)}}}'",
        wait=3,
    )
G.authorize("Reminders", data_prompt=True)
for title, due in REMS:
    G.run(
        f"osascript -e 'tell application \"Reminders\" to make new reminder at end of list \"Travel\" "
        f"with properties {{name:\"{title}\", due date:{G.as_date(*due)}}}'",
        wait=3,
    )
G.run("osascript -e 'tell application \"Reminders\" to get name of reminders of list \"Travel\"'", wait=3)
G.shot("final")
print("gold done")
