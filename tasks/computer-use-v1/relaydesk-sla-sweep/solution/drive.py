#!/usr/bin/env python3
"""Gold trajectory for relaydesk-sla-sweep, through the UI.

Connects to the Chromium that ensure-display.sh (or computer-1) started.
Walks every page of the queue, opens every High ticket's page as a person
checking dates and comments would, and on each breached ticket adds the
comment and sets the priority. Waits for the ticket that arrives mid-task,
looks at the queue once more and leaves it alone (created now, not
breached). Then composes one mail to Priya in Postbox listing the ticket
numbers. Which tickets are breached is computed from the task's own seed
with the rule in the instruction; the point of the gold is a complete
trajectory, not the arithmetic.

VB_SKIP=<id>         leave that breached ticket alone (partial)
VB_ALSO=<id>         also comment on and escalate that ticket (near-miss guard)
VB_NO_PRIORITY=1     add the comments but leave priorities alone
VB_NO_MAIL=1         escalate but send no mail
VB_MAIL_EXTRA=<id>   list that extra ticket number in the mail
VB_WRONG_TO=1        send the mail to the look-alike Priya
"""

from __future__ import annotations

import json
import os
import subprocess
import time
import urllib.request
from pathlib import Path

from playwright.sync_api import sync_playwright

DESK = "http://127.0.0.1:8000"
MAIL = "http://127.0.0.1:8010"
CUTOFF = "2026-02-27T09:15"
COMMENT = "SLA breached, escalated"
PRIYA = "priya@relaydesk.example"
LOOKALIKE = "priya@ramanathan-consulting.example"
SEED = json.loads(Path("/opt/relaydesk/seed.json").read_text())


def breached() -> list[int]:
    commented = {c["ticket_id"] for c in SEED["comments"]}
    return sorted(
        t["id"] for t in SEED["tickets"]
        if t["priority"] == "High" and t["status"] in ("Open", "Pending")
        and t["created_at"] < CUTOFF and t["id"] not in commented
    )


def arrivals_count() -> int:
    with urllib.request.urlopen(f"{DESK}/api/arrivals", timeout=5) as r:  # noqa: S310 - loopback
        return int(json.load(r)["count"])


def wait_for_arrivals(n: int, timeout_s: float) -> bool:
    t_end = time.time() + timeout_s
    while time.time() < t_end:
        if arrivals_count() >= n:
            return True
        time.sleep(2.0)
    return False


def escalate(page, tid: int, priority: bool) -> None:
    page.goto(f"{DESK}/tickets/{tid}")
    page.fill("textarea[name=body]", COMMENT)
    page.click("text=Add comment")
    page.wait_for_selector("#flash")
    if priority:
        page.select_option("#priority", "Urgent")
        page.click("text=Save changes")
        page.wait_for_selector("#flash")


def main() -> None:
    skip = int(os.environ.get("VB_SKIP", "0") or 0)
    also = int(os.environ.get("VB_ALSO", "0") or 0)
    set_priority = not os.environ.get("VB_NO_PRIORITY")
    targets = [t for t in breached() if t != skip]
    if also:
        targets.append(also)
    subjects = {t["id"]: t["subject"] for t in SEED["tickets"]}
    high = sorted(t["id"] for t in SEED["tickets"] if t["priority"] == "High")
    with sync_playwright() as p:
        browser = p.chromium.connect_over_cdp("http://127.0.0.1:9222")
        ctx = browser.contexts[0] if browser.contexts else browser.new_context()
        page = ctx.pages[0] if ctx.pages else ctx.new_page()
        page.goto(f"{DESK}/tickets")  # the first look: the arrivals clock starts here
        while True:  # every page of the queue, as a person would
            page.wait_for_load_state()
            nxt = page.locator(".pager a", has_text="Next")
            if nxt.count() == 0:
                break
            nxt.click()
        for tid in high:  # read every High ticket: created date, status, comments
            page.goto(f"{DESK}/tickets/{tid}")
            page.wait_for_selector("#edit")
        for tid in targets:
            escalate(page, tid, set_priority)
        # the arrival lands 30 s after the first page view; look at the queue again and leave it
        if wait_for_arrivals(1, 90):
            page.goto(f"{DESK}/tickets")
            page.wait_for_load_state()
        if not os.environ.get("VB_NO_MAIL"):
            listed = sorted(targets)
            extra = int(os.environ.get("VB_MAIL_EXTRA", "0") or 0)
            if extra:
                listed.append(extra)
            body = "Hi Priya,\n\nSLA sweep done. Escalated to Urgent with a comment on each:\n\n" + "\n".join(
                f"#{tid} {subjects.get(tid, '')}" for tid in listed
            ) + "\n\nAlex"
            page.goto(f"{MAIL}/compose")
            page.wait_for_selector("#compose")
            page.fill("#to", LOOKALIKE if os.environ.get("VB_WRONG_TO") else PRIYA)
            page.fill("#subject", "SLA sweep, Monday 2 March")
            page.fill("#body", body)
            page.click("#send")
            page.wait_for_selector("#flash")
        page.wait_for_timeout(500)
    out_dir = Path("/logs/agent") if Path("/logs/agent").is_dir() else Path("/tmp")
    subprocess.run(
        ["import", "-window", "root", str(out_dir / "gold-final.png")],
        env={**os.environ, "DISPLAY": ":1"},
        check=False,
    )
    print(f"gold: escalated {targets}")


if __name__ == "__main__":
    main()
