#!/usr/bin/env python3
"""Postbox: a small webmail client, the second in-house web application behind
VulcanBench ComputerUse v1 tasks (tasks/computer-use-v1/_apps/postbox/).

Standard library only (http.server + sqlite3). Folders, a message view that
shows the whole conversation, reply and forward composition that lands in an
outbox table (nothing leaves the container), archive, delete, empty trash and
newsletter unsubscribe. Every mutation is written to the ``audit`` table in
the same transaction and mirrored in process memory, with the same
session-token and page-script design as Relay Desk, so a checker can tell a
change made through the UI from one written to the database or posted
without a page load.

    python3 app.py --init  DB [--seed seed.json]   # create and seed the database
    python3 app.py --serve DB [--port 8010]        # serve on 127.0.0.1

POSTBOX_ARRIVALS=/path/arrivals.json (default none) schedules messages that
arrive while the task runs: a list of {"after_s": N, "message": {...}}
entries, each clocked from the first audited page view after boot (any
folder, search or message page; an entry may name its own trigger,
{"trigger": {"event": "page.view", "message_id": 4204}}), never from wall
time. Insertion writes an ``arrival`` audit row; open pages show a banner
from a ten-second poll of /api/arrivals, worded for the conversation on
screen when the new message belongs to it.
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
from datetime import datetime, timedelta
from http import HTTPStatus
from http.cookies import SimpleCookie
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import parse_qs, urlencode, urlparse

# Every date the app renders comes from this constant or from the seed, never
# from the wall clock, so screenshots do not depend on when a run happens.
# Records the person creates are stamped by next_stamp(), one minute after
# the newest mail in the mailbox (arrivals included), so a reply never sorts
# before the message it answers.
APP_NOW = "2026-03-02T09:15"
TODAY_LABEL = "Monday 2 March 2026"
PAGE_SIZE = 12
FOLDERS = ["Inbox", "Newsletters", "Archive", "Sent", "Trash"]
ME = "you@relaydesk.example"
ME_NAME = "Alex Morgan"

SECRET = secrets.token_bytes(32)  # per process; a restarted app has a new one
BOOT_ID = secrets.token_hex(8)
STARTED_AT = time.time()
AUDIT_MEM: list[dict] = []
AUDIT_LOCK = threading.Lock()
DB_PATH = Path("/var/lib/postbox/app.db")
ARRIVALS: list[dict] = []  # loaded from POSTBOX_ARRIVALS at --serve
ARRIVALS_LOCK = threading.Lock()
ARRIVALS_POLL_MS = 4000

SCHEMA = """
CREATE TABLE messages (
  id INTEGER PRIMARY KEY, thread_id INTEGER NOT NULL, folder TEXT NOT NULL,
  from_name TEXT NOT NULL, from_email TEXT NOT NULL, to_addr TEXT NOT NULL,
  subject TEXT NOT NULL, body TEXT NOT NULL, received_at TEXT NOT NULL,
  unread INTEGER NOT NULL DEFAULT 0, attachment TEXT NOT NULL DEFAULT '',
  newsletter INTEGER NOT NULL DEFAULT 0, unsubscribed INTEGER NOT NULL DEFAULT 0);
CREATE TABLE outbox (
  id INTEGER PRIMARY KEY AUTOINCREMENT, kind TEXT NOT NULL, in_reply_to INTEGER,
  to_addr TEXT NOT NULL, subject TEXT NOT NULL, body TEXT NOT NULL,
  attachment TEXT NOT NULL DEFAULT '', sent_at TEXT NOT NULL);
CREATE TABLE audit (
  seq INTEGER PRIMARY KEY AUTOINCREMENT, ts TEXT NOT NULL, sid TEXT,
  event TEXT NOT NULL, message_id INTEGER, field TEXT, old TEXT, new TEXT,
  via TEXT, tok_ok INTEGER, ui_ok INTEGER);
"""
MESSAGE_KEYS = (
    "id", "thread_id", "folder", "from_name", "from_email", "to_addr", "subject", "body",
    "received_at", "unread", "attachment", "newsletter",
)


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
    for msg in seed["messages"]:
        msg.setdefault("to_addr", ME)
        msg.setdefault("unread", 0)
        msg.setdefault("attachment", "")
        msg.setdefault("newsletter", 0)
    con.executemany(
        "INSERT INTO messages(id, thread_id, folder, from_name, from_email, to_addr, subject, body, "
        "received_at, unread, attachment, newsletter) VALUES (:id, :thread_id, :folder, :from_name, "
        ":from_email, :to_addr, :subject, :body, :received_at, :unread, :attachment, :newsletter)",
        seed["messages"],
    )
    con.commit()
    con.close()


def next_stamp(con: sqlite3.Connection) -> str:
    """The timestamp for a message the person sends now.

    One minute after the later of APP_NOW and the newest ``received_at`` in
    the mailbox, Sent and Trash included, so a reply is stamped after the
    mail it answers even when that mail arrived mid-task with a
    ``received_at`` later than APP_NOW, and a second send lands after the
    first. Deterministic: derived from the data, never from the wall clock.
    """
    newest = con.execute("SELECT MAX(received_at) FROM messages").fetchone()[0]
    base = max(APP_NOW, newest or APP_NOW)
    return (datetime.fromisoformat(base) + timedelta(minutes=1)).strftime("%Y-%m-%dT%H:%M")


def audit(con: sqlite3.Connection, **row: object) -> None:
    """Append one audit row inside the caller's transaction and mirror it."""
    full = {
        "ts": APP_NOW, "sid": None, "event": "", "message_id": None, "field": None,
        "old": None, "new": None, "via": None, "tok_ok": None, "ui_ok": None,
    }
    full.update(row)
    cur = con.execute(
        "INSERT INTO audit(ts, sid, event, message_id, field, old, new, via, tok_ok, ui_ok) "
        "VALUES (:ts, :sid, :event, :message_id, :field, :old, :new, :via, :tok_ok, :ui_ok)",
        full,
    )
    full["seq"] = cur.lastrowid
    with AUDIT_LOCK:
        AUDIT_MEM.append(full)


