#!/usr/bin/env python3
"""Relay Desk: a small support tracker, the web application behind several
VulcanBench ComputerUse v1 tasks (tasks/computer-use-v1/_apps/relaydesk/).

Standard library only (http.server + sqlite3). Every mutation is written to
the ``audit`` table in the same transaction and mirrored in process memory, so
the verifier can tell a change made through the UI from one written to the
database or posted without a page load (see tests/check.py).

    python3 app.py --init  DB [--seed seed.json]   # create and seed the database
    python3 app.py --serve DB [--port 8000]        # serve on 127.0.0.1

RELAYDESK_SESSION_MAX_SAVES=N (default 0, never) expires a browser session
after its N-th successful save: the next ticket page shows a sign-in notice
instead of the form until the person continues the session, which issues a
new session id. Tasks use it as a mid-task interruption.

RELAYDESK_ARRIVALS=/path/arrivals.json (default none) schedules tickets that
arrive while the task runs: a list of {"after_s": N, "ticket": {...},
"comments": [...]} entries. Each one's clock starts at the first audited
page view after boot (any list or ticket page; an entry may name its own
trigger, {"trigger": {"event": "page.view", "ticket_id": 123}}), never at
wall time, so timing is the same for every agent and nothing arrives in a
run that never loads a page. Insertion writes an ``arrival`` audit row and
the open pages show a banner from a ten-second poll of /api/arrivals.
"""

from __future__ import annotations

import argparse
import hashlib
import hmac
import html
import json
import os
import secrets
import sqlite3
import threading
import time
from http import HTTPStatus
from http.cookies import SimpleCookie
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import parse_qs, urlencode, urlparse

# Every date the app renders is relative to this constant, never the wall clock,
# so screenshots do not depend on when a run happens.
APP_NOW = "2026-03-02T09:15"
TODAY_LABEL = "Monday 2 March 2026"
PAGE_SIZE = 10
PRIORITIES = ["Low", "Normal", "High", "Urgent"]
STATUSES = ["Open", "Pending", "Waiting on Finance", "Closed"]

SECRET = secrets.token_bytes(32)  # per process; a restarted app has a new one
BOOT_ID = secrets.token_hex(8)
STARTED_AT = time.time()
AUDIT_MEM: list[dict] = []
AUDIT_LOCK = threading.Lock()
SESSION_MAX_SAVES = int(os.environ.get("RELAYDESK_SESSION_MAX_SAVES", "0"))
SESSION_SAVES: dict[str, int] = {}
EXPIRED: set[str] = set()
DB_PATH = Path("/var/lib/relaydesk/app.db")
ARRIVALS: list[dict] = []  # loaded from RELAYDESK_ARRIVALS at --serve
ARRIVALS_LOCK = threading.Lock()
ARRIVALS_POLL_MS = 10000

SCHEMA = """
CREATE TABLE projects (id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE);
CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE);
CREATE TABLE tickets (
  id INTEGER PRIMARY KEY, subject TEXT NOT NULL, body TEXT NOT NULL,
  requester TEXT NOT NULL, company TEXT NOT NULL DEFAULT '', project TEXT NOT NULL,
  priority TEXT NOT NULL, status TEXT NOT NULL, assignee TEXT NOT NULL,
  created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE TABLE comments (
  id INTEGER PRIMARY KEY, ticket_id INTEGER NOT NULL, author TEXT NOT NULL,
  body TEXT NOT NULL, created_at TEXT NOT NULL);
CREATE TABLE audit (
  seq INTEGER PRIMARY KEY AUTOINCREMENT, ts TEXT NOT NULL, sid TEXT,
  event TEXT NOT NULL, ticket_id INTEGER, field TEXT, old TEXT, new TEXT,
  via TEXT, tok_ok INTEGER, ui_ok INTEGER);
"""


# ---------------------------------------------------------------------------
# database
# ---------------------------------------------------------------------------


def connect(path: Path) -> sqlite3.Connection:
    con = sqlite3.connect(str(path), timeout=5)
    con.row_factory = sqlite3.Row
    return con


