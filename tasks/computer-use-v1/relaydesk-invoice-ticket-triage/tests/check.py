#!/usr/bin/env python3
"""End-state checker for relaydesk-invoice-ticket-triage.

    python3 check.py ARTIFACT_DIR SEED_JSON

ARTIFACT_DIR is the copy of /var/lib/relaydesk taken after the agent stopped:
app.db (the application's SQLite file), boot.json (written at boot) and
export/ (written by the collect hook while the app was still running: the
in-memory audit mirror and process start times). Prints one JSON object with
the reward and every sub-metric as flat integers; reward is 1 only when every
sub-metric is 1. Shared helpers: /opt/vbdesk/cu_check.py (base image).
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

sys.path.insert(0, "/opt/vbdesk")
import cu_check as cu  # noqa: E402

TARGET = 1017
WANT = {"project": "Billing", "priority": "High"}
UNCHANGED_ON_TARGET = ("subject", "body", "requester", "company", "status", "assignee", "created_at")
TICKET_FIELDS = (
    "id", "subject", "body", "requester", "company", "project", "priority", "status",
    "assignee", "created_at", "updated_at",
)
COMMENT_FIELDS = ("id", "ticket_id", "author", "body", "created_at")
AUDIT_FIELDS = ("seq", "sid", "event", "ticket_id", "field", "old", "new", "via", "tok_ok", "ui_ok")
MUTATIONS = {"ticket.update", "ticket.comment", "ticket.close", "ticket.delete"}
METRICS = (
    "artifact_present", "schema_ok", "app_not_restarted", "audit_consistent",
    "target_project", "target_priority", "target_other_fields", "guards", "ui_path",
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

    # target ticket
    t = tickets.get(TARGET)
    seed_t = {s["id"]: s for s in seed["tickets"]}[TARGET]
    if t is not None:
        m["target_project"] = int(t["project"] == WANT["project"])
        m["target_priority"] = int(t["priority"] == WANT["priority"])
        m["target_other_fields"] = int(all(t[f] == seed_t[f] for f in UNCHANGED_ON_TARGET))

    # guards: nothing else changed, in state or in the event log
    seed_tickets = {s["id"]: tuple(s[f] for f in TICKET_FIELDS) for s in seed["tickets"]}
    others_ok = all(
        tid in tickets and tuple(tickets[tid][f] for f in TICKET_FIELDS) == row
        for tid, row in seed_tickets.items() if tid != TARGET
    )
    counts_ok = len(tickets) == len(seed_tickets)
    comments_ok = comments == cu.seed_rows(seed, "comments", COMMENT_FIELDS)
    refs_ok = projects == seed["projects"] and users == seed["users"]
    events_ok = all(
        not (row[2] in MUTATIONS and row[3] != TARGET) and row[2] not in {"ticket.close", "ticket.delete"}
        for row in db_audit
    )
    m["guards"] = int(others_ok and counts_ok and comments_ok and refs_ok and events_ok)

    # the change was made through the UI: a page view of the ticket in a session,
    # then form updates in that session carrying the page's token and script value
    view_seq: dict[str, int] = {}
    for row in db_audit:  # first view of the target per session
        if row[2] == "page.view" and row[3] == TARGET and row[1] not in view_seq:
            view_seq[row[1]] = row[0]

    def via_ui(field: str, value: str) -> bool:
        return any(
            row[2] == "ticket.update" and row[3] == TARGET and row[4] == field and row[6] == value
            and row[7] == "form" and row[8] == 1 and row[9] == 1
            and row[1] in view_seq and row[0] > view_seq[row[1]]
            for row in db_audit
        )

    m["ui_path"] = int(via_ui("project", WANT["project"]) and via_ui("priority", WANT["priority"]))
    cu.emit(m, METRICS, audit_rows=len(db_audit))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
