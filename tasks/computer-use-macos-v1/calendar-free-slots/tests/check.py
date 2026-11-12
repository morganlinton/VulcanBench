#!/usr/bin/env python3
"""calendar-free-slots checker: the Calendar store copied from the stopped
VM. Next week is Monday 16 to Friday 20 March 2026 (New York time).

    check.py /var/lib/vbmac

The seeded week leaves exactly one slot per meeting (see environment/seed.sh):
  Design workshop  Wed 18 March  9:45 to 11:15
  Budget review    Thu 19 March 14:00 to 15:00
  1:1 with Priya   Fri 20 March 16:30 to 17:00
A title matches when it contains the meeting's name, ignoring case.

Sub-metrics (reward is 1 only when all are 1):
  artifact_present  the Calendar store was collected
  workshop_slot     exactly one Design workshop, at its slot
  budget_slot       exactly one Budget review, at its slot
  priya_slot        exactly one 1:1 with Priya, at its slot
  on_work_calendar  all three are on the Work calendar, not recurring
  others_untouched  every seeded event (and the standup series) is unchanged
                    and no other event was added
  ui_path           the MCP server logged pointer input
"""

from __future__ import annotations

import sys
from datetime import datetime
from pathlib import Path

sys.path.insert(0, "/opt/vbdesk")
import cu_check as cu  # noqa: E402
import mac_state as M  # noqa: E402

METRICS = (
    "artifact_present",
    "workshop_slot",
    "budget_slot",
    "priya_slot",
    "on_work_calendar",
    "others_untouched",
    "ui_path",
)
TZ = M.GUEST_TZ


def at(day: int, h: int, mi: int = 0) -> datetime:
    return datetime(2026, 3, day, h, mi, tzinfo=TZ)


WANT = {
    "workshop_slot": ("design workshop", at(18, 9, 45), at(18, 11, 15)),
    "budget_slot": ("budget review", at(19, 14), at(19, 15)),
    "priya_slot": ("1:1 with priya", at(20, 16, 30), at(20, 17)),
}
SEEDED = {
    ("Work", "Quarterly kickoff"): (at(16, 10), at(16, 11, 30)),
    ("Work", "Board prep"): (at(16, 13), at(16, 15)),
    ("Work", "Customer visit: Northwind"): (at(16, 15, 30), at(16, 17)),
    ("Work", "Vendor call"): (at(17, 10, 30), at(17, 11)),
    ("Work", "Offsite planning"): (at(17, 13, 30), at(17, 16, 30)),
    ("Personal", "School pickup"): (at(17, 16, 30), at(17, 17)),
    ("Work", "Interview: backend candidate"): (at(18, 11, 15), at(18, 12)),
    ("Work", "Security review"): (at(18, 14), at(18, 16)),
    ("Work", "Roadmap review"): (at(19, 10), at(19, 11)),
    ("Work", "Design sync"): (at(19, 13), at(19, 14)),
    ("Work", "Hiring panel"): (at(19, 15), at(19, 17)),
    ("Personal", "Breakfast with Sam"): (at(20, 8, 30), at(20, 9, 30)),
    ("Work", "Quarterly planning"): (at(20, 9, 45), at(20, 12)),
    ("Work", "Customer workshop"): (at(20, 13), at(20, 16)),
    ("Personal", "Pharmacy pickup"): (at(20, 16), at(20, 16, 30)),
    ("Personal", "Dinner with Alex"): (at(19, 19), at(19, 21)),
}


def local(t: float | None) -> datetime | None:
    return datetime.fromtimestamp(t, TZ) if t is not None else None


def main(argv: list[str]) -> int:
    root = Path(argv[1])
    export = root / "export"
    fs = export / "fs"
    m = {k: 0 for k in METRICS}
    extra: dict[str, object] = {"shutdown": M.manifest(export).get("shutdown", "none")}
    if not (fs / M.CALENDAR_DB).is_file():
        cu.emit(m, METRICS, **extra)
        return 0
    m["artifact_present"] = 1
    # The store also holds macOS's US Holidays calendar; only the owner's two count.
    evs = [e for e in M.events(fs) if e.calendar in ("Work", "Personal")]
    claimed: set[int] = set()
    on_work = True
    seen = {}
    for key, (name, start, end) in WANT.items():
        hits = [e for e in evs if name in " ".join(e.summary.lower().split())]
        claimed |= {e.rowid for e in hits}
        seen[key] = [f"{e.calendar}|{e.summary}|{local(e.start)}|{local(e.end)}" for e in hits]
        m[key] = int(len(hits) == 1 and local(hits[0].start) == start and local(hits[0].end) == end)
        on_work = on_work and len(hits) == 1 and hits[0].calendar == "Work" and not hits[0].recurring
    extra["placed"] = seen
    m["on_work_calendar"] = int(on_work)
    ok = True
    for (cal, title), (start, end) in SEEDED.items():
        hits = [e for e in evs if e.summary == title]
        claimed |= {e.rowid for e in hits}
        ok = ok and len(hits) == 1 and hits[0].calendar == cal
        ok = ok and local(hits[0].start) == start and local(hits[0].end) == end
    standup = [e for e in evs if e.summary == "Team standup"]
    claimed |= {e.rowid for e in standup}
    ok = ok and len(standup) == 1 and standup[0].recurring and not standup[0].exceptions
    stray = [f"{e.calendar}|{e.summary}" for e in evs if e.rowid not in claimed]
    extra["other_events"] = stray
    m["others_untouched"] = int(ok and not stray)
    events_log = M.input_events(root / "input.log")
    m["ui_path"] = int(bool({e.get("kind") for e in events_log} & {"click", "drag"}))
    extra["input_events"] = len(events_log)
    cu.emit(m, METRICS, **extra)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