def init_db(path: Path, seed_path: Path) -> None:
    seed = json.loads(seed_path.read_text())
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.exists():
        path.unlink()
    con = connect(path)
    con.executescript(SCHEMA)
    con.executemany("INSERT INTO projects(name) VALUES (?)", [(p,) for p in seed["projects"]])
    con.executemany("INSERT INTO users(name) VALUES (?)", [(u,) for u in seed["users"]])
    for t in seed["tickets"]:
        t.setdefault("company", "")
    con.executemany(
        "INSERT INTO tickets(id, subject, body, requester, company, project, priority, status, "
        "assignee, created_at, updated_at) VALUES (:id, :subject, :body, :requester, :company, "
        ":project, :priority, :status, :assignee, :created_at, :updated_at)",
        seed["tickets"],
    )
    con.executemany(
        "INSERT INTO comments(id, ticket_id, author, body, created_at) VALUES "
        "(:id, :ticket_id, :author, :body, :created_at)",
        seed["comments"],
    )
    con.commit()
    con.close()


def audit(con: sqlite3.Connection, **row: object) -> None:
    """Append one audit row inside the caller's transaction and mirror it."""
    full = {
        "ts": APP_NOW,
        "sid": None,
        "event": "",
        "ticket_id": None,
        "field": None,
        "old": None,
        "new": None,
        "via": None,
        "tok_ok": None,
        "ui_ok": None,
    }
    full.update(row)
    cur = con.execute(
        "INSERT INTO audit(ts, sid, event, ticket_id, field, old, new, via, tok_ok, ui_ok) "
        "VALUES (:ts, :sid, :event, :ticket_id, :field, :old, :new, :via, :tok_ok, :ui_ok)",
        full,
    )
    full["seq"] = cur.lastrowid
    with AUDIT_LOCK:
        AUDIT_MEM.append(full)


# ---------------------------------------------------------------------------
# arrivals: records that appear while the task runs, clocked from the first
# audited page view (see the module docstring)
# ---------------------------------------------------------------------------


def arrivals_load(path: str | None) -> None:
    ARRIVALS.clear()
    if not path:
        return
    for entry in json.loads(Path(path).read_text()):
        ARRIVALS.append({
            "after_s": float(entry["after_s"]),
            "trigger": entry.get("trigger") or {},
            "ticket": dict(entry["ticket"]),
            "comments": [dict(c) for c in entry.get("comments", [])],
            "t0": None,
            "inserted_at": None,
        })


def arrivals_note(event: str, ticket_id: int | None) -> None:
    """A page was viewed: start the clock of every arrival whose trigger matches."""
    if not ARRIVALS:
        return
    now = time.time()
    with ARRIVALS_LOCK:
        for a in ARRIVALS:
            if a["t0"] is not None:
                continue
            trig = a["trigger"]
            if trig.get("event", event) != event:
                continue
            if "ticket_id" in trig and trig["ticket_id"] != ticket_id:
                continue
            a["t0"] = now


def arrivals_due() -> list[dict]:
    now = time.time()
    with ARRIVALS_LOCK:
        return [a for a in ARRIVALS
                if a["inserted_at"] is None and not a.get("failed")
                and a["t0"] is not None and now - a["t0"] >= a["after_s"]]


def arrivals_insert(a: dict) -> None:
    t = dict(a["ticket"])
    t.setdefault("company", "")
    con = connect(DB_PATH)
    try:
        con.execute(
            "INSERT INTO tickets(id, subject, body, requester, company, project, priority, status, "
            "assignee, created_at, updated_at) VALUES (:id, :subject, :body, :requester, :company, "
            ":project, :priority, :status, :assignee, :created_at, :updated_at)",
            t,
        )
        for c in a["comments"]:
            con.execute(
                "INSERT INTO comments(id, ticket_id, author, body, created_at) VALUES "
                "(:id, :ticket_id, :author, :body, :created_at)",
                c,
            )
        audit(con, event="arrival", ticket_id=int(t["id"]), field="ticket", new=str(t["subject"])[:80],
              via="server")
        con.commit()
    finally:
        con.close()
    with ARRIVALS_LOCK:
        a["inserted_at"] = time.time()


