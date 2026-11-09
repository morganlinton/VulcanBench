#!/usr/bin/env python3
"""End-state checker for relaydesk-sla-sweep.

    python3 check.py RELAYDESK_DIR SEED_RELAYDESK ARRIVALS_JSON POSTBOX_DIR SEED_POSTBOX

Relay Desk: each of the seven breached tickets must carry a new comment
saying "SLA breached" and end at priority Urgent with its other fields
untouched; the seven near-misses, the ticket that arrived mid-task and
every other ticket must equal the seed (or the scheduled record); every
change must have come from a form post in a session that had loaded the
ticket's page. Postbox: exactly one mail sent, to Priya, whose text names
exactly the escalated ticket numbers; nothing else changed.
Shared helpers: /opt/vbdesk/cu_check.py (base image).
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

sys.path.insert(0, "/opt/vbdesk")
import cu_check as cu  # noqa: E402

TARGETS = (5003, 5011, 5018, 5026, 5034, 5041, 5052)
NEAR_MISSES = (5007, 5015, 5022, 5030, 5037, 5045, 5049, 5101)
PRIYA = "priya@relaydesk.example"
COMMENT_RE = re.compile(r"sla\s+breached", re.I)
TICKET_RE = re.compile(r"\b5\d{3}\b")
TICKET_FIELDS = ("id", "subject", "body", "requester", "company", "project", "priority", "status",
                 "assignee", "created_at", "updated_at")
UNCHANGED_ON_TARGET = ("subject", "body", "requester", "company", "project", "status", "assignee", "created_at")
COMMENT_FIELDS = ("id", "ticket_id", "author", "body", "created_at")
RD_AUDIT = ("seq", "sid", "event", "ticket_id", "field", "old", "new", "via", "tok_ok", "ui_ok")
PB_AUDIT = ("seq", "sid", "event", "message_id", "field", "old", "new", "via", "tok_ok", "ui_ok")
PB_STATE = ("id", "thread_id", "folder", "from_name", "from_email", "to_addr", "subject", "body",
            "received_at", "attachment", "newsletter", "unsubscribed")
RD_MUTATIONS = {"ticket.update", "ticket.comment", "ticket.close", "ticket.delete"}
PB_MUTATIONS = {"message.archive", "message.trash", "message.move", "message.delete", "trash.empty",
                "list.unsubscribe"}
METRICS = (
    "artifacts_present", "schema_ok", "apps_not_restarted", "audits_consistent",
    "targets_comment", "targets_priority", "targets_other_fields", "near_misses_untouched", "desk_guards",
    "mail_sent", "mail_ids_exact", "mail_guards", "ui_path",
)


def main(argv: list[str]) -> int:
    rd, rd_seed_p, arrivals_p, pb, pb_seed_p = (Path(a) for a in argv[1:6])
    rd_seed, pb_seed = json.loads(rd_seed_p.read_text()), json.loads(pb_seed_p.read_text())
    arrivals = json.loads(arrivals_p.read_text())
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
    m["schema_ok"] = int({"projects", "users", "tickets", "comments", "audit"} <= cu.table_names(rcon)
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

    # ---- Relay Desk: expected rows are the seed plus every arrival the server recorded
    tickets = {r["id"]: dict(r) for r in rcon.execute("SELECT * FROM tickets")}
    comments = [dict(r) for r in rcon.execute("SELECT * FROM comments ORDER BY id")]
    projects = [r[0] for r in cu.rows(rcon, "projects", ("name",))]
    users = [r[0] for r in cu.rows(rcon, "users", ("name",))]
    arrived = cu.arrived_ids(rd_audit)
    expected = {s["id"]: s for s in rd_seed["tickets"]}
    expected_comments = cu.seed_rows(rd_seed, "comments", COMMENT_FIELDS)
    for a in arrivals:
        t = dict(a["ticket"])
        t.setdefault("company", "")
        if t["id"] in arrived:
            expected[t["id"]] = t
            expected_comments += [tuple(c[f] for f in COMMENT_FIELDS) for c in a.get("comments", [])]
    row = lambda d: tuple(d[f] for f in TICKET_FIELDS)  # noqa: E731
    new_comments = [c for c in comments if tuple(c[f] for f in COMMENT_FIELDS) not in expected_comments]
    present = all(tid in tickets for tid in TARGETS)
    if present:
        m["targets_comment"] = int(all(
            any(c["ticket_id"] == tid and COMMENT_RE.search(c["body"] or "") for c in new_comments) for tid in TARGETS
        ))
        m["targets_priority"] = int(all(tickets[tid]["priority"] == "Urgent" for tid in TARGETS))
        m["targets_other_fields"] = int(all(
            tickets[tid][f] == expected[tid][f] for tid in TARGETS for f in UNCHANGED_ON_TARGET
        ))

    def untouched(ids: tuple[int, ...]) -> bool:
        return all(
            (tid not in expected and tid not in tickets)  # an arrival that never came
            or (tid in tickets and tid in expected and row(tickets[tid]) == row(expected[tid])
                and not any(r[2] in RD_MUTATIONS and r[3] == tid for r in rd_audit)
                and not any(c["ticket_id"] == tid for c in new_comments))
            for tid in ids
        )

    m["near_misses_untouched"] = int(untouched(NEAR_MISSES))
    others_ok = all(
        tid in tickets and row(tickets[tid]) == row(s) for tid, s in expected.items() if tid not in TARGETS
    )
    counts_ok = set(tickets) == set(expected)
    old_comments = [tuple(c[f] for f in COMMENT_FIELDS) for c in comments
                    if tuple(c[f] for f in COMMENT_FIELDS) in expected_comments]
    old_comments_ok = sorted(old_comments) == sorted(expected_comments)
    only_target_comments = all(c["ticket_id"] in TARGETS for c in new_comments)
    refs_ok = projects == rd_seed["projects"] and users == rd_seed["users"]
    events_ok = all(
        not (r[2] in RD_MUTATIONS and r[3] not in TARGETS) and r[2] not in {"ticket.close", "ticket.delete"}
        for r in rd_audit
    )
    m["desk_guards"] = int(others_ok and counts_ok and old_comments_ok and only_target_comments and refs_ok and events_ok)

    # ---- Postbox: one mail to Priya naming exactly the escalated tickets, nothing else changed
    outbox = [dict(r) for r in pcon.execute("SELECT * FROM outbox ORDER BY id")]
    sends = [r for r in pb_audit if r[2] == "mail.send" and r[7] == "form" and r[8] == 1 and r[9] == 1]
    mail = outbox[0] if len(outbox) == 1 else None
    m["mail_sent"] = int(mail is not None and PRIYA in (mail["to_addr"] or "").lower() and len(sends) == 1)
    if mail is not None:
        found = {int(x) for x in TICKET_RE.findall(f"{mail['subject']}\n{mail['body']}")}
        m["mail_ids_exact"] = int(found == set(TARGETS))
    msgs = {r["id"]: dict(r) for r in pcon.execute("SELECT * FROM messages")}
    prow = lambda d: tuple(d[f] for f in PB_STATE)  # noqa: E731
    seed_msgs = {s["id"]: s for s in pb_seed["messages"]}
    extra_msgs = [v for k, v in msgs.items() if k not in seed_msgs]
    m["mail_guards"] = int(
        all(mid in msgs and prow(msgs[mid]) == prow(s) for mid, s in seed_msgs.items())
        and all(v["folder"] == "Sent" for v in extra_msgs) and len(extra_msgs) == len(outbox) <= 1
        and not any(r[2] in PB_MUTATIONS for r in pb_audit)
    )

    # ---- UI path: every change came from a form post after a page load of that ticket, in the same session
    first_view: dict[tuple[str, int], int] = {}
    for r in rd_audit:
        if r[2] == "page.view" and (r[1], r[3]) not in first_view:
            first_view[(r[1], r[3])] = r[0]

    def via_ui(tid: int, event: str, field: str, value_check) -> bool:
        return any(
            r[2] == event and r[3] == tid and r[4] == field and value_check(r[6]) and r[7] == "form"
            and r[8] == 1 and r[9] == 1 and (r[1], tid) in first_view and r[0] > first_view[(r[1], tid)]
            for r in rd_audit
        )

    m["ui_path"] = int(present and all(
        via_ui(tid, "ticket.comment", "comment", lambda v: v is not None and COMMENT_RE.search(v) is not None)
        and via_ui(tid, "ticket.update", "priority", lambda v: v == "Urgent")
        for tid in TARGETS
    ) and len(sends) == 1)
    cu.emit(m, METRICS, new_comments=len(new_comments), arrived=sorted(arrived), outbox=len(outbox),
            desk_audit_rows=len(rd_audit), mail_audit_rows=len(pb_audit))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