# ---------------------------------------------------------------------------
# arrivals: messages that appear while the task runs, clocked from the first
# audited page view (same design as Relay Desk's; see the module docstring)
# ---------------------------------------------------------------------------


def arrivals_load(path: str | None) -> None:
    ARRIVALS.clear()
    if not path:
        return
    for entry in json.loads(Path(path).read_text()):
        msg = dict(entry["message"])
        msg.setdefault("to_addr", ME)
        msg.setdefault("unread", 1)
        msg.setdefault("attachment", "")
        msg.setdefault("newsletter", 0)
        ARRIVALS.append({
            "after_s": float(entry["after_s"]),
            "trigger": entry.get("trigger") or {},
            "message": msg,
            "t0": None,
            "inserted_at": None,
        })


def arrivals_note(event: str, message_id: int | None, thread_id: int | None = None) -> None:
    """A page was viewed: start the clock of every arrival whose trigger matches.

    A trigger may name an event, a message id or a thread id (any message of
    the conversation, since the message page shows the whole thread).
    """
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
            if "message_id" in trig and trig["message_id"] != message_id:
                continue
            if "thread_id" in trig and trig["thread_id"] != thread_id:
                continue
            a["t0"] = now


def arrivals_due() -> list[dict]:
    now = time.time()
    with ARRIVALS_LOCK:
        return [a for a in ARRIVALS
                if a["inserted_at"] is None and not a.get("failed")
                and a["t0"] is not None and now - a["t0"] >= a["after_s"]]


def arrivals_insert(a: dict) -> None:
    msg = a["message"]
    con = connect(DB_PATH)
    try:
        con.execute(
            "INSERT INTO messages(id, thread_id, folder, from_name, from_email, to_addr, subject, body, "
            "received_at, unread, attachment, newsletter) VALUES (:id, :thread_id, :folder, :from_name, "
            ":from_email, :to_addr, :subject, :body, :received_at, :unread, :attachment, :newsletter)",
            msg,
        )
        audit(con, event="arrival", message_id=int(msg["id"]), field="message",
              new=str(msg["subject"])[:80], via="server")
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
                print(f"arrival {a['message'].get('id')} failed: {e}", flush=True)
            except sqlite3.Error as e:  # pragma: no cover - a busy database: retried next tick
                print(f"arrival {a['message'].get('id')} deferred: {e}", flush=True)
        time.sleep(1.0)


def arrivals_state() -> dict:
    with ARRIVALS_LOCK:
        items = [
            {"id": int(a["message"]["id"]), "thread_id": int(a["message"]["thread_id"]),
             "after_s": a["after_s"], "t0": a["t0"], "inserted_at": a["inserted_at"]}
            for a in ARRIVALS
        ]
    arrived = [i for i in items if i["inserted_at"] is not None]
    return {
        "count": len(arrived),
        "items": [{"id": i["id"], "thread_id": i["thread_id"]} for i in arrived],
        "scheduled": items,
    }


def arrivals_html(thread_id: int | None, back: str | None = None) -> str:
    """Hidden banner plus the poll that fills it; empty when no arrivals are scheduled.

    ``back`` is where a "new message in this conversation" banner should lead
    when the current page cannot show the conversation (the compose form).
    """
    if not ARRIVALS:
        return ""
    n0 = arrivals_state()["count"]
    th = int(thread_id) if thread_id is not None else 0
    back_js = repr(back) if back else "(location.pathname+location.search)"
    back_label = "Open conversation" if back else "Refresh"
    return (
        "<div class='banner arrivals' id='arrivals' hidden></div>"
        f"<script>(function(){{var n0={n0},th={th};function poll(){{fetch('/api/arrivals',{{cache:'no-store'}})"
        ".then(function(r){return r.json()}).then(function(d){if(d.count>n0){var b=document.getElementById('arrivals');"
        "var fresh=d.items.slice(n0);var mine=fresh.filter(function(i){return i.thread_id===th}).length;"
        "var k=mine||fresh.length;var what=mine?(k===1?' new message in this conversation. ':' new messages in this conversation. ')"
        ":(k===1?' new message has arrived. ':' new messages have arrived. ');b.textContent=k+what;"
        f"var a=document.createElement('a');a.href=mine?{back_js}:'/mail/Inbox';"
        f"a.textContent=mine?'{back_label}':'Open Inbox';b.appendChild(a);b.hidden=false;}}}})"
        f".catch(function(){{}});}}setInterval(poll,{ARRIVALS_POLL_MS});}})();</script>"
    )


# ---------------------------------------------------------------------------
# tokens (same scheme as Relay Desk)
# ---------------------------------------------------------------------------


