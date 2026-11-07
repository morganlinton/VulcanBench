#!/usr/bin/env python3
"""End-state checker for postbox-to-relaydesk-refund.

    python3 check.py RELAYDESK_DIR RELAYDESK_SEED POSTBOX_DIR POSTBOX_SEED

Relay Desk: ticket #1016 must carry a new comment containing RF-2026-0388 and
be in status Waiting on Finance, with nothing else changed, both through the
UI. Postbox: nothing changed and nothing sent (reading is free).
Shared helpers: /opt/vbdesk/cu_check.py (base image).
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

sys.path.insert(0, "/opt/vbdesk")
import cu_check as cu  # noqa: E402

TARGET = 1016
REFERENCE = "RF-2026-0388"
WANT_STATUS = "Waiting on Finance"
TICKET_FIELDS = ("id", "subject", "body", "requester", "company", "project", "priority", "status",
                 "assignee", "created_at", "updated_at")
UNCHANGED_ON_TARGET = ("subject", "body", "requester", "company", "project", "priority", "assignee", "created_at")
COMMENT_FIELDS = ("id", "ticket_id", "author", "body", "created_at")
RD_AUDIT = ("seq", "sid", "event", "ticket_id", "field", "old", "new", "via", "tok_ok", "ui_ok")
PB_AUDIT = ("seq", "sid", "event", "message_id", "field", "old", "new", "via", "tok_ok", "ui_ok")
PB_STATE = ("id", "thread_id", "folder", "from_name", "from_email", "to_addr", "subject", "body",
            "received_at", "attachment", "newsletter", "unsubscribed")
RD_MUTATIONS = {"ticket.update", "ticket.comment", "ticket.close", "ticket.delete"}
PB_MUTATIONS = {"message.archive", "message.trash", "message.move", "message.delete", "trash.empty",
                "list.unsubscribe", "mail.send"}
METRICS = (
    "artifacts_present", "schema_ok", "apps_not_restarted", "audits_consistent",
    "target_comment", "target_status", "target_other_fields", "desk_guards", "mail_untouched", "ui_path",
)


def main(argv: list[str]) -> int:
    rd, rd_seed_p, pb, pb_seed_p = (Path(a) for a in argv[1:5])
    rd_seed, pb_seed = json.loads(rd_seed_p.read_text()), json.loads(pb_seed_p.read_text())
    for t in rd_seed["tickets"]:
        t.setdefault("company", "")
    for s in pb_seed["messages"]:
        s.setdefault("to_addr", "you@relaydesk.example")
        s.setdefault("attachment", "")
        s.setdefault("newsletter", 0)
        s["unsubscribed"] = 0
    m: dict[str, int] = {k: 0 for k in METRICS}
    m["artifacts_present"] = int((rd / "app.db").is_file() and (pb / "app.db").is_file())
    if not m["artifacts_present"]:
        cu.emit(m, METRICS, note="a database is missing")
        return 0
    rcon, pcon = cu.open_ro(rd / "app.db"), cu.open_ro(pb / "app.db")
    m["schema_ok"] = int({"tickets", "comments", "audit"} <= cu.table_names(rcon)
                         and {"messages", "outbox", "audit"} <= cu.table_names(pcon))
    if not m["schema_ok"]:
        cu.emit(m, METRICS, note="tables missing")
        return 0

    rd_export = cu.load_json(rd / "export" / "audit.json")
    pb_export = cu.load_json(pb / "export" / "audit.json")
    rd_audit = cu.rows(rcon, "audit", RD_AUDIT, order_by="seq")
    pb_audit = cu.rows(pcon, "audit", PB_AUDIT, order_by="seq")
    m["apps_not_restarted"] = int(cu.app_not_restarted(rd, rd_export) and cu.app_not_restarted(pb, pb_export))
    m["audits_consistent"] = int(cu.audit_consistent(rd_audit, rd_export, RD_AUDIT)
                                 and cu.audit_consistent(pb_audit, pb_export, PB_AUDIT))

    tickets = {r["id"]: dict(r) for r in rcon.execute("SELECT * FROM tickets")}
    comments = [dict(r) for r in rcon.execute("SELECT * FROM comments ORDER BY id")]
    seed_by_id = {s["id"]: s for s in rd_seed["tickets"]}
    seed_comments = cu.seed_rows(rd_seed, "comments", COMMENT_FIELDS)
    new_comments = [c for c in comments if tuple(c[f] for f in COMMENT_FIELDS) not in seed_comments]
    t = tickets.get(TARGET)
    m["target_comment"] = int(any(c["ticket_id"] == TARGET and REFERENCE in c["body"] for c in new_comments))
    if t is not None:
        m["target_status"] = int(t["status"] == WANT_STATUS)
        m["target_other_fields"] = int(all(t[f] == seed_by_id[TARGET][f] for f in UNCHANGED_ON_TARGET))
    row = lambda d: tuple(d[f] for f in TICKET_FIELDS)  # noqa: E731
    others_ok = all(tid in tickets and row(tickets[tid]) == row(s) for tid, s in seed_by_id.items() if tid != TARGET)
    old_comments_ok = [tuple(c[f] for f in COMMENT_FIELDS) for c in comments if tuple(c[f] for f in COMMENT_FIELDS) in seed_comments] == seed_comments
    only_target_comments = all(c["ticket_id"] == TARGET for c in new_comments) and len(new_comments) <= 2
    events_ok = all(not (r[2] in RD_MUTATIONS and r[3] != TARGET) and r[2] not in {"ticket.close", "ticket.delete"} for r in rd_audit)
    m["desk_guards"] = int(others_ok and old_comments_ok and only_target_comments and events_ok and len(tickets) == len(seed_by_id))

    msgs = {r["id"]: dict(r) for r in pcon.execute("SELECT * FROM messages")}
    prow = lambda d: tuple(d[f] for f in PB_STATE)  # noqa: E731
    outbox = cu.rows(pcon, "outbox", ("id",))
    m["mail_untouched"] = int(
        all(mid in msgs and prow(msgs[mid]) == prow(s) for mid, s in ((s["id"], s) for s in pb_seed["messages"]))
        and len(msgs) == len(pb_seed["messages"]) and not outbox
        and not any(r[2] in PB_MUTATIONS for r in pb_audit)
    )

    first_view: dict[tuple[str, int], int] = {}
    for r in rd_audit:
        if r[2] == "page.view" and (r[1], r[3]) not in first_view:
            first_view[(r[1], r[3])] = r[0]

    def via_ui(event: str, field: str, value_check) -> bool:
        return any(
            r[2] == event and r[3] == TARGET and r[4] == field and value_check(r[6]) and r[7] == "form"
            and r[8] == 1 and r[9] == 1 and (r[1], TARGET) in first_view and r[0] > first_view[(r[1], TARGET)]
            for r in rd_audit
        )

    m["ui_path"] = int(via_ui("ticket.comment", "comment", lambda v: v is not None and REFERENCE in v)
                       and via_ui("ticket.update", "status", lambda v: v == WANT_STATUS))
    cu.emit(m, METRICS, new_comments=len(new_comments), desk_audit_rows=len(rd_audit), mail_audit_rows=len(pb_audit))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
