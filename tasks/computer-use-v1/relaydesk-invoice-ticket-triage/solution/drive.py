#!/usr/bin/env python3
"""Gold trajectory for relaydesk-invoice-ticket-triage, through the UI.

Connects to the Chromium that ensure-display.sh (or computer-1) started, so
real input events reach the visible window. VB_ALSO_TOUCH=<ticket id> makes it
also edit a second ticket; the validator uses that to prove the guards fire.
"""

from __future__ import annotations

import os
import subprocess
from pathlib import Path

from playwright.sync_api import sync_playwright

BASE = "http://127.0.0.1:8000"


def set_ticket(page, ticket_id: int, project: str, priority: str) -> None:
    page.goto(f"{BASE}/tickets/{ticket_id}")
    page.select_option("#project", project)
    page.select_option("#priority", priority)
    page.click("text=Save changes")
    page.wait_for_selector("#flash")


def main() -> None:
    with sync_playwright() as p:
        browser = p.chromium.connect_over_cdp("http://127.0.0.1:9222")
        ctx = browser.contexts[0] if browser.contexts else browser.new_context()
        page = ctx.pages[0] if ctx.pages else ctx.new_page()
        page.goto(f"{BASE}/tickets")
        page.fill("input[name=q]", "Kowalczyk")
        page.click("text=Search")
        page.click("text=Charged twice for order A-20931")
        page.select_option("#project", "Billing")
        page.select_option("#priority", "High")
        page.click("text=Save changes")
        page.wait_for_selector("#flash")
        also = os.environ.get("VB_ALSO_TOUCH")
        if also:
            set_ticket(page, int(also), "Billing", "High")
        page.wait_for_timeout(500)
    out_dir = Path("/logs/agent") if Path("/logs/agent").is_dir() else Path("/tmp")
    subprocess.run(
        ["import", "-window", "root", str(out_dir / "gold-final.png")],
        env={**os.environ, "DISPLAY": ":1"},
        check=False,
    )
    print("gold: done")


if __name__ == "__main__":
    main()