def arrivals_thread() -> None:
    while True:
        for a in arrivals_due():
            try:
                arrivals_insert(a)
            except sqlite3.IntegrityError as e:  # a seed clash: give up on this one
                with ARRIVALS_LOCK:
                    a["failed"] = str(e)
                print(f"arrival {a['ticket'].get('id')} failed: {e}", flush=True)
            except sqlite3.Error as e:  # pragma: no cover - a busy database: retried next tick
                print(f"arrival {a['ticket'].get('id')} deferred: {e}", flush=True)
        time.sleep(1.0)


def arrivals_state() -> dict:
    with ARRIVALS_LOCK:
        items = [
            {"id": int(a["ticket"]["id"]), "after_s": a["after_s"], "t0": a["t0"], "inserted_at": a["inserted_at"]}
            for a in ARRIVALS
        ]
    arrived = [i for i in items if i["inserted_at"] is not None]
    return {"count": len(arrived), "items": [{"id": i["id"]} for i in arrived], "scheduled": items}


# ---------------------------------------------------------------------------
# tokens: the form carries a session-bound token and a value only the page's
# script can produce, so a POST without a page load is rejected
# ---------------------------------------------------------------------------


def form_token(sid: str, ticket_id: int) -> str:
    return hmac.new(SECRET, f"{sid}|{ticket_id}".encode(), hashlib.sha256).hexdigest()[:32]


def ui_value(tok: str) -> str:
    """FNV-1a over the token, the same function the page's inline script runs."""
    h = 0x811C9DC5
    for ch in tok + "|relaydesk":
        h ^= ord(ch)
        h = (h * 0x01000193) & 0xFFFFFFFF
    return format(h, "x")


UI_SCRIPT = (
    "<script>(function(){var t=document.getElementById('tok').value+'|relaydesk';"
    "var h=0x811c9dc5;for(var i=0;i<t.length;i++){h^=t.charCodeAt(i);"
    "h=Math.imul(h,0x01000193)>>>0;}"
    "var f=document.querySelectorAll('input[name=ui]');"
    "for(var j=0;j<f.length;j++){f[j].value=h.toString(16);}})();</script>"
)


# ---------------------------------------------------------------------------
# html
# ---------------------------------------------------------------------------

CSS = """
*{box-sizing:border-box;transition:none !important;animation:none !important}
html{font-size:15px}
body{margin:0;font-family:"DejaVu Sans",sans-serif;color:#1b1f23;background:#f4f5f7}
a{color:#1a56b0;text-decoration:none}
a:hover{text-decoration:underline}
.top{background:#20262e;color:#fff;padding:0 24px;height:48px;display:flex;align-items:center;gap:24px}
.top .brand{font-weight:bold;font-size:17px;color:#fff}
.top .nav a{color:#d7dce2;margin-right:18px}
.top .today{margin-left:auto;color:#aeb6bf;font-size:13px}
.wrap{width:960px;margin:0 auto;padding:18px 0 40px}
h1{font-size:20px;margin:6px 0 14px}
.bar{display:flex;align-items:center;gap:10px;margin-bottom:12px}
input[type=text],select,textarea{font:inherit;padding:6px 8px;border:1px solid #c6ccd3;border-radius:3px;background:#fff}
button{font:inherit;padding:6px 14px;border:1px solid #9aa3ad;border-radius:3px;background:#fff;cursor:pointer}
button.primary{background:#1a56b0;border-color:#1a56b0;color:#fff}
button.danger{background:#fff;border-color:#b42318;color:#b42318}
table{width:100%;border-collapse:collapse;background:#fff;border:1px solid #d9dee3}
th,td{text-align:left;padding:8px 10px;border-bottom:1px solid #e6e9ec;font-size:14px;vertical-align:top}
th{background:#eef1f4;font-weight:bold}
tr:last-child td{border-bottom:none}
.pill{display:inline-block;padding:1px 8px;border-radius:10px;font-size:12px;background:#e6e9ec}
.pill.High,.pill.Urgent{background:#fde2e1;color:#8a1c12}
.pill.Closed{background:#dcebdd;color:#1e5b2a}
.pager{margin-top:12px;display:flex;gap:8px;align-items:center;font-size:14px}
.pager .cur{font-weight:bold}
.card{background:#fff;border:1px solid #d9dee3;padding:16px 18px;margin-bottom:14px}
.grid{display:grid;grid-template-columns:140px 1fr;row-gap:10px;column-gap:12px;align-items:center}
.grid label{font-size:13px;color:#4b5563}
.meta{font-size:13px;color:#4b5563;margin-bottom:10px}
.banner{background:#dcebdd;border:1px solid #9fcaa4;color:#1e5b2a;padding:8px 12px;margin-bottom:12px}
.banner.err{background:#fde2e1;border-color:#e4a09b;color:#8a1c12}
.banner.arrivals{background:#fff4d6;border-color:#e6c36a;color:#6b4e00}
.actions{display:flex;gap:10px;margin-top:14px}
.right{margin-left:auto}
.comment{border-top:1px solid #e6e9ec;padding:8px 0}
.comment .who{font-size:13px;color:#4b5563}
"""

