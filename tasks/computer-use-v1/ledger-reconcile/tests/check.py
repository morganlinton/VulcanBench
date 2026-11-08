#!/usr/bin/env python3
"""End-state checker for ledger-reconcile.

    python3 check.py DESK_ARTIFACT SEED_RELAYDESK FINANCE_DIR LEDGER_JSON INPUT_LOG_DIR

Reads the report the agent wrote, the ledger workbook and the tracker's
database copy. Sub-metrics:

  report_present      discrepancies.txt exists and is not empty
  report_ids_exact    the set of invoice numbers in the report equals the four
                      discrepant ones (no misses, no false positives)
  report_details      each line says what is wrong: the amount line carries one
                      of the two figures or the word amount; the double entry
                      says twice or duplicate; the one-sided ones say missing,
                      no ticket, not in, only, or the like
  ledger_untouched    the workbook's rows equal the seed
  desk_untouched      no ticket changed and no mutation event in the tracker
  apps_not_restarted  the tracker that served the run is the one that booted
  audit_consistent    the tracker's audit log equals its live mirror
  ui_path             real input on the display before the report was written,
                      and the tracker's list was viewed at least once

Shared helpers: /opt/vbdesk/cu_check.py (base image).
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

sys.path.insert(0, "/opt/vbdesk")
import cu_check as cu  # noqa: E402

INV = r"INV-2026-\d{4}"
TICKET_FIELDS = (
    "id", "subject", "body", "requester", "company", "project", "priority", "status",
    "assignee", "created_at", "updated_at",
)
AUDIT_FIELDS = ("seq", "sid", "event", "ticket_id", "field", "old", "new", "via", "tok_ok", "ui_ok")
MUTATIONS = {"ticket.update", "ticket.comment", "ticket.close", "ticket.delete"}
ONE_SIDED = re.compile(
    r"missing|no (\w+[ -]){0,2}(ticket|row|entry|record|posting|match)|not in|only in|absent|without"
    r"|no (matching|corresponding)|isn.t in|is not|does not|doesn.t|never|but no\b|not (found|present|posted|recorded)",
    re.I,
)
TWICE = re.compile(r"twice|duplicat|two (rows|entries|times)|double|repeated|entered 2", re.I)
METRICS = (
    "report_present", "report_ids_exact", "report_details", "ledger_untouched", "desk_untouched",
    "apps_not_restarted", "audit_consistent", "ui_path",
)


def amount_words(v: float) -> re.Pattern:
    whole = int(v)
    return re.compile(rf"{whole:,}(\.\d\d)?|{whole}(\.\d\d)?".replace(",", "[,.]?"))


def main(argv: list[str]) -> int:
    desk, seed_path, finance, key_path, vb = (Path(a) for a in argv[1:6])
    seed = json.loads(seed_path.read_text())
    key = json.loads(key_path.read_text())
    for t in seed["tickets"]:
        t.setdefault("company", "")
    m: dict[str, int] = {k: 0 for k in METRICS}

    report = finance / key["report"]
    text = cu.read_text(report)
    m["report_present"] = int(report.is_file() and bool(text.strip()))
    want = set(key["discrepancies"])
    found = cu.text_ids(report, INV)
    m["report_ids_exact"] = int(found == want)
    details = []
    for inv, d in key["discrepancies"].items():
        lines = " ".join(cu.text_lines_with(report, inv))
        if d["kind"] == "amount":
            ok = bool(amount_words(d["ledger"]).search(lines) or amount_words(d["ticket"]).search(lines)
                      or re.search(r"amount|differ|mismatch|wrong|total", lines, re.I))
        elif d["kind"] == "duplicate":
            ok = bool(TWICE.search(lines))
        else:
            ok = bool(ONE_SIDED.search(lines))
        details.append(ok)
    m["report_details"] = int(all(details) and m["report_ids_exact"] == 1)

    ods = finance / key["file"]
    try:
        sheets = cu.ods_sheets(ods)
        seed_rows = cu.ods_rows_from_seed([key["headers"]] + [
            [r["invoice"], r["customer"], r["posted"], r["amount"]] for r in key["rows"]
        ])
        m["ledger_untouched"] = int(
            cu.ods_is_spreadsheet(ods) == 1 and key["sheet"] in sheets and cu.ods_same_rows(sheets[key["sheet"]], seed_rows)
        )
    except Exception as e:  # noqa: BLE001 - an unreadable workbook is a changed workbook
        print(f"ledger unreadable: {e}", file=sys.stderr)

    db = desk / "app.db"
    db_audit: list[tuple] = []
    if db.is_file():
        con = cu.open_ro(db)
        if {"tickets", "comments", "audit"} <= cu.table_names(con):
            tickets = cu.rows(con, "tickets", TICKET_FIELDS)
            db_audit = cu.rows(con, "audit", AUDIT_FIELDS, order_by="seq")
            export = cu.load_json(desk / "export" / "audit.json")
            m["apps_not_restarted"] = cu.app_not_restarted(desk, export)
            m["audit_consistent"] = cu.audit_consistent(db_audit, export, AUDIT_FIELDS)
            m["desk_untouched"] = int(
                tickets == cu.seed_rows(seed, "tickets", TICKET_FIELDS)
                and cu.rows(con, "comments", ("id",)) == []
                and not any(r[2] in MUTATIONS for r in db_audit)
            )

    presses = cu.presses(cu.xinput_events(vb / "xinput.log"))
    looked = any(r[2] == "page.list" for r in db_audit)
    m["ui_path"] = int(
        m["report_present"] == 1 and looked
        and cu.ui_path_fs(presses, [report.stat().st_mtime], window_s=1800.0, min_presses=20)
    )
    cu.emit(m, METRICS, presses=len(presses), found=sorted(found), list_views=sum(1 for r in db_audit if r[2] == "page.list"))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
