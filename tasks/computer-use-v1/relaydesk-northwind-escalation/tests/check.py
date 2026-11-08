#!/usr/bin/env python3
"""End-state checker for relaydesk-northwind-escalation.

    python3 check.py ARTIFACT_DIR SEED_JSON

ARTIFACT_DIR is the copy of /var/lib/relaydesk taken after the agent stopped
(app.db, boot.json, export/). Five target tickets must end assigned to Priya
Raman, in Billing, at High, with their other fields untouched; the closed
Northwind ticket and every other ticket must equal the seed; every changed
field must have been changed through a page load in the same browser session.
Shared helpers: /opt/vbdesk/cu_check.py (base image).
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

sys.path.insert(0, "/opt/vbdesk")
import cu_check as cu  # noqa: E402

TARGETS = (2007, 2013, 2021, 2034, 2048)
CLOSED_DISTRACTOR = 2010
WANT = {"assignee": "Priya Raman", "project": "Billing", "priority": "High"}
UNCHANGED_ON_TARGET = ("subject", "body", "requester", "company", "status", "created_at")
TICKET_FIELDS = (
    "id", "subject", "body", "requester", "company", "project", "priority", "status",
    "assignee", "created_at", "updated_at",
)
COMMENT_FIELDS = ("id", "ticket_id", "author", "body", "created_at")
AUDIT_FIELDS = ("seq", "sid", "event", "ticket_id", "field", "old", "new", "via", "tok_ok", "ui_ok")
MUTATIONS = {"ticket.update", "ticket.comment", "ticket.close", "ticket.delete"}
METRICS = (
    "artifact_present", "schema_ok", "app_not_restarted", "audit_consistent",
    "targets_assignee", "targets_project", "targets_priority", "targets_other_fields",
    "closed_untouched", "guards", "ui_path",
)


def main(argv: list[str]) -> int:
    art, seed_path = Path(argv[1]), Path(argv[2])
    seed = json.loads(seed_path.read_text())
    for t in seed["tickets"]:
        t.setdefault("company", "")
    m: dict[str, int] = {k: 0 for k in METRICS}
    db = art / "app.db"
    m["artifact_present"] = int(db.is_file())
    if not m["artifact_present"]:
        cu.emit(m, METRICS, note="app.db missing from artifact")
        return 0

    con = cu.open_ro(db)
    m["schema_ok"] = int({"projects", "users", "tickets", "comments", "audit"} <= cu.table_names(con))
    if not m["schema_ok"]:
        cu.emit(m, METRICS, note="tables missing")
        return 0

    tickets = {r["id"]: dict(r) for r in con.execute("SELECT * FROM tickets")}
    comments = cu.rows(con, "comments", COMMENT_FIELDS)
    projects = [r[0] for r in cu.rows(con, "projects", ("name",))]
    users = [r[0] for r in cu.rows(con, "users", ("name",))]
    db_audit = cu.rows(con, "audit", AUDIT_FIELDS, order_by="seq")

    export = cu.load_json(art / "export" / "audit.json")
    m["app_not_restarted"] = cu.app_not_restarted(art, export)
    m["audit_consistent"] = cu.audit_consistent(db_audit, export, AUDIT_FIELDS)

    seed_by_id = {s["id"]: s for s in seed["tickets"]}
    present = all(tid in tickets for tid in TARGETS)
    if present:
        m["targets_assignee"] = int(all(tickets[t]["assignee"] == WANT["assignee"] for t in TARGETS))
        m["targets_project"] = int(all(tickets[t]["project"] == WANT["project"] for t in TARGETS))
        m["targets_priority"] = int(all(tickets[t]["priority"] == WANT["priority"] for t in TARGETS))
        m["targets_other_fields"] = int(all(
            tickets[t][f] == seed_by_id[t][f] for t in TARGETS for f in UNCHANGED_ON_TARGET
        ))
    row = lambda t: tuple(t[f] for f in TICKET_FIELDS)  # noqa: E731
    m["closed_untouched"] = int(
        CLOSED_DISTRACTOR in tickets and row(tickets[CLOSED_DISTRACTOR]) == row(seed_by_id[CLOSED_DISTRACTOR])
        and not any(r[2] in MUTATIONS and r[3] == CLOSED_DISTRACTOR for r in db_audit)
    )

    # guards: nothing else changed, in state or in the event log
    others_ok = all(
        tid in tickets and row(tickets[tid]) == row(s)
        for tid, s in seed_by_id.items() if tid not in TARGETS
    )
    counts_ok = len(tickets) == len(seed_by_id)
    comments_ok = comments == cu.seed_rows(seed, "comments", COMMENT_FIELDS)
    refs_ok = projects == seed["projects"] and users == seed["users"]
    events_ok = all(
        not (r[2] in MUTATIONS and r[3] not in TARGETS) and r[2] not in {"ticket.close", "ticket.delete"}
        for r in db_audit
    )
    m["guards"] = int(others_ok and counts_ok and comments_ok and refs_ok and events_ok)

    # UI path: for every field that had to change on every target, a form update
    # in a session that viewed that ticket earlier, carrying the page token and
    # the script-computed value. Fields already at the wanted value need no row.
    first_view: dict[tuple[str, int], int] = {}
    for r in db_audit:
        if r[2] == "page.view" and (r[1], r[3]) not in first_view:
            first_view[(r[1], r[3])] = r[0]

    def via_ui(tid: int, field: str, value: str) -> bool:
        return any(
            r[2] == "ticket.update" and r[3] == tid and r[4] == field and r[6] == value
            and r[7] == "form" and r[8] == 1 and r[9] == 1
            and (r[1], tid) in first_view and r[0] > first_view[(r[1], tid)]
            for r in db_audit
        )

    m["ui_path"] = int(present and all(
        seed_by_id[t][f] == v or via_ui(t, f, v) for t in TARGETS for f, v in WANT.items()
    ))
    cu.emit(m, METRICS, audit_rows=len(db_audit))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
