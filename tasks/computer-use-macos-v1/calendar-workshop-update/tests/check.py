#!/usr/bin/env python3
"""calendar-workshop-update checker: the Calendar store and the plan folder
copied from the stopped VM. The guest's time zone is America/New_York.

    check.py /var/lib/vbmac

The corrected plan (the correction appears 120 s after TextEdit or Calendar first runs):
  Onboarding: session 1  Tue 17 March 10:00 to 11:30
  Onboarding: session 2  Thu 19 March 10:00 to 11:30
  Onboarding: session 3  Wed 25 March 10:00 to 11:30   (moved from Tue 24)
  Onboarding: session 4  Thu 26 March 10:00 to 11:30
An event counts as a session when its title mentions onboarding (or the
workshop) and a session number.

Sub-metrics (reward is 1 only when all are 1):
  artifact_present  the Calendar store was collected
  four_sessions     exactly four session events, numbered 1 to 4 once each, on Work
  corrected_times   every session runs 10:00 to 11:30 on its day
  session3_moved    session 3 is on Wednesday 25 March
  others_untouched  the three seeded events are unchanged
  ui_path           the MCP server logged pointer input
Extra: correction_present (the correction file was in the folder at the end).
"""

from __future__ import annotations

import re
import sys
from datetime import date, datetime
from pathlib import Path

sys.path.insert(0, "/opt/vbdesk")
import cu_check as cu  # noqa: E402
import mac_state as M  # noqa: E402

METRICS = (
    "artifact_present",
    "four_sessions",
    "corrected_times",
    "session3_moved",
    "others_untouched",
    "ui_path",
)
DAYS = {1: date(2026, 3, 17), 2: date(2026, 3, 19), 3: date(2026, 3, 25), 4: date(2026, 3, 26)}
SEEDED = {
    "All-hands": ("Work", (2026, 3, 24, 9, 0), (2026, 3, 24, 12, 0)),
    "Hiring sync": ("Work", (2026, 3, 18, 14, 0), (2026, 3, 18, 14, 30)),
    "Dentist": ("Personal", (2026, 3, 19, 16, 0), (2026, 3, 19, 17, 0)),
}
SESSION = re.compile(r"(onboarding|workshop).*?session\s*(\d)|session\s*(\d).*?(onboarding|workshop)", re.I)
CORRECTION = "Users/Shared/Workshop/Workshop plan - correction from Priya.txt"


def local(t: tuple[int, int, int, int, int]) -> float:
    return datetime(*t, tzinfo=M.GUEST_TZ).timestamp()


def near(a: float | None, b: float) -> bool:
    return a is not None and abs(a - b) <= 60


def main(argv: list[str]) -> int:
    root = Path(argv[1])
    export = root / "export"
    fs = export / "fs"
    m = {k: 0 for k in METRICS}
    extra: dict[str, object] = {"shutdown": M.manifest(export).get("shutdown", "none")}
    extra["correction_present"] = int((fs / CORRECTION).is_file())
    if not (fs / M.CALENDAR_DB).is_file():
        cu.emit(m, METRICS, **extra)
        return 0
    m["artifact_present"] = 1
    evs = M.events(fs)
    sessions: dict[int, list[M.Event]] = {}
    for e in evs:
        hit = SESSION.search(e.summary)
        if hit:
            sessions.setdefault(int(hit.group(2) or hit.group(3)), []).append(e)
    extra["sessions"] = {
        n: [(e.calendar, str(e.start_local), str(e.local(e.end))) for e in v] for n, v in sorted(sessions.items())
    }
    count = sum(len(v) for v in sessions.values())
    one = {n: v[0] for n, v in sessions.items() if len(v) == 1}
    m["four_sessions"] = int(count == 4 and set(one) == {1, 2, 3, 4} and all(e.calendar == "Work" for e in one.values()))
    m["corrected_times"] = int(
        m["four_sessions"] == 1
        and all(
            near(one[n].start, local((d.year, d.month, d.day, 10, 0))) and near(one[n].end, local((d.year, d.month, d.day, 11, 30)))
            for n, d in DAYS.items()
        )
    )
    s3 = one.get(3)
    m["session3_moved"] = int(s3 is not None and s3.start_local is not None and s3.start_local.date() == DAYS[3])
    ok = True
    for name, (cal, s, e) in SEEDED.items():
        hits = [ev for ev in evs if ev.summary == name]
        ok = ok and len(hits) == 1 and hits[0].calendar == cal and near(hits[0].start, local(s)) and near(hits[0].end, local(e))
    m["others_untouched"] = int(ok)
    events_log = M.input_events(root / "input.log")
    m["ui_path"] = int(bool({e.get("kind") for e in events_log} & {"click", "drag"}))
    extra["input_events"] = len(events_log)
    cu.emit(m, METRICS, **extra)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