def form_token(sid: str, scope: str) -> str:
    return hmac.new(SECRET, f"{sid}|{scope}".encode(), hashlib.sha256).hexdigest()[:32]


def ui_value(tok: str) -> str:
    h = 0x811C9DC5
    for ch in tok + "|postbox":
        h ^= ord(ch)
        h = (h * 0x01000193) & 0xFFFFFFFF
    return format(h, "x")


UI_SCRIPT = (
    "<script>(function(){var ts=document.querySelectorAll('input[name=tok]');"
    "for(var k=0;k<ts.length;k++){var t=ts[k].value+'|postbox';var h=0x811c9dc5;"
    "for(var i=0;i<t.length;i++){h^=t.charCodeAt(i);h=Math.imul(h,0x01000193)>>>0;}"
    "var f=ts[k].form.querySelectorAll('input[name=ui]');"
    "for(var j=0;j<f.length;j++){f[j].value=h.toString(16);}}})();</script>"
)


# ---------------------------------------------------------------------------
# html
# ---------------------------------------------------------------------------

CSS = """
*{box-sizing:border-box;transition:none !important;animation:none !important}
html{font-size:15px}
body{margin:0;font-family:"DejaVu Sans",sans-serif;color:#1b1f23;background:#f6f7f6}
a{color:#0f6b5c;text-decoration:none}
a:hover{text-decoration:underline}
.top{background:#114b42;color:#fff;padding:0 20px;height:48px;display:flex;align-items:center;gap:20px}
.top .brand{font-weight:bold;font-size:17px;color:#fff}
.top .me{margin-left:auto;color:#bfe0d8;font-size:13px}
.layout{width:980px;margin:0 auto;display:flex;gap:16px;padding:16px 0 40px}
.side{width:170px;flex:none}
.side a,.side span{display:block;padding:7px 10px;border-radius:3px;margin-bottom:2px;color:#1b1f23}
.side a.cur{background:#d6ece6;font-weight:bold}
.side .n{float:right;color:#4b5563;font-size:12px}
.main{flex:1;min-width:0}
h1{font-size:19px;margin:4px 0 12px}
.bar{display:flex;align-items:center;gap:8px;margin-bottom:10px;flex-wrap:wrap}
input[type=text],select,textarea{font:inherit;padding:6px 8px;border:1px solid #c6ccd3;border-radius:3px;background:#fff}
button{font:inherit;padding:6px 12px;border:1px solid #9aa3ad;border-radius:3px;background:#fff;cursor:pointer}
button.primary{background:#0f6b5c;border-color:#0f6b5c;color:#fff}
button.danger{border-color:#b42318;color:#b42318}
table{width:100%;border-collapse:collapse;background:#fff;border:1px solid #d9dee3}
th,td{text-align:left;padding:7px 8px;border-bottom:1px solid #e6e9ec;font-size:14px;vertical-align:top}
th{background:#eef1f0;font-weight:bold}
tr.unread td.subj a{font-weight:bold}
td.chk{width:28px}
td.date{white-space:nowrap;color:#4b5563;font-size:13px}
.pill{display:inline-block;padding:1px 7px;border-radius:10px;font-size:12px;background:#e6e9ec;margin-left:6px}
.pager{margin-top:10px;display:flex;gap:8px;align-items:center;font-size:14px}
.pager .cur{font-weight:bold}
.card{background:#fff;border:1px solid #d9dee3;padding:14px 16px;margin-bottom:12px}
.card.sel{border-color:#0f6b5c;box-shadow:inset 3px 0 0 #0f6b5c}
.hdr{font-size:13px;color:#4b5563;margin-bottom:8px}
.hdr b{color:#1b1f23}
pre.body{font:inherit;white-space:pre-wrap;margin:0}
.att{display:inline-block;background:#eef1f0;border:1px solid #d9dee3;padding:3px 8px;border-radius:3px;font-size:13px;margin-top:8px}
.banner{background:#d6ece6;border:1px solid #9fcab9;color:#0f4a3f;padding:8px 12px;margin-bottom:12px}
.banner.err{background:#fde2e1;border-color:#e4a09b;color:#8a1c12}
.banner.arrivals{background:#fff4d6;border-color:#e6c36a;color:#6b4e00}
.actions{display:flex;gap:8px;margin-top:10px;align-items:center;flex-wrap:wrap}
.right{margin-left:auto}
.grid{display:grid;grid-template-columns:90px 1fr;row-gap:8px;column-gap:10px;align-items:center}
.grid label{font-size:13px;color:#4b5563}
.foot{font-size:12px;color:#6b7280;border-top:1px solid #e6e9ec;margin-top:12px;padding-top:8px}
"""

FAVICON = "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg'/%3E"


def esc(value: object) -> str:
    return html.escape("" if value is None else str(value), quote=True)


def fmt_date(iso: str) -> str:
    return iso.replace("T", " ")


def folder_counts(con: sqlite3.Connection) -> dict[str, tuple[int, int]]:
    out = {f: (0, 0) for f in FOLDERS}
    for r in con.execute("SELECT folder, COUNT(*) AS n, SUM(unread) AS u FROM messages GROUP BY folder"):
        out[r["folder"]] = (r["n"], r["u"] or 0)
    return out