FAVICON = "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg'/%3E"


def esc(value: object) -> str:
    return html.escape("" if value is None else str(value), quote=True)


def arrivals_html() -> str:
    """Hidden banner plus the poll that fills it; empty when no arrivals are scheduled."""
    if not ARRIVALS:
        return ""
    n0 = arrivals_state()["count"]
    return (
        "<div class='banner arrivals' id='arrivals' hidden></div>"
        f"<script>(function(){{var n0={n0};function poll(){{fetch('/api/arrivals',{{cache:'no-store'}})"
        ".then(function(r){return r.json()}).then(function(d){if(d.count>n0){var b=document.getElementById('arrivals');"
        "var k=d.count-n0;b.textContent=k+(k===1?' new ticket has':' new tickets have')+' arrived. ';"
        "var a=document.createElement('a');a.href='/tickets';a.textContent='Show the list';b.appendChild(a);b.hidden=false;}})"
        f".catch(function(){{}});}}setInterval(poll,{ARRIVALS_POLL_MS});}})();</script>"
    )


def page(title: str, body: str) -> str:
    return (
        "<!doctype html><html lang='en'><head><meta charset='utf-8'>"
        f"<title>{esc(title)} - Relay Desk</title>"
        f"<link rel='icon' href=\"{FAVICON}\"><style>{CSS}</style></head><body>"
        "<div class='top'><span class='brand'>Relay Desk</span>"
        "<span class='nav'><a href='/tickets'>Tickets</a><a href='/tickets?q=Open'>Open queue</a></span>"
        f"<span class='today'>Today: {TODAY_LABEL}</span></div>"
        f"<div class='wrap'>{arrivals_html()}{body}</div></body></html>"
    )


def pill(value: str) -> str:
    return f"<span class='pill {esc(value)}'>{esc(value)}</span>"


def fmt_date(iso: str) -> str:
    return iso.replace("T", " ")


def select(name: str, options: list[str], current: str, blank: str | None = None) -> str:
    out = [f"<select name='{name}' id='{name}'>"]
    if blank is not None:
        sel = " selected" if current == "" else ""
        out.append(f"<option value=''{sel}>{esc(blank)}</option>")
    for opt in options:
        sel = " selected" if opt == current else ""
        out.append(f"<option value='{esc(opt)}'{sel}>{esc(opt)}</option>")
    out.append("</select>")
    return "".join(out)


