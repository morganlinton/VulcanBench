#!/usr/bin/env python3
"""reminders-renovation-plan checker: the Reminders store copied from the
stopped VM.

    check.py /var/lib/vbmac

Reminders are found by title (live ones only); priority 1 is high.

Sub-metrics (reward is 1 only when all are 1):
  artifact_present  the Reminders store was collected
  moved_to_list     Order cabinets, Order countertop and Order sink and faucet
                    are in a list named Ordering, and nothing else is
  flags             Choose a contractor and Book electrician for new outlets
                    are flagged
  priority_high     Get three contractor quotes has high priority
  paint_done        Pick paint color is completed (not deleted)
  others_untouched  every other reminder in the list is unflagged, without
                    priority and not completed; all nine are still there once,
                    the six others in Kitchen renovation; Personal is unchanged
  ui_path           the MCP server logged pointer input
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, "/opt/vbdesk")
import cu_check as cu  # noqa: E402
import mac_state as M  # noqa: E402

METRICS = (
    "artifact_present",
    "moved_to_list",
    "flags",
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
FLAGS = ("Choose a contractor", "Book electrician for new outlets")


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
        r.title: {"list": r.list, "flag": r.flagged, "prio": r.priority, "done": r.completed}
        for r in allr
        if r.list in (LIST, "Ordering")
    }
    if all(len(v) == 1 for v in by.values()):
        one = {t: v[0] for t, v in by.items()}
        in_ordering = sorted(r.title for r in allr if r.list == "Ordering")
        m["moved_to_list"] = int(in_ordering == sorted(ORDERS))
        m["flags"] = int(all(one[t].flagged for t in FLAGS))
        m["priority_high"] = int(one["Get three contractor quotes"].priority == 1)
        m["paint_done"] = int(one["Pick paint color"].completed)
        changed = {"Get three contractor quotes": "prio", "Pick paint color": "done", **dict.fromkeys(FLAGS, "flag")}
        ok = all(one[t].list == LIST for t in TITLES if t not in ORDERS)
        for t, r in one.items():
            what = changed.get(t)
            ok = ok and (r.flagged == (what == "flag"))
            ok = ok and ((r.priority == 1) if what == "prio" else r.priority == 0)
            ok = ok and (r.completed == (what == "done"))
        personal = {r.title: r for r in allr if r.list == "Personal"}
        ok = ok and len(personal) == 2
        ok = ok and personal.get("Renew passport") is not None and personal["Renew passport"].flagged
        ok = ok and personal.get("Call the dentist") is not None and personal["Call the dentist"].priority == 5
        m["others_untouched"] = int(ok)
    events_log = M.input_events(root / "input.log")
    m["ui_path"] = int(bool({e.get("kind") for e in events_log} & {"click", "drag"}))
    extra["input_events"] = len(events_log)
    cu.emit(m, METRICS, **extra)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
