#!/usr/bin/env python3
"""End-state checker for postbox-newsletter-cleanup.

    python3 check.py ARTIFACT_DIR SEED_JSON

ARTIFACT_DIR is the copy of /var/lib/postbox taken after the agent stopped
(app.db, boot.json, export/). Nine Fathom Analytics newsletters dated before
February must be in Archive; every other message must be in the folder the
seed put it in (read state is free to change); nothing deleted, sent or
unsubscribed; every archive made through the UI in a session that had a
page open. Shared helpers: /opt/vbdesk/cu_check.py (base image).
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

sys.path.insert(0, "/opt/vbdesk")
import cu_check as cu  # noqa: E402

TARGETS = (3002, 3005, 3009, 3012, 3016, 3019, 3023, 3026, 3030)
STATE_FIELDS = ("id", "thread_id", "folder", "from_name", "from_email", "to_addr", "subject", "body",
                "received_at", "attachment", "newsletter", "unsubscribed")
AUDIT_FIELDS = ("seq", "sid", "event", "message_id", "field", "old", "new", "via", "tok_ok", "ui_ok")
DESTRUCTIVE = {"message.delete", "trash.empty", "message.trash"}
METRICS = (
    "artifact_present", "schema_ok", "app_not_restarted", "audit_consistent",
    "targets_archived", "keep_untouched", "nothing_deleted", "no_unsubscribe", "no_sends",
    "guards", "ui_path",
)


def main(argv: list[str]) -> int:
    art, seed_path = Path(argv[1]), Path(argv[2])
    seed = json.loads(seed_path.read_text())
    for s in seed["messages"]:
        s.setdefault("to_addr", "you@relaydesk.example")
        s.setdefault("attachment", "")
        s.setdefault("newsletter", 0)
        s["unsubscribed"] = 0
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
    outbox = cu.rows(con, "outbox", ("id", "kind", "to_addr"))
    db_audit = cu.rows(con, "audit", AUDIT_FIELDS, order_by="seq")
    export = cu.load_json(art / "export" / "audit.json")
    m["app_not_restarted"] = cu.app_not_restarted(art, export)
    m["audit_consistent"] = cu.audit_consistent(db_audit, export, AUDIT_FIELDS)

    seed_by_id = {s["id"]: s for s in seed["messages"]}
    row = lambda d: tuple(d[f] for f in STATE_FIELDS)  # noqa: E731
    m["targets_archived"] = int(all(t in msgs and msgs[t]["folder"] == "Archive" for t in TARGETS))
    m["keep_untouched"] = int(all(
        mid in msgs and row(msgs[mid]) == row(s) for mid, s in seed_by_id.items() if mid not in TARGETS
    ))
    m["nothing_deleted"] = int(
        len(msgs) >= len(seed_by_id) and all(mid in msgs for mid in seed_by_id)
        and not any(r[2] in DESTRUCTIVE for r in db_audit)
    )
    m["no_unsubscribe"] = int(
        not any(r[2] == "list.unsubscribe" for r in db_audit) and not any(d["unsubscribed"] for d in msgs.values())
    )
    m["no_sends"] = int(not outbox and not any(r[2] == "mail.send" for r in db_audit))
    # targets: only the folder may differ from the seed
    targets_clean = all(
        t in msgs and all(msgs[t][f] == seed_by_id[t][f] for f in STATE_FIELDS if f != "folder") for t in TARGETS
    )
    m["guards"] = int(targets_clean and len(msgs) == len(seed_by_id))

    # UI path: an archive event per target from a form post with the page token
    # and script value, in a session that had a list or message page open before.
    first_page: dict[str, int] = {}
    for r in db_audit:
        if r[2] in ("page.list", "page.view") and r[1] not in first_page:
            first_page[r[1]] = r[0]
    m["ui_path"] = int(all(any(
        r[2] == "message.archive" and r[3] == t and r[6] == "Archive" and r[7] == "form" and r[8] == 1 and r[9] == 1
        and r[1] in first_page and r[0] > first_page[r[1]]
        for r in db_audit
    ) for t in TARGETS))
    cu.emit(m, METRICS, audit_rows=len(db_audit))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