def page(con: sqlite3.Connection, title: str, body: str, current: str | None = None,
         thread_id: int | None = None, arrivals_back: str | None = None) -> str:
    counts = folder_counts(con)
    side = ["<div class='side'>"]
    for f in FOLDERS:
        n, u = counts.get(f, (0, 0))
        cls = " class='cur'" if f == current else ""
        badge = f"<span class='n'>{u} new</span>" if u else f"<span class='n'>{n}</span>"
        side.append(f"<a href='/mail/{f}'{cls}>{f}{badge}</a>")
    side.append("</div>")
    return (
        "<!doctype html><html lang='en'><head><meta charset='utf-8'>"
        f"<title>{esc(title)} - Postbox</title>"
        f"<link rel='icon' href=\"{FAVICON}\"><style>{CSS}</style></head><body>"
        "<div class='top'><span class='brand'>Postbox</span>"
        "<form method='get' action='/search' style='margin:0'>"
        "<input type='text' name='q' placeholder='Search mail' size='34'> <button type='submit'>Search</button></form>"
        f"<span class='me'>{esc(ME_NAME)} &lt;{esc(ME)}&gt; &middot; {TODAY_LABEL}</span></div>"
        f"<div class='layout'>{''.join(side)}<div class='main'>{arrivals_html(thread_id, arrivals_back)}{body}</div></div>"
        "</body></html>"
    )


def flash_html(flash: str | None) -> str:
    texts = {
        "sent": "Message sent.",
        "archived": "Moved to Archive.",
        "trashed": "Moved to Trash.",
        "deleted": "Deleted permanently.",
        "emptied": "Trash emptied.",
        "moved": "Message moved.",
        "unsubscribed": "You have been unsubscribed from this list.",
        "read": "Marked as read.",
        "discarded": "Draft discarded.",
    }
    if flash in texts:
        return f"<div class='banner' id='flash'>{texts[flash]}</div>"
    if flash == "rejected":
        return "<div class='banner err' id='flash'>That action could not be completed. Open the message again and retry.</div>"
    return ""


