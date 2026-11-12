#!/usr/bin/env python3
"""reminders-renovation-plan checker: the Reminders store copied from the
stopped VM.

    check.py /var/lib/vbmac

Reminders are found by title (live ones only); priority 1 is high.

Sub-metrics (reward is 1 only when all are 1):
  artifact_present  the Reminders store was collected
  moved_to_list     Order cabinets, Order countertop and Order sink and faucet
                    are in a list named Ordering, and nothing else is
  due_dates         Choose a contractor is due Friday 13 March and Book
                    electrician for new outlets Tuesday 17 March
  priority_high     Get three contractor quotes has high priority
  paint_done        Pick paint color is completed (not deleted)
  others_untouched  every other reminder has no due date, no priority
                    and is not completed; all nine are still there once,
                    the six others in Kitchen renovation; Personal is unchanged
  ui_path           the MCP server logged pointer input
"""

from __future__ import annotations

import sys
from datetime import date
from pathlib import Path

sys.path.insert(0, "/opt/vbdesk")
import cu_check as cu  # noqa: E402
import mac_state as M  # noqa: E402

METRICS = (
    "artifact_present",
    "moved_to_list",
    "due_dates",
    "priority_high",
    "paint_done",
    "others_untouched",
    "ui_path",
)
LIST = "Kitchen renovation"
TITLES = [
    "Get three contractor quotes", "Choose a contractor", "Order cabinets", "Order countertop",
    "Order sink and faucet", "Book electrician for new outlets", "Pick paint color",
    "Schedule final inspection", "Clear out the pantry",
]  # fmt: skip
ORDERS = ("Order cabinets", "Order countertop", "Order sink and faucet")
DUE = {"Choose a contractor": date(2026, 3, 13), "Book electrician for new outlets": date(2026, 3, 17)}


def main(argv: list[str]) -> int:
    root = Path(argv[1])
    export = root / "export"
    fs = export / "fs"
    m = {k: 0 for k in METRICS}
    extra: dict[str, object] = {"shutdown": M.manifest(export).get("shutdown", "none")}
    if not (fs / M.REMINDERS_STORES).is_dir():
        cu.emit(m, METRICS, **extra)
        return 0
    m["artifact_present"] = 1
    allr = [r for r in M.reminders(fs) if not r.deleted]
    by = {t: [r for r in allr if r.title.strip() == t] for t in TITLES}
    extra["state"] = {
        r.title: {"list": r.list, "due": str(r.due_date), "prio": r.priority, "done": r.completed}
        for r in allr
        if r.list in (LIST, "Ordering")
    }
    if all(len(v) == 1 for v in by.values()):
        one = {t: v[0] for t, v in by.items()}
        in_ordering = sorted(r.title for r in allr if r.list == "Ordering")
        m["moved_to_list"] = int(in_ordering == sorted(ORDERS))
        m["due_dates"] = int(all(one[t].due_date == d for t, d in DUE.items()))
        m["priority_high"] = int(one["Get three contractor quotes"].priority == 1)
        m["paint_done"] = int(one["Pick paint color"].completed)
        changed = {"Get three contractor quotes": "prio", "Pick paint color": "done", **dict.fromkeys(DUE, "due")}
        ok = all(one[t].list == LIST for t in TITLES if t not in ORDERS)
        for t, r in one.items():
            what = changed.get(t)
            ok = ok and ((r.due_date is not None) == (what == "due"))
            ok = ok and ((r.priority == 1) if what == "prio" else r.priority == 0)
            ok = ok and (r.completed == (what == "done"))
        personal = {r.title: r for r in allr if r.list == "Personal"}
        ok = ok and len(personal) == 2
        ok = ok and personal.get("Renew passport") is not None and personal["Renew passport"].priority == 9
        ok = ok and personal.get("Call the dentist") is not None and personal["Call the dentist"].priority == 5
        m["others_untouched"] = int(ok)
    events_log = M.input_events(root / "input.log")
    m["ui_path"] = int(bool({e.get("kind") for e in events_log} & {"click", "drag"}))
    extra["input_events"] = len(events_log)
    cu.emit(m, METRICS, **extra)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
