#!/usr/bin/env python3
"""invoices-to-reminders checker: the Reminders store copied from the
stopped VM. Today in the guest is Monday 9 March 2026.

    check.py /var/lib/vbmac

Expected, in a list named "Bills" (a reminder matches a vendor when its
title contains the vendor's key word; the amount may be in the title or the
notes, written 1,240.00, 1240, 2.450,00 or 2,450.00):
  Brightwave  due 2026-03-14  1240.00  (invoiced 27 Feb, Net 15)
  Cobalt      due 2026-03-31   389.50
  Lumiere     due 2026-03-16  2450.00  (16.03.2026, day first; EUR)
  Greenleaf   due 2026-03-22   860.00  (invoiced 20 Feb, 30 days)
No reminder for Northwind (paid) or for the credit note.

Sub-metrics (reward is 1 only when all are 1):
  artifact_present  the Reminders store was collected
  bills_list        a list named "Bills" exists
  terms_dates       Brightwave and Greenleaf are due on the dates their
                    terms give
  european_date     Atelier Lumiere is due on 16 March
  amounts           each of the four carries its amount
  skipped_settled   Bills holds exactly the four (nothing for Northwind or
                    the credit note), titled with "Pay"
  ui_path           the MCP server logged pointer input
"""

from __future__ import annotations

import re
import sys
from datetime import date
from pathlib import Path

sys.path.insert(0, "/opt/vbdesk")
import cu_check as cu  # noqa: E402
import mac_state as M  # noqa: E402

METRICS = (
    "artifact_present",
    "bills_list",
    "terms_dates",
    "european_date",
    "amounts",
    "skipped_settled",
    "ui_path",
)
WANT = {
    "brightwave": (date(2026, 3, 14), 1240.00),
    "cobalt": (date(2026, 3, 31), 389.50),
    "lumiere": (date(2026, 3, 16), 2450.00),
    "greenleaf": (date(2026, 3, 22), 860.00),
}
NUM = re.compile(r"\d[\d.,]*")


def amounts_in(text: str) -> set[float]:
    """Every number in the text, read both US style (1,240.50) and European
    style (1.240,50)."""
    out: set[float] = set()
    for tok in NUM.findall(text):
        tok = tok.rstrip(".,")
        for s in (tok.replace(",", ""), tok.replace(".", "").replace(",", ".")):
            try:
                out.add(round(float(s), 2))
            except ValueError:
                pass
    return out


def main(argv: list[str]) -> int:
    root = Path(argv[1])
    export = root / "export"
    fs = export / "fs"
    m = {k: 0 for k in METRICS}
    extra: dict[str, object] = {"shutdown": M.manifest(export).get("shutdown", "none")}
    stores = fs / M.REMINDERS_STORES
    if not stores.is_dir():
        cu.emit(m, METRICS, **extra)
        return 0
    m["artifact_present"] = 1
    lists = M.reminder_lists(fs)
    m["bills_list"] = int("Bills" in lists)
    bills = [r for r in M.reminders(fs) if r.list == "Bills" and not r.deleted]
    extra["bills"] = [(r.title, str(r.due_date), r.notes[:40]) for r in bills]
    hit = {k: [r for r in bills if k in r.title.lower()] for k in WANT}
    one = {k: v[0] for k, v in hit.items() if len(v) == 1}
    m["terms_dates"] = int(all(k in one and one[k].due_date == WANT[k][0] for k in ("brightwave", "greenleaf")))
    m["european_date"] = int("lumiere" in one and one["lumiere"].due_date == WANT["lumiere"][0])
    m["amounts"] = int(
        len(one) == 4 and all(WANT[k][1] in amounts_in(f"{r.title} {r.notes}") for k, r in one.items())
    )
    titled = all(r.title.lower().lstrip().startswith("pay") for r in bills)
    m["skipped_settled"] = int(len(bills) == 4 and len(one) == 4 and titled and m["bills_list"] == 1)
    events_log = M.input_events(root / "input.log")
    m["ui_path"] = int(bool({e.get("kind") for e in events_log} & {"click", "drag"}))
    extra["input_events"] = len(events_log)
    cu.emit(m, METRICS, **extra)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
