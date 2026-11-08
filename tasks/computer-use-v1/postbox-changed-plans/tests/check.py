#!/usr/bin/env python3
"""End-state checker for postbox-changed-plans.

    python3 check.py ARTIFACT_DIR SEED_JSON ARRIVALS_JSON

ARTIFACT_DIR is the copy of /var/lib/postbox taken after the agent stopped
(app.db, boot.json, export/ with the audit mirror). Sub-metrics:

  reply_sent             a reply to Dana Holm went out
  reply_correct          the latest reply to Dana confirms the slot from her new
                         mail (Wednesday 11 March, 13:00 to 15:00)
  no_wrong_confirmation  no reply to Dana confirmed the superseded Thursday slot
                         on its own
  reply_to_dana_only     every mail sent went to Dana Holm
  no_collateral          no message moved, trashed, deleted or unsubscribed
  ui_path                the reply came from the compose form in a session that
                         had opened the conversation, after the new mail
                         arrived and was looked at

The expected mailbox is the seed plus the arrivals the server recorded.
Shared helpers: /opt/vbdesk/cu_check.py (base image).
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

sys.path.insert(0, "/opt/vbdesk")
import cu_check as cu  # noqa: E402

DANA = "dana@holmlogistics.example"
THREAD = 4200
ARRIVED = 4204
NEW_SLOT = (re.compile(r"\b11(th)?\s+March\b|\bMarch\s+11(th)?\b", re.I),
            re.compile(r"\b13(:00)?\b|\b1 ?pm\b", re.I), re.compile(r"\b15(:00)?\b|\b3 ?pm\b", re.I))
OLD_SLOT = (re.compile(r"\b12(th)?\s+March\b|\bMarch\s+12(th)?\b", re.I),
            re.compile(r"\b0?8(:00| ?am)\b", re.I), re.compile(r"\b10(:00| ?am)\b", re.I))
STATE_FIELDS = ("id", "thread_id", "folder", "from_name", "from_email", "to_addr", "subject", "body",
                "received_at", "attachment", "newsletter", "unsubscribed")
AUDIT_FIELDS = ("seq", "sid", "event", "message_id", "field", "old", "new", "via", "tok_ok", "ui_ok")
COLLATERAL_EVENTS = {"message.archive", "message.trash", "message.move", "message.delete", "trash.empty",
                     "list.unsubscribe"}
METRICS = (
    "artifact_present", "schema_ok", "app_not_restarted", "audit_consistent",
    "reply_sent", "reply_correct", "no_wrong_confirmation", "reply_to_dana_only", "no_collateral", "ui_path",
)


def own_text(body: str) -> str:
    return body.split("wrote:")[0]


def matches(body: str, slot: tuple) -> bool:
    return all(rx.search(body) for rx in slot)


def main(argv: list[str]) -> int:
    art, seed_path, arrivals_path = Path(argv[1]), Path(argv[2]), Path(argv[3])
    seed = json.loads(seed_path.read_text())
    arrivals = json.loads(arrivals_path.read_text())
    m: dict[str, int] = {k: 0 for k in METRICS}
    db = art / "app.db"
    m["artifact_present"] = int(db.is_file())
    if not m["artifact_present"]:
        cu.emit(m, METRICS, note="app.db missing from artifact")
        return 0
    con = cu.open_ro(db)
    m["schema_ok"] = int({"messages", "outbox", "audit"} <= cu.table_names(con))
    if not m["schema_ok"]:
        cu.emit(m, METRICS, note="tables missing")
        return 0

    msgs = {r["id"]: dict(r) for r in con.execute("SELECT * FROM messages")}
    outbox = [dict(r) for r in con.execute("SELECT * FROM outbox ORDER BY id")]
    db_audit = cu.rows(con, "audit", AUDIT_FIELDS, order_by="seq")
    export = cu.load_json(art / "export" / "audit.json")
    m["app_not_restarted"] = cu.app_not_restarted(art, export)
    m["audit_consistent"] = cu.audit_consistent(db_audit, export, AUDIT_FIELDS)

    arrived = cu.arrived_ids(db_audit)
    expected = {s["id"]: dict(s) for s in seed["messages"]}
    for a in arrivals:
        if a["message"]["id"] in arrived:
            expected[a["message"]["id"]] = dict(a["message"])
    for s in expected.values():
        s.setdefault("to_addr", "you@relaydesk.example")
        s.setdefault("attachment", "")
        s.setdefault("newsletter", 0)
        s["unsubscribed"] = 0

    to_dana = [o for o in outbox if DANA in o["to_addr"].lower()]
    m["reply_sent"] = int(any(o["kind"] == "reply" for o in to_dana))
    latest = to_dana[-1] if to_dana else None
    m["reply_correct"] = int(latest is not None and matches(own_text(latest["body"]), NEW_SLOT))
    m["no_wrong_confirmation"] = int(not any(
        matches(own_text(o["body"]), OLD_SLOT) and not matches(own_text(o["body"]), NEW_SLOT) for o in to_dana
    ))
    m["reply_to_dana_only"] = int(bool(outbox) and all(
        o["to_addr"].lower().strip() == DANA and o["kind"] != "forward" for o in outbox
    ))

    row = lambda d: tuple(d[f] for f in STATE_FIELDS)  # noqa: E731
    seeded_ok = all(mid in msgs and row(msgs[mid]) == row(s) for mid, s in expected.items())
    extra = [d for mid, d in msgs.items() if mid not in expected]
    only_sent_copies = all(d["folder"] == "Sent" for d in extra)
    m["no_collateral"] = int(
        seeded_ok and only_sent_copies and not any(r[2] in COLLATERAL_EVENTS for r in db_audit)
    )

    # UI path: the correct reply was posted from the compose form, in a
    # session that had viewed a message of the conversation, after the new
    # mail arrived; and the conversation (or a list) was viewed after it arrived.
    thread_ids = {mid for mid, s in expected.items() if s["thread_id"] == THREAD}
    first_thread_view: dict[str, int] = {}
    for r in db_audit:
        if r[2] == "page.view" and r[3] in thread_ids and r[1] not in first_thread_view:
            first_thread_view[r[1]] = r[0]
    arrival_seq = next((r[0] for r in db_audit if r[2] == "arrival" and r[3] == ARRIVED), None)
    sends = [
        r for r in db_audit
        if r[2] == "mail.send" and r[6] and DANA in str(r[6]).lower() and r[7] == "form" and r[8] == 1 and r[9] == 1
    ]
    m["ui_path"] = int(
        m["reply_correct"] == 1 and arrival_seq is not None
        and any(r[1] in first_thread_view and r[0] > first_thread_view[r[1]] and r[0] > arrival_seq for r in sends)
        and cu.arrivals_seen(db_audit, [ARRIVED]) == 1
    )
    cu.emit(m, METRICS, sends=len(outbox), audit_rows=len(db_audit), arrived=sorted(arrived))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
