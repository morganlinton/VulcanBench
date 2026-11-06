#!/usr/bin/env python3
"""End-state checker for relaydesk-invoice-ticket-triage.

    python3 check.py ARTIFACT_DIR SEED_JSON

ARTIFACT_DIR is the copy of /var/lib/relaydesk taken after the agent stopped:
app.db (the application's SQLite file) and export/ (written by the collect
hook while the app was still running: the in-memory audit mirror and process
start times). Prints one JSON object with the reward and every sub-metric as
flat integers; reward is 1 only when every sub-metric is 1.
"""

from __future__ import annotations

import json
import sqlite3
import sys
from pathlib import Path

TARGET = 1017
WANT = {"project": "Billing", "priority": "High"}
UNCHANGED_ON_TARGET = ("subject", "body", "requester", "status", "assignee", "created_at")
TICKET_FIELDS = (
    "id", "subject", "body", "requester", "project", "priority", "status",
    "assignee", "created_at", "updated_at",
)
COMMENT_FIELDS = ("id", "ticket_id", "author", "body", "created_at")
AUDIT_FIELDS = ("seq", "sid", "event", "ticket_id", "field", "old", "new", "via", "tok_ok", "ui_ok")
MUTATIONS = {"ticket.update", "ticket.comment", "ticket.close", "ticket.delete"}
RESTART_WINDOW_S = 20.0
METRICS = (
    "artifact_present", "schema_ok", "app_not_restarted", "audit_consistent",
    "target_project", "target_priority", "target_other_fields", "guards", "ui_path",
)


def emit(m: dict[str, int], **extra: object) -> None:
    out = {"reward": int(all(m.get(k) == 1 for k in METRICS))}
    out.update({k: int(m.get(k, 0)) for k in METRICS})
    out.update(extra)
    print(json.dumps(out, sort_keys=True))


def load_json(path: Path) -> dict | None:
    try:
        return json.loads(path.read_text())
    except (OSError, ValueError):
        return None


def main(argv: list[str]) -> int:
    art, seed_path = Path(argv[1]), Path(argv[2])
    seed = json.loads(seed_path.read_text())
    m: dict[str, int] = {k: 0 for k in METRICS}
    db = art / "app.db"
    m["artifact_present"] = int(db.is_file())
    if not m["artifact_present"]:
        emit(m, note="app.db missing from artifact")
        return 0

    con = sqlite3.connect(f"file:{db}?mode=ro", uri=True)
    con.row_factory = sqlite3.Row
    names = {r["name"] for r in con.execute("SELECT name FROM sqlite_master WHERE type='table'")}
    m["schema_ok"] = int({"projects", "users", "tickets", "comments", "audit"} <= names)
    if not m["schema_ok"]:
        emit(m, note="tables missing")
        return 0

    tickets = {r["id"]: dict(r) for r in con.execute("SELECT * FROM tickets")}
    comments = [tuple(dict(r)[k] for k in COMMENT_FIELDS) for r in con.execute("SELECT * FROM comments ORDER BY id")]
    projects = [r["name"] for r in con.execute("SELECT name FROM projects ORDER BY id")]
    users = [r["name"] for r in con.execute("SELECT name FROM users ORDER BY id")]
    db_audit = [tuple(dict(r)[k] for k in AUDIT_FIELDS) for r in con.execute("SELECT * FROM audit ORDER BY seq")]

    # process liveness: the app the entrypoint started is the one that served the run
    proc = load_json(art / "export" / "process.json") or {}
    export = load_json(art / "export" / "audit.json") or {}
    boot = load_json(art / "boot.json") or {}
    try:
        gap = (float(proc["app_start_ticks"]) - float(proc["pid1_start_ticks"])) / float(proc["clk_tck"])
        same_boot = bool(boot.get("boot_id")) and boot.get("boot_id") == export.get("boot_id")
        m["app_not_restarted"] = int(
            proc.get("app_pid") is not None and 0 <= gap <= RESTART_WINDOW_S and same_boot
        )
    except (KeyError, TypeError, ValueError):
        m["app_not_restarted"] = 0

    # the database audit log must match what the live process remembers
    mem_audit = [tuple(row.get(k) for k in AUDIT_FIELDS) for row in export.get("audit", [])]
    m["audit_consistent"] = int("audit" in export and mem_audit == db_audit)

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
    comments_ok = comments == [tuple(c[f] for f in COMMENT_FIELDS) for c in seed["comments"]]
    refs_ok = projects == seed["projects"] and users == seed["users"]
    events_ok = all(
        not (row[2] in MUTATIONS and row[3] != TARGET) and row[2] not in {"ticket.close", "ticket.delete"}
        for row in db_audit
    )
    m["guards"] = int(others_ok and counts_ok and comments_ok and refs_ok and events_ok)

    # the change was made through the UI: a page view of the ticket in a session,
    # then form updates in that session carrying the page's token and script value
    viewed = {row[1] for row in db_audit if row[2] == "page.view" and row[3] == TARGET}
    view_seq: dict[str, int] = {}
    for row in db_audit:  # first view of the target per session
        if row[2] == "page.view" and row[3] == TARGET and row[1] not in view_seq:
            view_seq[row[1]] = row[0]

    def via_ui(field: str, value: str) -> bool:
        return any(
            row[2] == "ticket.update" and row[3] == TARGET and row[4] == field and row[6] == value
            and row[7] == "form" and row[8] == 1 and row[9] == 1 and row[1] in viewed
            and row[0] > view_seq.get(row[1], 10**9)
            for row in db_audit
        )

    m["ui_path"] = int(via_ui("project", WANT["project"]) and via_ui("priority", WANT["priority"]))
    emit(m, audit_rows=len(db_audit))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