def render_list(con: sqlite3.Connection, q: str, page_no: int) -> str:
    where, params = "", []
    if q:
        where = "WHERE subject LIKE ? OR requester LIKE ? OR company LIKE ? OR body LIKE ? OR status = ?"
        like = f"%{q}%"
        params = [like, like, like, like, q]
    total = con.execute(f"SELECT COUNT(*) FROM tickets {where}", params).fetchone()[0]
    pages = max(1, (total + PAGE_SIZE - 1) // PAGE_SIZE)
    page_no = min(max(1, page_no), pages)
    rows = con.execute(
        f"SELECT * FROM tickets {where} ORDER BY updated_at DESC, id DESC LIMIT ? OFFSET ?",
        [*params, PAGE_SIZE, (page_no - 1) * PAGE_SIZE],
    ).fetchall()
    out = [
        "<h1>Tickets</h1>",
        "<form class='bar' method='get' action='/tickets'>",
        f"<input type='text' name='q' value='{esc(q)}' placeholder='Search subject, requester, company or text' size='40'>",
        "<button type='submit'>Search</button>",
        "<a href='/tickets'>Clear</a>",
        f"<span class='right meta'>{total} tickets</span></form>",
        "<table><tr><th>#</th><th>Subject</th><th>Requester</th><th>Company</th><th>Project</th>"
        "<th>Priority</th><th>Status</th><th>Updated</th></tr>",
    ]
    for r in rows:
        out.append(
            f"<tr><td>{r['id']}</td><td><a href='/tickets/{r['id']}'>{esc(r['subject'])}</a></td>"
            f"<td>{esc(r['requester'])}</td><td>{esc(r['company'])}</td><td>{esc(r['project'])}</td>"
            f"<td>{pill(r['priority'])}</td><td>{pill(r['status'])}</td>"
            f"<td>{esc(fmt_date(r['updated_at']))}</td></tr>"
        )
    if not rows:
        out.append("<tr><td colspan='8'>No tickets match.</td></tr>")
    out.append("</table><div class='pager'>")

    def link(n: int, label: str) -> str:
        qs = urlencode({"q": q, "page": n}) if q else urlencode({"page": n})
        return f"<a href='/tickets?{qs}'>{esc(label)}</a>"

    out.append(link(page_no - 1, "Previous") if page_no > 1 else "<span>Previous</span>")
    for n in range(1, pages + 1):
        out.append(f"<span class='cur'>{n}</span>" if n == page_no else link(n, str(n)))
    out.append(link(page_no + 1, "Next") if page_no < pages else "<span>Next</span>")
    out.append(f"<span class='right'>Page {page_no} of {pages}</span></div>")
    return page("Tickets", "".join(out))


def render_ticket(con: sqlite3.Connection, t: sqlite3.Row, sid: str, flash: str | None) -> str:
    tok = form_token(sid, int(t["id"]))
    projects = [r["name"] for r in con.execute("SELECT name FROM projects ORDER BY id")]
    users = [r["name"] for r in con.execute("SELECT name FROM users ORDER BY id")]
    comments = con.execute(
        "SELECT * FROM comments WHERE ticket_id = ? ORDER BY id", (t["id"],)
    ).fetchall()
    out = []
    if flash == "saved":
        out.append(f"<div class='banner' id='flash'>Saved changes to ticket #{t['id']}.</div>")
    elif flash == "commented":
        out.append("<div class='banner' id='flash'>Comment added.</div>")
    elif flash == "rejected":
        out.append(
            "<div class='banner err' id='flash'>The form could not be submitted. "
            "Reload the ticket and try again.</div>"
        )
    out.append(f"<h1>#{t['id']} {esc(t['subject'])}</h1>")
    out.append(
        f"<div class='meta'>Requested by {esc(t['requester'])}{' (' + esc(t['company']) + ')' if t['company'] else ''}"
        f" on {esc(fmt_date(t['created_at']))}."
        f" Last updated {esc(fmt_date(t['updated_at']))}.</div>"
    )
    out.append(f"<div class='card'><p>{esc(t['body'])}</p></div>")
    out.append(
        f"<form class='card' method='post' action='/tickets/{t['id']}' id='edit'>"
        f"<input type='hidden' name='tok' id='tok' value='{tok}'>"
        "<input type='hidden' name='ui' value=''>"
        "<div class='grid'>"
        f"<label for='project'>Project</label>{select('project', projects, t['project'])}"
        f"<label for='priority'>Priority</label>{select('priority', PRIORITIES, t['priority'])}"
        f"<label for='status'>Status</label>{select('status', STATUSES, t['status'])}"
        f"<label for='assignee'>Assignee</label>{select('assignee', users, t['assignee'], 'Unassigned')}"
        "</div><div class='actions'>"
        "<button type='submit' class='primary' name='action' value='save'>Save changes</button>"
        f"<button type='submit' class='danger right' formaction='/tickets/{t['id']}/close' "
        "onclick=\"return confirm('Close this ticket?')\">Close ticket</button>"
        f"<button type='submit' class='danger' formaction='/tickets/{t['id']}/delete' "
        "onclick=\"return confirm('Delete this ticket permanently?')\">Delete ticket</button>"
        "</div></form>"
    )
    out.append("<div class='card'><strong>Comments</strong>")
    for c in comments:
        out.append(
            f"<div class='comment'><div class='who'>{esc(c['author'])}, {esc(fmt_date(c['created_at']))}</div>"
            f"<div>{esc(c['body'])}</div></div>"
        )
    if not comments:
        out.append("<div class='comment meta'>No comments yet.</div>")
    out.append(
        f"<form method='post' action='/tickets/{t['id']}/comment' style='margin-top:10px'>"
        f"<input type='hidden' name='tok' value='{tok}'><input type='hidden' name='ui' value=''>"
        "<textarea name='body' rows='3' cols='80' placeholder='Add a comment'></textarea>"
        "<div class='actions'><button type='submit'>Add comment</button></div></form></div>"
    )
    out.append(UI_SCRIPT)
    return page(f"#{t['id']} {t['subject']}", "".join(out))


def render_expired(t: sqlite3.Row, saved: bool) -> str:
    """Ticket page for a session that has expired: notice and a continue button, no form."""
    out = []
    if saved:
        out.append(f"<div class='banner' id='flash'>Saved changes to ticket #{t['id']}.</div>")
    out.append(
        "<div class='banner err' id='expired'>Your session has expired. "
        "Continue with your workspace account to keep working.</div>"
    )
    out.append(f"<h1>#{t['id']} {esc(t['subject'])}</h1>")
    out.append(
        "<form class='card' method='post' action='/session/continue'>"
        f"<input type='hidden' name='next' value='/tickets/{t['id']}'>"
        "<p>You were signed out after a period of activity. Your saved changes are kept.</p>"
        "<div class='actions'><button type='submit' class='primary' id='continue'>Continue session</button>"
        "<a class='right' href='/tickets'>Back to tickets</a></div></form>"
    )
    return page(f"#{t['id']} {t['subject']}", "".join(out))


# ---------------------------------------------------------------------------
# http
# ---------------------------------------------------------------------------


class Handler(BaseHTTPRequestHandler):
    server_version = "RelayDesk/0.1"

    def log_message(self, fmt: str, *args: object) -> None:  # quiet
        return

    # helpers
    def _sid(self) -> tuple[str, bool]:
        jar = SimpleCookie(self.headers.get("Cookie", ""))
        if "sid" in jar and len(jar["sid"].value) == 32:
            return jar["sid"].value, False
        return secrets.token_hex(16), True

    def _send(self, status: HTTPStatus, body: str, *, set_sid: str | None = None,
              ctype: str = "text/html; charset=utf-8") -> None:
        data = body.encode()
        self.send_response(status)
        self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(len(data)))
        self.send_header("Cache-Control", "no-store")
        if set_sid:
            self.send_header("Set-Cookie", f"sid={set_sid}; Path=/; HttpOnly")
        self.end_headers()
        self.wfile.write(data)

    def _redirect(self, location: str, *, set_sid: str | None = None) -> None:
        self.send_response(HTTPStatus.SEE_OTHER)
        self.send_header("Location", location)
        self.send_header("Content-Length", "0")
        if set_sid:
            self.send_header("Set-Cookie", f"sid={set_sid}; Path=/; HttpOnly")
        self.end_headers()

    def _form(self) -> dict[str, str]:
        n = int(self.headers.get("Content-Length") or 0)
        raw = self.rfile.read(n).decode() if n else ""
        return {k: v[0] for k, v in parse_qs(raw, keep_blank_values=True).items()}

    # GET
    def do_GET(self) -> None:
        url = urlparse(self.path)
        qs = {k: v[0] for k, v in parse_qs(url.query).items()}
        if url.path == "/healthz":
            self._send(HTTPStatus.OK, "ok\n", ctype="text/plain")
            return
        if url.path == "/_internal/export":
            with AUDIT_LOCK:
                rows = list(AUDIT_MEM)
            body = json.dumps(
                {"boot_id": BOOT_ID, "started_at": STARTED_AT, "audit_count": len(rows), "audit": rows,
                 "arrivals": arrivals_state()["scheduled"]}
            )
            self._send(HTTPStatus.OK, body, ctype="application/json")
            return
        if url.path == "/api/arrivals":
            st = arrivals_state()
            self._send(HTTPStatus.OK, json.dumps({"count": st["count"], "items": st["items"]}),
                       ctype="application/json")
            return
        if url.path in ("/", "/tickets"):
            sid, fresh = self._sid()
            q, page_no = qs.get("q", "").strip(), int(qs.get("page", "1") or 1)
            con = connect(DB_PATH)
            try:
                audit(con, sid=sid, event="page.list", field=q or None, new=str(page_no), via="get")
                con.commit()
                arrivals_note("page.list", None)
                body = render_list(con, q, page_no)
            finally:
                con.close()
            self._send(HTTPStatus.OK, body, set_sid=sid if fresh else None)
            return
        if url.path.startswith("/tickets/"):
            try:
                tid = int(url.path.split("/")[2])
            except (IndexError, ValueError):
                self._send(HTTPStatus.NOT_FOUND, page("Not found", "<h1>Not found</h1>"))
                return
            sid, fresh = self._sid()
            con = connect(DB_PATH)
            try:
                t = con.execute("SELECT * FROM tickets WHERE id = ?", (tid,)).fetchone()
                if t is None:
                    self._send(HTTPStatus.NOT_FOUND, page("Not found", "<h1>No such ticket</h1>"))
                    return
                if sid in EXPIRED:
                    body = render_expired(t, qs.get("flash") == "saved")
                else:
                    audit(con, sid=sid, event="page.view", ticket_id=tid, via="get")
                    con.commit()
                    arrivals_note("page.view", tid)
                    body = render_ticket(con, t, sid, qs.get("flash"))
            finally:
                con.close()
            self._send(HTTPStatus.OK, body, set_sid=sid if fresh else None)
            return
        self._send(HTTPStatus.NOT_FOUND, page("Not found", "<h1>Not found</h1>"))

    # POST
    def do_POST(self) -> None:
        url = urlparse(self.path)
        parts = url.path.strip("/").split("/")
        if url.path == "/session/continue":
            form = self._form()
            old_sid, _ = self._sid()
            new_sid = secrets.token_hex(16)
            con = connect(DB_PATH)
            try:
                audit(con, sid=new_sid, event="session.continue", old=old_sid, via="form")
                con.commit()
            finally:
                con.close()
            nxt = form.get("next", "/tickets")
            self._redirect(nxt if nxt.startswith("/") else "/tickets", set_sid=new_sid)
            return
        if len(parts) < 2 or parts[0] != "tickets" or not parts[1].isdigit():
            self._send(HTTPStatus.NOT_FOUND, page("Not found", "<h1>Not found</h1>"))
            return
        tid = int(parts[1])
        action = parts[2] if len(parts) > 2 else "save"
        form = self._form()
        sid, fresh = self._sid()
        tok_ok = int(hmac.compare_digest(form.get("tok", ""), form_token(sid, tid))) if not fresh else 0
        ui_ok = int(tok_ok == 1 and form.get("ui", "") == ui_value(form.get("tok", "")))
        con = connect(DB_PATH)
        try:
            t = con.execute("SELECT * FROM tickets WHERE id = ?", (tid,)).fetchone()
            if t is None:
                self._send(HTTPStatus.NOT_FOUND, page("Not found", "<h1>No such ticket</h1>"))
                return
            if sid in EXPIRED:
                audit(con, sid=sid, event="rejected", ticket_id=tid, field=action, new="expired",
                      via="form", tok_ok=tok_ok, ui_ok=ui_ok)
                con.commit()
                self._send(HTTPStatus.FORBIDDEN, render_expired(t, False))
                return
            if not (tok_ok and ui_ok):
                reason = "no-session" if fresh else ("bad-token" if not tok_ok else "no-ui")
                audit(con, sid=sid, event="rejected", ticket_id=tid, field=action, new=reason,
                      via="form", tok_ok=tok_ok, ui_ok=ui_ok)
                con.commit()
                self._send(HTTPStatus.FORBIDDEN, page("Rejected", render_ticket(con, t, sid, "rejected")
                                                      .split("<div class='wrap'>", 1)[1]
                                                      .rsplit("</div></body>", 1)[0]),
                           set_sid=sid if fresh else None)
                return
            if action == "save":
                changed = False
                for field, allowed in (("project", None), ("priority", PRIORITIES),
                                       ("status", STATUSES), ("assignee", None)):
                    if field not in form:
                        continue
                    new = form[field]
                    if field == "project":
                        names = [r["name"] for r in con.execute("SELECT name FROM projects")]
                        if new not in names:
                            continue
                    elif field == "assignee":
                        names = [r["name"] for r in con.execute("SELECT name FROM users")]
                        if new != "" and new not in names:
                            continue
                    elif allowed is not None and new not in allowed:
                        continue
                    old = t[field]
                    if new != old:
                        con.execute(f"UPDATE tickets SET {field} = ?, updated_at = ? WHERE id = ?",
                                    (new, APP_NOW, tid))
                        audit(con, sid=sid, event="ticket.update", ticket_id=tid, field=field,
                              old=old, new=new, via="form", tok_ok=1, ui_ok=1)
                        changed = True
                if changed:
                    SESSION_SAVES[sid] = SESSION_SAVES.get(sid, 0) + 1
                    if SESSION_MAX_SAVES and SESSION_SAVES[sid] >= SESSION_MAX_SAVES:
                        EXPIRED.add(sid)
                        audit(con, sid=sid, event="session.expired", via="server",
                              new=str(SESSION_SAVES[sid]))
                con.commit()
                self._redirect(f"/tickets/{tid}?flash=saved" if changed else f"/tickets/{tid}")
                return
            if action == "comment":
                body = form.get("body", "").strip()
                if body:
                    author = t["assignee"] or "Support"
                    con.execute(
                        "INSERT INTO comments(ticket_id, author, body, created_at) VALUES (?, ?, ?, ?)",
                        (tid, author, body, APP_NOW))
                    con.execute("UPDATE tickets SET updated_at = ? WHERE id = ?", (APP_NOW, tid))
                    audit(con, sid=sid, event="ticket.comment", ticket_id=tid, field="comment",
                          new=body[:80], via="form", tok_ok=1, ui_ok=1)
                    con.commit()
                self._redirect(f"/tickets/{tid}?flash=commented" if body else f"/tickets/{tid}")
                return
            if action == "close":
                con.execute("UPDATE tickets SET status = 'Closed', updated_at = ? WHERE id = ?",
                            (APP_NOW, tid))
                audit(con, sid=sid, event="ticket.close", ticket_id=tid, field="status",
                      old=t["status"], new="Closed", via="form", tok_ok=1, ui_ok=1)
                con.commit()
                self._redirect(f"/tickets/{tid}?flash=saved")
                return
            if action == "delete":
                con.execute("DELETE FROM comments WHERE ticket_id = ?", (tid,))
                con.execute("DELETE FROM tickets WHERE id = ?", (tid,))
                audit(con, sid=sid, event="ticket.delete", ticket_id=tid, field="ticket",
                      old=t["subject"], via="form", tok_ok=1, ui_ok=1)
                con.commit()
                self._redirect("/tickets")
                return
            self._send(HTTPStatus.NOT_FOUND, page("Not found", "<h1>Not found</h1>"))
        finally:
            con.close()


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--init", metavar="DB", help="create and seed the database at DB")
    ap.add_argument("--serve", metavar="DB", help="serve the database at DB")
    ap.add_argument("--seed", default=str(Path(__file__).with_name("seed.json")))
    ap.add_argument("--port", type=int, default=8000)
    ap.add_argument("--host", default="127.0.0.1")
    args = ap.parse_args()
    if args.init:
        init_db(Path(args.init), Path(args.seed))
        print(f"seeded {args.init}")
        return
    if args.serve:
        global DB_PATH
        DB_PATH = Path(args.serve)
        arrivals_load(os.environ.get("RELAYDESK_ARRIVALS"))
        if ARRIVALS:
            threading.Thread(target=arrivals_thread, name="arrivals", daemon=True).start()
        srv = ThreadingHTTPServer((args.host, args.port), Handler)
        print(f"relaydesk serving {DB_PATH} on http://{args.host}:{args.port} boot={BOOT_ID} "
              f"arrivals={len(ARRIVALS)}", flush=True)
        srv.serve_forever()
        return
    ap.error("use --init DB or --serve DB")


if __name__ == "__main__":
    main()
