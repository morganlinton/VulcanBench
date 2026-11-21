#!/usr/bin/env python3
"""notes-calls-to-calendar checker: the Calendar store copied from the
stopped VM. Today in the guest is Monday 9 March 2026, New York (EDT, UTC-4
since 8 March); London and Lisbon are on UTC+0 until 29 March, Tokyo UTC+9,
India UTC+5:30, San Francisco PDT (UTC-7).

    check.py /var/lib/vbmac

Expected Work events, New York time (a title matches when it contains the
call's name as Maya wrote it, ignoring case):
  Intro call with Oliver Grant      Wed 11 Mar 10:00 to 10:30 (14:00 London)
  Budget sync with Ana Ribeiro      Thu 12 Mar 12:00 to 12:45 (moved from Tue)
  Design review with Kenji Watanabe Thu 12 Mar 20:00 to 21:00 (Fri 09:00 Tokyo)
  Vendor demo with Priya Nair       Wed 11 Mar 11:00 to 12:00 (20:30 India)
  Catch-up with Marcus Reed         Mon 16 Mar 12:00 to 12:30 (09:00 Pacific)
and nothing for Lena Vogel (tentative).

Sub-metrics (reward is 1 only when all are 1):
  artifact_present   the Calendar store was collected
  london_dst         Oliver's call: exactly one event, at the time above
                     (an agent that assumes five hours puts it at 9:00)
  lisbon_update      Ana's call: exactly one event, on Thursday (not Tuesday)
  tokyo_date         Kenji's call: exactly one event, Thursday evening
  india_half_hour    Priya's demo: exactly one event, at the time above
  pacific            Marcus's call: exactly one event, at the time above
  tentative_skipped  no event mentions Lena Vogel
  on_work_calendar   all five are on the Work calendar
  others_untouched   the standup series, the roadmap and planning reviews and
                     the Personal lunch are unchanged, and no other event exists
  ui_path            the MCP server logged pointer input
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
    "london_dst",
    "lisbon_update",
    "tokyo_date",
    "india_half_hour",
    "pacific",
    "tentative_skipped",
    "on_work_calendar",
    "others_untouched",
    "ui_path",
)
TZ = M.GUEST_TZ


def at(day: int, h: int, mi: int = 0) -> datetime:
    return datetime(2026, 3, day, h, mi, tzinfo=TZ)


CALLS = {
    "london_dst": ("intro call with oliver grant", at(11, 10), at(11, 10, 30)),
    "lisbon_update": ("budget sync with ana ribeiro", at(12, 12), at(12, 12, 45)),
    "tokyo_date": ("design review with kenji watanabe", at(12, 20), at(12, 21)),
    "india_half_hour": ("vendor demo with priya nair", at(11, 11), at(11, 12)),
    "pacific": ("catch-up with marcus reed", at(16, 12), at(16, 12, 30)),
}
SEEDED = {
    "Roadmap review": ("Work", at(10, 14), at(10, 15)),
    "Quarterly planning review": ("Work", at(19, 15), at(19, 16)),
    "Lunch with Sofia": ("Personal", at(13, 12, 30), at(13, 13, 30)),
}


def norm(s: str) -> str:
    return " ".join(s.lower().replace("‑", "-").replace("‐", "-").split())


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
    seen: dict[str, list[str]] = {}
    on_work = True
    claimed: set[int] = set()
    for key, (title, start, end) in CALLS.items():
        hits = [e for e in evs if title in norm(e.summary)]
        seen[key] = [f"{e.calendar}|{e.summary}|{local(e.start)}|{local(e.end)}" for e in hits]
        claimed |= {e.rowid for e in hits}
        ok = len(hits) == 1 and local(hits[0].start) == start and local(hits[0].end) == end
        ok = ok and not hits[0].recurring and not hits[0].all_day
        m[key] = int(ok)
        on_work = on_work and len(hits) == 1 and hits[0].calendar == "Work"
    extra["calls"] = seen
    m["on_work_calendar"] = int(on_work)
    lena = [e for e in evs if "lena" in norm(e.summary) or "vogel" in norm(e.summary)]
    m["tentative_skipped"] = int(not lena)
    claimed |= {e.rowid for e in lena}
    standup = [e for e in evs if e.summary == "Team standup"]
    ok = (
        len(standup) == 1
        and standup[0].calendar == "Work"
        and standup[0].recurring
        and not standup[0].exceptions
        and local(standup[0].start) == datetime(2026, 3, 2, 9, 30, tzinfo=TZ)
    )
    claimed |= {e.rowid for e in standup}
    for name, (cal, start, end) in SEEDED.items():
        hits = [e for e in evs if e.summary == name]
        claimed |= {e.rowid for e in hits}
        ok = ok and len(hits) == 1 and hits[0].calendar == cal
        ok = ok and local(hits[0].start) == start and local(hits[0].end) == end
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
