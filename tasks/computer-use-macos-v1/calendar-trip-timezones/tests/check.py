#!/usr/bin/env python3
"""calendar-trip-timezones checker: the Calendar and Reminders stores copied
from the stopped VM. The guest's time zone is America/New_York.

    check.py /var/lib/vbmac

Targets are absolute instants (UTC); an event or reminder matches within a
minute. In the trip week Boston is UTC-4 (daylight time since 8 March),
Lisbon UTC+0 and Munich UTC+1 (Europe changes on 29 March).
  AX 218   2026-03-17 01:30Z to 09:05Z    (Boston 21:30 Mon to Lisbon 09:05 Tue)
  check-out Casa Alfama  2026-03-20 11:00Z, as the event's start or end
  AW 1173  2026-03-20 12:40Z to 15:25Z    (Lisbon 12:40 to Munich 16:25)
  check-in reminders  due 2026-03-16 01:30Z (AX 218) and 2026-03-19 12:40Z (AW 1173)

Sub-metrics (reward is 1 only when all are 1):
  artifact_present   both stores were collected
  overnight_flight   one Travel event titled with AX 218 at the right start and end
  connecting_flight  one Travel event titled with AW 1173 at the right start and end
  hotel_checkout     a Travel event about the check-out starts or ends at 11:00 Lisbon
  checkin_reminders  one timed reminder per flight in the Travel list (title with
                     its flight number), due 24 hours before departure
  skipped_cancelled  nothing for the cancelled AX 216 and no new event on 9 or 10 March
  others_untouched   the three seeded events are unchanged and the adapter reminder is there
  ui_path            the MCP server logged pointer input
"""

from __future__ import annotations

import re
import sys
from datetime import UTC, datetime
from pathlib import Path

sys.path.insert(0, "/opt/vbdesk")
import cu_check as cu  # noqa: E402
import mac_state as M  # noqa: E402

METRICS = (
    "artifact_present",
    "overnight_flight",
    "connecting_flight",
    "hotel_checkout",
    "checkin_reminders",
    "skipped_cancelled",
    "others_untouched",
    "ui_path",
)


def z(y: int, mo: int, d: int, h: int, mi: int) -> float:
    return datetime(y, mo, d, h, mi, tzinfo=UTC).timestamp()


FLIGHTS = {
    "overnight_flight": (re.compile(r"AX\s*218", re.I), z(2026, 3, 17, 1, 30), z(2026, 3, 17, 9, 5)),
    "connecting_flight": (re.compile(r"AW\s*1173", re.I), z(2026, 3, 20, 12, 40), z(2026, 3, 20, 15, 25)),
}
CHECKOUT = z(2026, 3, 20, 11, 0)
CHECKINS = {re.compile(r"AX\s*218", re.I): z(2026, 3, 16, 1, 30), re.compile(r"AW\s*1173", re.I): z(2026, 3, 19, 12, 40)}
SEEDED = {
    "Quarterly kickoff": ("Work", (2026, 3, 16, 10, 0), (2026, 3, 16, 11, 30)),
    "Remote: Lisbon team sync": ("Work", (2026, 3, 18, 9, 0), (2026, 3, 18, 9, 30)),
    "Passport photo appointment": ("Travel", (2026, 3, 12, 16, 0), (2026, 3, 12, 16, 30)),
}


def near(a: float | None, b: float, tol: float = 60) -> bool:
    return a is not None and abs(a - b) <= tol


def local(t: tuple[int, int, int, int, int]) -> float:
    return datetime(*t, tzinfo=M.GUEST_TZ).timestamp()


def main(argv: list[str]) -> int:
    root = Path(argv[1])
    export = root / "export"
    fs = export / "fs"
    m = {k: 0 for k in METRICS}
    extra: dict[str, object] = {"shutdown": M.manifest(export).get("shutdown", "none")}
    if not (fs / M.CALENDAR_DB).is_file() or not (fs / M.REMINDERS_STORES).is_dir():
        cu.emit(m, METRICS, **extra)
        return 0
    m["artifact_present"] = 1
    evs = M.events(fs)
    travel = [e for e in evs if e.calendar == "Travel"]
    extra["travel"] = [
        (e.summary, datetime.fromtimestamp(e.start or 0, UTC).isoformat(), datetime.fromtimestamp(e.end or 0, UTC).isoformat())
        for e in travel
    ]
    for key, (pat, s, e) in FLIGHTS.items():
        hits = [ev for ev in travel if pat.search(ev.summary)]
        m[key] = int(len(hits) == 1 and near(hits[0].start, s) and near(hits[0].end, e) and not hits[0].all_day)
    outs = [ev for ev in travel if re.search(r"check.?out|alfama|hotel", ev.summary, re.I)]
    m["hotel_checkout"] = int(len(outs) >= 1 and any(near(ev.start, CHECKOUT) or near(ev.end, CHECKOUT) for ev in outs))
    rems = [r for r in M.reminders(fs) if not r.deleted and r.list == "Travel"]
    extra["reminders"] = [(r.title, r.due, r.all_day) for r in rems]
    ok = True
    for pat, due in CHECKINS.items():
        hits = [r for r in rems if pat.search(r.title)]
        ok = ok and len(hits) == 1 and not hits[0].all_day and near(hits[0].due, due)
    m["checkin_reminders"] = int(ok)
    day9 = datetime(2026, 3, 9, tzinfo=M.GUEST_TZ).timestamp()
    day11 = datetime(2026, 3, 11, tzinfo=M.GUEST_TZ).timestamp()
    seeded_names = set(SEEDED)
    m["skipped_cancelled"] = int(
        not any(re.search(r"AX\s*216", ev.summary, re.I) for ev in evs)
        and not any(re.search(r"AX\s*216", r.title, re.I) for r in rems)
        and not any(ev.summary not in seeded_names and ev.start is not None and day9 <= ev.start < day11 for ev in evs)
    )
    ok = True
    for name, (cal, s, e) in SEEDED.items():
        hits = [ev for ev in evs if ev.summary == name]
        ok = ok and len(hits) == 1 and hits[0].calendar == cal and near(hits[0].start, local(s)) and near(hits[0].end, local(e))
    ok = ok and any(r.title == "Buy a travel adapter" for r in rems)
    m["others_untouched"] = int(ok)
    events_log = M.input_events(root / "input.log")
    m["ui_path"] = int(bool({e.get("kind") for e in events_log} & {"click", "drag"}))
    extra["input_events"] = len(events_log)
    cu.emit(m, METRICS, **extra)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
