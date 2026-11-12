#!/usr/bin/env python3
"""calendar-offsite-conflicts checker: the Calendar store copied from the
stopped VM. Today in the guest is Monday 9 March 2026; the offsite is
Tuesday 17 and Wednesday 18 March.

    check.py /var/lib/vbmac

Each 1:1 series is expanded week by week from its master (weekly, from its
first date), with occurrences removed (exception dates) or moved (detached
rows), plus any stand-alone event with the same title. Either way of moving a
single meeting counts: editing that occurrence, or removing it and adding a
one-off at the new time.

Sub-metrics (reward is 1 only when all are 1):
  artifact_present  the Calendar store was collected
  alex_moved        the week of 16 March has exactly one "1:1 with Alex", on
                    Thursday 19 March 10:00 to 10:30
  priya_moved       likewise "1:1 with Priya", Thursday 19 March 14:00 to 14:30
  series_intact     both series keep their rule and first date, and every
                    other week from 3 March to 28 April still has its meeting
                    at the usual day and time
  others_untouched  the Sam series, the standup, the offsite, the planning
                    review and the Personal dentist visit are unchanged
  ui_path           the MCP server logged pointer input
"""

from __future__ import annotations

import sys
from datetime import datetime, timedelta
from pathlib import Path

sys.path.insert(0, "/opt/vbdesk")
import cu_check as cu  # noqa: E402
import mac_state as M  # noqa: E402

METRICS = ("artifact_present", "alex_moved", "priya_moved", "series_intact", "others_untouched", "ui_path")
TZ = M.GUEST_TZ
WEEK_OF = datetime(2026, 3, 16, tzinfo=TZ)
HORIZON = datetime(2026, 4, 29, tzinfo=TZ)


def local(t: float | None) -> datetime | None:
    return datetime.fromtimestamp(t, TZ) if t is not None else None


def occurrences(evs: list[M.Event], title: str, first: datetime) -> tuple[list[tuple[datetime, datetime]], bool]:
    """All meetings titled `title` up to HORIZON, and whether the series' own
    definition (rule and first date) is intact."""
    masters = [e for e in evs if e.summary == title and e.orig_item == 0 and e.recurring]
    out: list[tuple[datetime, datetime]] = []
    intact = len(masters) == 1
    for mst in masters:
        start, end = local(mst.start), local(mst.end)
        if start is None or end is None:
            continue
        rule = mst.rule or (0, 0, "", None, 0)
        intact = intact and start == first and rule[0] == 2 and rule[1] == 1 and rule[3] is None and rule[4] == 0
        dur = end - start
        detached = {
            round(e.orig_start): e for e in evs if e.orig_item == mst.rowid and e.orig_start is not None
        }
        removed = {round(x) for x in mst.exceptions}
        t = start
        while t < HORIZON:
            key = round(t.timestamp())
            if key in detached:
                d = detached[key]
                ds, de = local(d.start), local(d.end)
                if ds and de:
                    out.append((ds, de))
            elif key not in removed:
                out.append((t, t + dur))
            t += timedelta(days=7)
    for e in evs:
        if e.summary == title and e.orig_item == 0 and not e.recurring:
            es, ee = local(e.start), local(e.end)
            if es and ee:
                out.append((es, ee))
    return sorted(out), intact


def week(occ: list[tuple[datetime, datetime]], monday: datetime) -> list[tuple[datetime, datetime]]:
    return [o for o in occ if monday <= o[0] < monday + timedelta(days=7)]


def at(day: int, h: int, mi: int) -> datetime:
    return datetime(2026, 3, day, h, mi, tzinfo=TZ)


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
    evs = [e for e in M.events(fs) if e.calendar in ("Work", "Personal")]
    series_ok = True
    for name, first, key, target in (
        ("1:1 with Alex", at(3, 10, 0), "alex_moved", (at(19, 10, 0), at(19, 10, 30))),
        ("1:1 with Priya", at(4, 14, 0), "priya_moved", (at(19, 14, 0), at(19, 14, 30))),
    ):
        occ, intact = occurrences(evs, name, first)
        wk = week(occ, WEEK_OF)
        extra[key + "_week"] = [(a.isoformat(), b.isoformat()) for a, b in wk]
        m[key] = int(wk == [target])
        expected_other_weeks = [
            (first + timedelta(days=7 * i), first + timedelta(days=7 * i, minutes=30))
            for i in range(0, 9)
            if first + timedelta(days=7 * i) < HORIZON
            and not (WEEK_OF <= first + timedelta(days=7 * i) < WEEK_OF + timedelta(days=7))
        ]
        others = [o for o in occ if not (WEEK_OF <= o[0] < WEEK_OF + timedelta(days=7))]
        series_ok = series_ok and intact and others == expected_other_weeks
    m["series_intact"] = int(series_ok)
    sam, sam_ok = occurrences(evs, "1:1 with Sam", at(5, 9, 0))
    stand = [e for e in evs if e.summary == "Team standup" and e.orig_item == 0]
    detached_stand = [e for e in evs if stand and e.orig_item == stand[0].rowid]
    one_off = {
        e.summary: (local(e.start), local(e.end))
        for e in evs
        if e.orig_item == 0 and not e.recurring
    }
    m["others_untouched"] = int(
        sam_ok
        and week(sam, WEEK_OF) == [(at(19, 9, 0), at(19, 9, 30))]
        and len(stand) == 1
        and not stand[0].exceptions
        and not detached_stand
        and "Leadership offsite" in one_off
        and one_off.get("Quarterly planning review") == (at(19, 15, 0), at(19, 16, 0))
        and one_off.get("Dentist") == (at(18, 8, 0), at(18, 8, 45))
    )
    events_log = M.input_events(root / "input.log")
    kinds = {e.get("kind") for e in events_log}
    m["ui_path"] = int(bool(kinds & {"click", "drag"}))
    extra["input_events"] = len(events_log)
    cu.emit(m, METRICS, **extra)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
