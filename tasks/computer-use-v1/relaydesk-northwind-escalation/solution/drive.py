#!/usr/bin/env python3
"""Gold trajectory for relaydesk-northwind-escalation, through the UI.

Connects to the Chromium that ensure-display.sh (or computer-1) started, so
real input events reach the visible window. Searches for Northwind, walks
both result pages, opens each target by subject, sets the three fields and
saves; when the app signs the session out after the third save it continues
the session and reloads the ticket.

VB_ALSO_TOUCH=<ticket id>  also edit that ticket (validator guard check)
VB_REOPEN=<ticket id>      also set that ticket's status to Open (closed-ticket check)
VB_SKIP=<ticket id>        leave that target alone (validator partial check)
"""

from __future__ import annotations

import os
import subprocess
from pathlib import Path

from playwright.sync_api import sync_playwright

BASE = "http://127.0.0.1:8000"
TARGETS = {
    2007: "Invoice export to CSV drops the VAT column",
    2013: "Export of invoices times out for January",
    2021: "Invoice export missing credit notes",
    2034: "Cannot export invoices as PDF bundle",
    2048: "Exported invoice totals do not match the statement",
}
WANT = {"assignee": "Priya Raman", "project": "Billing", "priority": "High"}


def continue_if_expired(page) -> None:
    if page.locator("#continue").count():
        page.click("#continue")
        page.wait_for_selector("#edit")


def edit_ticket(page, want: dict[str, str]) -> None:
    continue_if_expired(page)
    for field, value in want.items():
        page.select_option(f"#{field}", value)
    page.click("text=Save changes")
    page.wait_for_selector("#flash")


def main() -> None:
    skip = int(os.environ.get("VB_SKIP", "0") or 0)
    with sync_playwright() as p:
        browser = p.chromium.connect_over_cdp("http://127.0.0.1:9222")
        ctx = browser.contexts[0] if browser.contexts else browser.new_context()
        page = ctx.pages[0] if ctx.pages else ctx.new_page()
        page.goto(f"{BASE}/tickets")
        page.fill("input[name=q]", "Northwind")
        page.click("text=Search")
        # collect the result links across pages, then edit each target
        found: dict[int, str] = {}
        while True:
            for tid, subject in TARGETS.items():
                if tid != skip and page.locator(f"a[href='/tickets/{tid}']").count():
                    found[tid] = f"{BASE}/tickets/{tid}"
            nxt = page.locator(".pager a", has_text="Next")
            if nxt.count() == 0:
                break
            nxt.click()
            page.wait_for_load_state()
        for tid in sorted(found):
            page.goto(found[tid])
            edit_ticket(page, WANT)
        also = os.environ.get("VB_ALSO_TOUCH")
        if also:
            page.goto(f"{BASE}/tickets/{int(also)}")
            edit_ticket(page, {"priority": "High"})
        reopen = os.environ.get("VB_REOPEN")
        if reopen:
            page.goto(f"{BASE}/tickets/{int(reopen)}")
            edit_ticket(page, {"status": "Open"})
        page.goto(f"{BASE}/tickets?q=Northwind")
        page.wait_for_timeout(500)
    out_dir = Path("/logs/agent") if Path("/logs/agent").is_dir() else Path("/tmp")
    subprocess.run(
        ["import", "-window", "root", str(out_dir / "gold-final.png")],
        env={**os.environ, "DISPLAY": ":1"},
        check=False,
    )
    print(f"gold: edited {sorted(found)}")


if __name__ == "__main__":
    main()