def render_list(con: sqlite3.Connection, sid: str, folder: str | None, q: str, page_no: int,
                flash: str | None) -> str:
    if folder is not None:
        where, params, title = "WHERE folder = ?", [folder], folder
    else:
        like = f"%{q}%"
        where = "WHERE (subject LIKE ? OR from_name LIKE ? OR from_email LIKE ? OR body LIKE ?)"
        params, title = [like, like, like, like], f"Search: {q}"
    total = con.execute(f"SELECT COUNT(*) FROM messages {where}", params).fetchone()[0]
    pages = max(1, (total + PAGE_SIZE - 1) // PAGE_SIZE)
    page_no = min(max(1, page_no), pages)
    rows = con.execute(
        f"SELECT * FROM messages {where} ORDER BY received_at DESC, id DESC LIMIT ? OFFSET ?",
        [*params, PAGE_SIZE, (page_no - 1) * PAGE_SIZE],
    ).fetchall()
    tok = form_token(sid, "bulk")
    out = [flash_html(flash), f"<h1>{esc(title)} <span class='pill'>{total}</span></h1>"]
    out.append(
        "<form method='post' action='/bulk' id='bulk'>"
        f"<input type='hidden' name='tok' value='{tok}'><input type='hidden' name='ui' value=''>"
        f"<input type='hidden' name='back' value='{esc(folder or '')}'>"
        "<div class='bar'>"
        "<button type='submit' name='action' value='archive' "
        "onclick=\"var n=document.querySelectorAll('input[name=sel]:checked').length;"
        "return n>0&&confirm('Archive '+n+' message(s)?')\">Archive</button>"
        "<button type='submit' name='action' value='trash' class='danger' "
        "onclick=\"var n=document.querySelectorAll('input[name=sel]:checked').length;"
        "return n>0&&confirm('Move '+n+' message(s) to Trash?')\">Delete</button>"
        "<button type='submit' name='action' value='read'>Mark as read</button>"
    )
    if folder == "Trash":
        out.append(
            "<button type='submit' name='action' value='empty' class='danger right' "
            "onclick=\"return confirm('Permanently delete everything in Trash? This cannot be undone.')\">Empty trash</button>"
        )
    out.append("</div>")
    out.append("<table><tr><th></th><th>From</th><th>Subject</th>"
               + ("<th>Folder</th>" if folder is None else "") + "<th>Received</th></tr>")
    for r in rows:
        cls = " class='unread'" if r["unread"] else ""
        att = " &#128206;" if r["attachment"] else ""
        out.append(
            f"<tr{cls}><td class='chk'><input type='checkbox' name='sel' value='{r['id']}'></td>"
            f"<td>{esc(r['from_name'])}</td>"
            f"<td class='subj'><a href='/message/{r['id']}'>{esc(r['subject'])}</a>{att}</td>"
            + (f"<td>{esc(r['folder'])}</td>" if folder is None else "")
            + f"<td class='date'>{esc(fmt_date(r['received_at']))}</td></tr>"
        )
    if not rows:
        out.append("<tr><td colspan='5'>Nothing here.</td></tr>")
    out.append("</table></form><div class='pager'>")

    def link(n: int, label: str) -> str:
        if folder is not None:
            return f"<a href='/mail/{folder}?{urlencode({'page': n})}'>{esc(label)}</a>"
        return f"<a href='/search?{urlencode({'q': q, 'page': n})}'>{esc(label)}</a>"

    out.append(link(page_no - 1, "Previous") if page_no > 1 else "<span>Previous</span>")
    for n in range(1, pages + 1):
        out.append(f"<span class='cur'>{n}</span>" if n == page_no else link(n, str(n)))
    out.append(link(page_no + 1, "Next") if page_no < pages else "<span>Next</span>")
    out.append(f"<span class='right'>Page {page_no} of {pages}</span></div>")
    out.append(UI_SCRIPT)
    return page(con, title, "".join(out), folder)


def render_message(con: sqlite3.Connection, m: sqlite3.Row, sid: str, flash: str | None) -> str:
    tok = form_token(sid, f"msg:{m['id']}")
    thread = con.execute(
        "SELECT * FROM messages WHERE thread_id = ? AND folder != 'Trash' ORDER BY received_at, id",
        (m["thread_id"],),
    ).fetchall()
    if m["folder"] == "Trash" or not thread:
        thread = [m]
    out = [flash_html(flash), f"<h1>{esc(m['subject'])}</h1>"]
    out.append(
        f"<form method='post' action='/message/{m['id']}' class='actions' id='actions' style='margin:0 0 12px'>"
        f"<input type='hidden' name='tok' value='{tok}'><input type='hidden' name='ui' value=''>"
        f"<a href='/compose?reply={m['id']}'><button type='button' class='primary' id='reply'>Reply</button></a>"
        f"<a href='/compose?forward={m['id']}'><button type='button' id='forward'>Forward</button></a>"
        "<button type='submit' name='action' value='archive' id='archive'>Archive</button>"
        "<button type='submit' name='action' value='trash' class='danger' id='trash'>Delete</button>"
        "<span class='right'></span><label for='to'>Move to</label>"
        "<select name='to' id='to'>" + "".join(
            f"<option value='{f}'{' selected' if f == m['folder'] else ''}>{f}</option>" for f in FOLDERS if f != "Sent"
        ) + "</select><button type='submit' name='action' value='move'>Move</button></form>"
    )
    for t in thread:
        sel = " sel" if t["id"] == m["id"] else ""
        out.append(f"<div class='card{sel}' id='m{t['id']}'>")
        out.append(
            f"<div class='hdr'><b>{esc(t['from_name'])}</b> &lt;{esc(t['from_email'])}&gt; to {esc(t['to_addr'])}"
            f" &middot; {esc(fmt_date(t['received_at']))} &middot; {esc(t['folder'])}</div>"
        )
        out.append(f"<pre class='body'>{esc(t['body'])}</pre>")
        if t["attachment"]:
            out.append(f"<div class='att'>&#128206; <a href='/attachment/{t['id']}'>{esc(t['attachment'])}</a></div>")
        if t["newsletter"]:
            out.append(
                f"<div class='foot'>You receive this newsletter because you subscribed with {esc(ME)}. "
                f"<a href='/unsubscribe/{t['id']}'>Unsubscribe</a></div>"
            )
        out.append("</div>")
    out.append(UI_SCRIPT)
    return page(con, m["subject"], "".join(out), m["folder"], thread_id=int(m["thread_id"]))


def quote(m: sqlite3.Row) -> str:
    quoted = "\n".join("> " + line for line in m["body"].splitlines())
    return (f"\n\n\nOn {fmt_date(m['received_at'])}, {m['from_name']} <{m['from_email']}> wrote:\n{quoted}\n")


def render_compose(con: sqlite3.Connection, sid: str, kind: str, src: sqlite3.Row | None) -> str:
    tok = form_token(sid, "compose")
    to, subject, body, att = "", "", "", ""
    if src is not None and kind == "reply":
        to = src["from_email"]
        subject = src["subject"] if src["subject"].startswith("Re:") else f"Re: {src['subject']}"
        body = quote(src)
    elif src is not None and kind == "forward":
        subject = src["subject"] if src["subject"].startswith("Fwd:") else f"Fwd: {src['subject']}"
        body = "\n\n\n---------- Forwarded message ----------\n" + (
            f"From: {src['from_name']} <{src['from_email']}>\nDate: {fmt_date(src['received_at'])}\n"
            f"Subject: {src['subject']}\nTo: {src['to_addr']}\n\n{src['body']}\n"
        )
        att = src["attachment"]
    out = [f"<h1>{'Reply' if kind == 'reply' else 'Forward' if kind == 'forward' else 'New message'}</h1>"]
    out.append(
        "<form method='post' action='/compose' class='card' id='compose'>"
        f"<input type='hidden' name='tok' value='{tok}'><input type='hidden' name='ui' value=''>"
        f"<input type='hidden' name='kind' value='{kind}'>"
        f"<input type='hidden' name='in_reply_to' value='{src['id'] if src is not None else ''}'>"
        f"<input type='hidden' name='attachment' value='{esc(att)}' id='attachment'>"
        "<div class='grid'>"
        f"<label for='to'>To</label><input type='text' name='to' id='to' value='{esc(to)}' size='60'>"
        f"<label for='subject'>Subject</label><input type='text' name='subject' id='subject' value='{esc(subject)}' size='60'>"
        "</div>"
        f"<textarea name='body' id='body' rows='14' style='width:100%;margin-top:10px'>{esc(body)}</textarea>"
    )
    if att:
        out.append(
            f"<div class='att' id='att-chip'>&#128206; {esc(att)} "
            "<a href='#' onclick=\"document.getElementById('attachment').value='';"
            "document.getElementById('att-chip').remove();return false\">remove</a></div>"
        )
    out.append(
        "<div class='actions'><button type='submit' class='primary' id='send'>Send</button>"
        f"<a href='/message/{src['id']}' class='right'>Discard</a></div></form>" if src is not None else
        "<div class='actions'><button type='submit' class='primary' id='send'>Send</button>"
        "<a href='/mail/Inbox' class='right'>Discard</a></div></form>"
    )
    out.append(UI_SCRIPT)
    return page(
        con, "Compose", "".join(out), None,
        thread_id=int(src["thread_id"]) if src is not None else None,
        arrivals_back=f"/message/{src['id']}" if src is not None else None,
    )


def render_unsubscribe(con: sqlite3.Connection, m: sqlite3.Row, sid: str) -> str:
    tok = form_token(sid, f"unsub:{m['id']}")
    body = (
        f"<h1>Unsubscribe</h1><form method='post' action='/unsubscribe/{m['id']}' class='card'>"
        f"<input type='hidden' name='tok' value='{tok}'><input type='hidden' name='ui' value=''>"
        f"<p>Stop receiving mail from <b>{esc(m['from_name'])}</b> &lt;{esc(m['from_email'])}&gt;?</p>"
        "<div class='actions'><button type='submit' class='danger' id='unsubscribe'>Unsubscribe</button>"
        f"<a href='/message/{m['id']}' class='right'>Cancel</a></div></form>" + UI_SCRIPT
    )
    return page(con, "Unsubscribe", body, m["folder"])


# ---------------------------------------------------------------------------
# http
# ---------------------------------------------------------------------------


class Handler(BaseHTTPRequestHandler):
    server_version = "Postbox/0.1"

    def log_message(self, fmt: str, *args: object) -> None:  # quiet
        return

    def _sid(self) -> tuple[str, bool]:
        jar = SimpleCookie(self.headers.get("Cookie", ""))
        if "psid" in jar and len(jar["psid"].value) == 32:
            return jar["psid"].value, False
        return secrets.token_hex(16), True

    def _send(self, status: HTTPStatus, body: str | bytes, *, set_sid: str | None = None,
              ctype: str = "text/html; charset=utf-8") -> None:
        data = body.encode() if isinstance(body, str) else body
        self.send_response(status)
        self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(len(data)))
        self.send_header("Cache-Control", "no-store")
        if set_sid:
            self.send_header("Set-Cookie", f"psid={set_sid}; Path=/; HttpOnly")
        self.end_headers()
        self.wfile.write(data)

    def _redirect(self, location: str, *, set_sid: str | None = None) -> None:
        self.send_response(HTTPStatus.SEE_OTHER)
        self.send_header("Location", location)
        self.send_header("Content-Length", "0")
        if set_sid:
            self.send_header("Set-Cookie", f"psid={set_sid}; Path=/; HttpOnly")
        self.end_headers()

    def _form(self) -> tuple[dict[str, str], list[str]]:
        n = int(self.headers.get("Content-Length") or 0)
        raw = self.rfile.read(n).decode() if n else ""
        parsed = parse_qs(raw, keep_blank_values=True)
        return {k: v[0] for k, v in parsed.items()}, parsed.get("sel", [])

    def _not_found(self, con: sqlite3.Connection | None = None) -> None:
        own = con is None
        con = con or connect(DB_PATH)
        try:
            self._send(HTTPStatus.NOT_FOUND, page(con, "Not found", "<h1>Not found</h1>"))
        finally:
            if own:
                con.close()

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
            self._send(HTTPStatus.OK, json.dumps(
                {"boot_id": BOOT_ID, "started_at": STARTED_AT, "audit_count": len(rows), "audit": rows,
                 "arrivals": arrivals_state()["scheduled"]}
            ), ctype="application/json")
            return
        if url.path == "/api/arrivals":
            st = arrivals_state()
            self._send(HTTPStatus.OK, json.dumps({"count": st["count"], "items": st["items"]}),
                       ctype="application/json")
            return
        sid, fresh = self._sid()
        set_sid = sid if fresh else None
        if url.path == "/":
            self._redirect("/mail/Inbox", set_sid=set_sid)
            return
        con = connect(DB_PATH)
        try:
            parts = url.path.strip("/").split("/")
            if parts[0] == "mail" and len(parts) == 2 and parts[1] in FOLDERS:
                audit(con, sid=sid, event="page.list", field=parts[1], new=qs.get("page", "1"), via="get")
                con.commit()
                arrivals_note("page.list", None)
                body = render_list(con, sid, parts[1], "", int(qs.get("page", "1") or 1), qs.get("flash"))
                self._send(HTTPStatus.OK, body, set_sid=set_sid)
                return
            if url.path == "/search":
                q = qs.get("q", "").strip()
                if not q:
                    self._redirect("/mail/Inbox", set_sid=set_sid)
                    return
                audit(con, sid=sid, event="page.list", field="search", new=q[:80], via="get")
                con.commit()
                arrivals_note("page.list", None)
                body = render_list(con, sid, None, q, int(qs.get("page", "1") or 1), qs.get("flash"))
                self._send(HTTPStatus.OK, body, set_sid=set_sid)
                return
            if parts[0] in ("message", "unsubscribe", "attachment") and len(parts) == 2 and parts[1].isdigit():
                m = con.execute("SELECT * FROM messages WHERE id = ?", (int(parts[1]),)).fetchone()
                if m is None:
                    self._not_found(con)
                    return
                if parts[0] == "attachment":
                    if not m["attachment"]:
                        self._not_found(con)
                        return
                    audit(con, sid=sid, event="attachment.open", message_id=m["id"], new=m["attachment"], via="get")
                    con.commit()
                    self._send(HTTPStatus.OK, f"[{m['attachment']}: attachment placeholder]\n".encode(),
                               ctype="application/octet-stream", set_sid=set_sid)
                    return
                if parts[0] == "unsubscribe":
                    self._send(HTTPStatus.OK, render_unsubscribe(con, m, sid), set_sid=set_sid)
                    return
                if m["unread"]:
                    con.execute("UPDATE messages SET unread = 0 WHERE id = ?", (m["id"],))
                audit(con, sid=sid, event="page.view", message_id=m["id"], via="get")
                con.commit()
                arrivals_note("page.view", int(m["id"]), int(m["thread_id"]))
                m = con.execute("SELECT * FROM messages WHERE id = ?", (m["id"],)).fetchone()
                self._send(HTTPStatus.OK, render_message(con, m, sid, qs.get("flash")), set_sid=set_sid)
                return
            if url.path == "/compose":
                kind, src = "new", None
                for k in ("reply", "forward"):
                    if qs.get(k, "").isdigit():
                        src = con.execute("SELECT * FROM messages WHERE id = ?", (int(qs[k]),)).fetchone()
                        kind = k if src is not None else "new"
                audit(con, sid=sid, event="compose.open", message_id=src["id"] if src is not None else None,
                      field=kind, via="get")
                con.commit()
                arrivals_note("compose.open", src["id"] if src is not None else None,
                              int(src["thread_id"]) if src is not None else None)
                self._send(HTTPStatus.OK, render_compose(con, sid, kind, src), set_sid=set_sid)
                return
            self._not_found(con)
        finally:
            con.close()

    # POST
    def do_POST(self) -> None:
        url = urlparse(self.path)
        form, sel = self._form()
        sid, fresh = self._sid()
        parts = url.path.strip("/").split("/")
        scope = {"bulk": "bulk", "compose": "compose"}.get(parts[0])
        if scope is None and parts[0] in ("message", "unsubscribe") and len(parts) == 2 and parts[1].isdigit():
            scope = ("msg:" if parts[0] == "message" else "unsub:") + parts[1]
        if scope is None:
            self._not_found()
            return
        tok_ok = int(hmac.compare_digest(form.get("tok", ""), form_token(sid, scope))) if not fresh else 0
        ui_ok = int(tok_ok == 1 and form.get("ui", "") == ui_value(form.get("tok", "")))
        con = connect(DB_PATH)
        try:
            if not (tok_ok and ui_ok):
                reason = "no-session" if fresh else ("bad-token" if not tok_ok else "no-ui")
                mid = int(parts[1]) if len(parts) == 2 and parts[1].isdigit() else None
                audit(con, sid=sid, event="rejected", message_id=mid, field=parts[0], new=reason,
                      via="form", tok_ok=tok_ok, ui_ok=ui_ok)
                con.commit()
                self._send(HTTPStatus.FORBIDDEN, page(con, "Rejected", flash_html("rejected") + "<h1>Rejected</h1>"),
                           set_sid=sid if fresh else None)
                return
            if parts[0] == "message":
                self._message_action(con, sid, int(parts[1]), form)
            elif parts[0] == "unsubscribe":
                self._unsubscribe(con, sid, int(parts[1]))
            elif parts[0] == "bulk":
                self._bulk(con, sid, form, sel)
            else:
                self._compose(con, sid, form)
        finally:
            con.close()

    def _move(self, con: sqlite3.Connection, sid: str, m: sqlite3.Row, folder: str, event: str) -> None:
        if folder == m["folder"]:
            return
        con.execute("UPDATE messages SET folder = ? WHERE id = ?", (folder, m["id"]))
        audit(con, sid=sid, event=event, message_id=m["id"], field="folder", old=m["folder"], new=folder,
              via="form", tok_ok=1, ui_ok=1)

    def _message_action(self, con: sqlite3.Connection, sid: str, mid: int, form: dict[str, str]) -> None:
        m = con.execute("SELECT * FROM messages WHERE id = ?", (mid,)).fetchone()
        if m is None:
            self._not_found(con)
            return
        action = form.get("action", "")
        if action == "archive":
            self._move(con, sid, m, "Archive", "message.archive")
            con.commit()
            self._redirect(f"/mail/{m['folder']}?flash=archived")
        elif action == "trash":
            if m["folder"] == "Trash":
                con.execute("DELETE FROM messages WHERE id = ?", (mid,))
                audit(con, sid=sid, event="message.delete", message_id=mid, field="folder", old="Trash",
                      via="form", tok_ok=1, ui_ok=1)
                con.commit()
                self._redirect("/mail/Trash?flash=deleted")
            else:
                self._move(con, sid, m, "Trash", "message.trash")
                con.commit()
                self._redirect(f"/mail/{m['folder']}?flash=trashed")
        elif action == "move" and form.get("to") in FOLDERS:
            self._move(con, sid, m, form["to"], "message.move")
            con.commit()
            self._redirect(f"/message/{mid}?flash=moved")
        else:
            self._redirect(f"/message/{mid}")

    def _unsubscribe(self, con: sqlite3.Connection, sid: str, mid: int) -> None:
        m = con.execute("SELECT * FROM messages WHERE id = ?", (mid,)).fetchone()
        if m is None:
            self._not_found(con)
            return
        con.execute("UPDATE messages SET unsubscribed = 1 WHERE from_email = ?", (m["from_email"],))
        audit(con, sid=sid, event="list.unsubscribe", message_id=mid, field="from_email", new=m["from_email"],
              via="form", tok_ok=1, ui_ok=1)
        con.commit()
        self._redirect(f"/message/{mid}?flash=unsubscribed")

    def _bulk(self, con: sqlite3.Connection, sid: str, form: dict[str, str], sel: list[str]) -> None:
        action = form.get("action", "")
        back = form.get("back") if form.get("back") in FOLDERS else "Inbox"
        ids = [int(s) for s in sel if s.isdigit()]
        if action == "empty":
            rows = con.execute("SELECT id FROM messages WHERE folder = 'Trash'").fetchall()
            con.execute("DELETE FROM messages WHERE folder = 'Trash'")
            audit(con, sid=sid, event="trash.empty", new=str(len(rows)), via="form", tok_ok=1, ui_ok=1)
            for r in rows:
                audit(con, sid=sid, event="message.delete", message_id=r["id"], field="folder", old="Trash",
                      via="form", tok_ok=1, ui_ok=1)
            con.commit()
            self._redirect("/mail/Trash?flash=emptied")
            return
        flash = ""
        for mid in ids:
            m = con.execute("SELECT * FROM messages WHERE id = ?", (mid,)).fetchone()
            if m is None:
                continue
            if action == "archive":
                self._move(con, sid, m, "Archive", "message.archive")
                flash = "archived"
            elif action == "trash":
                self._move(con, sid, m, "Trash", "message.trash")
                flash = "trashed"
            elif action == "read" and m["unread"]:
                con.execute("UPDATE messages SET unread = 0 WHERE id = ?", (mid,))
                flash = "read"
        con.commit()
        self._redirect(f"/mail/{back}" + (f"?flash={flash}" if flash else ""))

    def _compose(self, con: sqlite3.Connection, sid: str, form: dict[str, str]) -> None:
        to = form.get("to", "").strip()
        subject = form.get("subject", "").strip()
        body = form.get("body", "")
        kind = form.get("kind", "new") if form.get("kind") in ("reply", "forward", "new") else "new"
        in_reply_to = int(form["in_reply_to"]) if form.get("in_reply_to", "").isdigit() else None
        attachment = form.get("attachment", "").strip()
        if not to:
            self._send(HTTPStatus.BAD_REQUEST, page(con, "Compose", "<div class='banner err'>A recipient is required.</div>"
                                                   "<p><a href='javascript:history.back()'>Back</a></p>"))
            return
        sent_at = next_stamp(con)
        con.execute(
            "INSERT INTO outbox(kind, in_reply_to, to_addr, subject, body, attachment, sent_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
            (kind, in_reply_to, to, subject, body, attachment, sent_at),
        )
        sent_id = con.execute("SELECT MAX(id) FROM outbox").fetchone()[0]
        # a copy in Sent, like a real client
        thread = con.execute("SELECT thread_id FROM messages WHERE id = ?", (in_reply_to,)).fetchone() if in_reply_to else None
        thread_id = thread["thread_id"] if thread else 90000 + sent_id
        con.execute(
            "INSERT INTO messages(thread_id, folder, from_name, from_email, to_addr, subject, body, received_at, "
            "unread, attachment, newsletter) VALUES (?, 'Sent', ?, ?, ?, ?, ?, ?, 0, ?, 0)",
            (thread_id, ME_NAME, ME, to, subject, body, sent_at, attachment),
        )
        audit(con, sid=sid, event="mail.send", message_id=in_reply_to, field=kind, new=to,
              old=attachment or None, via="form", tok_ok=1, ui_ok=1)
        con.commit()
        self._redirect("/mail/Sent?flash=sent")


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--init", metavar="DB", help="create and seed the database at DB")
    ap.add_argument("--serve", metavar="DB", help="serve the database at DB")
    ap.add_argument("--seed", default=str(Path(__file__).with_name("seed.json")))
    ap.add_argument("--port", type=int, default=8010)
    ap.add_argument("--host", default="127.0.0.1")
    args = ap.parse_args()
    if args.init:
        init_db(Path(args.init), Path(args.seed))
        print(f"seeded {args.init}")
        return
    if args.serve:
        global DB_PATH
        DB_PATH = Path(args.serve)
        arrivals_load(os.environ.get("POSTBOX_ARRIVALS"))
        if ARRIVALS:
            threading.Thread(target=arrivals_thread, name="arrivals", daemon=True).start()
        srv = ThreadingHTTPServer((args.host, args.port), Handler)
        print(f"postbox serving {DB_PATH} on http://{args.host}:{args.port} boot={BOOT_ID} "
              f"arrivals={len(ARRIVALS)}", flush=True)
        srv.serve_forever()
        return
    ap.error("use --init DB or --serve DB")


if __name__ == "__main__":
    main()
