#!/usr/bin/env python3
"""Gold trajectory for relaydesk-live-queue, through the UI.

Connects to the Chromium that ensure-display.sh (or computer-1) started.
Searches for Harbor Lane, walks both result pages, opens each open outage
ticket from the clinic and sets the three fields. Then it does what a person
watching the queue does: waits for the "new ticket" banner, looks at the list
again, handles the ticket that arrived (a target), waits for the second
arrival and leaves it alone (an account request, not the outage).

VB_ALSO_TOUCH=<ticket id>  also edit that ticket (guard check)
VB_REOPEN=<ticket id>      also set that ticket's status to Open (closed-ticket check)
VB_SKIP=<ticket id>        leave that seeded target alone (partial check)
VB_NO_WAIT=1               finish without waiting for the arrivals (the trap)
"""

from __future__ import annotations

import json
import os
import subprocess
import time
import urllib.request
from pathlib import Path

from playwright.sync_api import sync_playwright

BASE = "http://127.0.0.1:8000"
SEEDED_TARGETS = (3004, 3011, 3019, 3027)
ARRIVAL_TARGET = 3101
WANT = {"assignee": "Marcus Webb", "project": "Platform", "priority": "High"}


def arrivals_count() -> int:
    with urllib.request.urlopen(f"{BASE}/api/arrivals", timeout=5) as r:  # noqa: S310 - loopback
        return int(json.load(r)["count"])


def wait_for_arrivals(n: int, timeout_s: float) -> bool:
    t_end = time.time() + timeout_s
    while time.time() < t_end:
        if arrivals_count() >= n:
            return True
        time.sleep(2.0)
    return False


def edit_ticket(page, want: dict[str, str]) -> None:
    for field, value in want.items():
        page.select_option(f"#{field}", value)
    page.click("text=Save changes")
    page.wait_for_selector("#flash")


def main() -> None:
    skip = int(os.environ.get("VB_SKIP", "0") or 0)
    edited: list[int] = []
    with sync_playwright() as p:
        browser = p.chromium.connect_over_cdp("http://127.0.0.1:9222")
        ctx = browser.contexts[0] if browser.contexts else browser.new_context()
        page = ctx.pages[0] if ctx.pages else ctx.new_page()
        page.goto(f"{BASE}/tickets")  # the first look: the arrivals clock starts here
        page.fill("input[name=q]", "Harbor Lane")
        page.click("text=Search")
        found: list[int] = []
        while True:
            for tid in SEEDED_TARGETS:
                if tid != skip and page.locator(f"a[href='/tickets/{tid}']").count():
                    found.append(tid)
            nxt = page.locator(".pager a", has_text="Next")
            if nxt.count() == 0:
                break
            nxt.click()
            page.wait_for_load_state()
        for tid in sorted(found):
            page.goto(f"{BASE}/tickets/{tid}")
            edit_ticket(page, WANT)
            edited.append(tid)
        also = os.environ.get("VB_ALSO_TOUCH")
        if also:
            page.goto(f"{BASE}/tickets/{int(also)}")
            edit_ticket(page, {"priority": "High"})
        reopen = os.environ.get("VB_REOPEN")
        if reopen:
            page.goto(f"{BASE}/tickets/{int(reopen)}")
            edit_ticket(page, {"status": "Open"})
        if not os.environ.get("VB_NO_WAIT"):
            # the first arrival (a target) comes 60 s after the first page view
            if wait_for_arrivals(1, 150):
                page.goto(f"{BASE}/tickets?q=Harbor+Lane")
                page.wait_for_selector(f"a[href='/tickets/{ARRIVAL_TARGET}']")
                page.goto(f"{BASE}/tickets/{ARRIVAL_TARGET}")
                edit_ticket(page, WANT)
                edited.append(ARRIVAL_TARGET)
            # the second arrival (not a target) comes at 150 s; look, leave it
            if wait_for_arrivals(2, 200):
                page.goto(f"{BASE}/tickets?q=Harbor+Lane")
                page.wait_for_load_state()
        page.goto(f"{BASE}/tickets?q=Harbor+Lane")
        page.wait_for_timeout(500)
    out_dir = Path("/logs/agent") if Path("/logs/agent").is_dir() else Path("/tmp")
    subprocess.run(
        ["import", "-window", "root", str(out_dir / "gold-final.png")],
        env={**os.environ, "DISPLAY": ":1"},
        check=False,
    )
    print(f"gold: edited {edited}")


if __name__ == "__main__":
    main()
