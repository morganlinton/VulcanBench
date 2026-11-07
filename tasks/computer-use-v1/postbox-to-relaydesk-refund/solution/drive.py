#!/usr/bin/env python3
"""Gold trajectory for postbox-to-relaydesk-refund, through the UI.

Searches Postbox for "refund", opens the newest result (which shows the whole
conversation), then goes to Relay Desk, finds the customer's ticket, adds the
reference as a comment and sets the status, all in the same browser.

VB_WRONG_TICKET=<id>  comment on and update that ticket instead (look-alike check)
VB_REF=<text>         use that reference text (wrong-reference check)
VB_ARCHIVE_MAIL=1     also archive the email (mail guard check)
VB_NO_STATUS=1        add the comment but leave the status alone (partial check)
"""

from __future__ import annotations

import os
import subprocess
from pathlib import Path

from playwright.sync_api import sync_playwright

MAIL = "http://127.0.0.1:8010"
DESK = "http://127.0.0.1:8000"
TICKET = 1016
REFERENCE = "RF-2026-0388"


def main() -> None:
    ticket = int(os.environ.get("VB_WRONG_TICKET") or TICKET)
    ref = os.environ.get("VB_REF") or REFERENCE
    with sync_playwright() as p:
        browser = p.chromium.connect_over_cdp("http://127.0.0.1:9222")
        ctx = browser.contexts[0] if browser.contexts else browser.new_context()
        page = ctx.pages[0] if ctx.pages else ctx.new_page()
        page.goto(f"{MAIL}/search?q=refund")
        page.click("table tr:nth-child(2) td.subj a")  # newest result
        page.wait_for_selector(".card.sel")
        if os.environ.get("VB_ARCHIVE_MAIL"):
            page.click("#archive")
            page.wait_for_selector("#flash")
        page.goto(f"{DESK}/tickets")
        page.fill("input[name=q]", "Olivia Park")
        page.click("text=Search")
        page.goto(f"{DESK}/tickets/{ticket}")
        page.fill("textarea[name=body]", f"Customer's refund reference: {ref} (from her email of 26 February).")
        page.click("text=Add comment")
        page.wait_for_selector("#flash")
        if not os.environ.get("VB_NO_STATUS"):
            page.select_option("#status", "Waiting on Finance")
            page.click("text=Save changes")
            page.wait_for_selector("#flash")
        page.wait_for_timeout(500)
    out_dir = Path("/logs/agent") if Path("/logs/agent").is_dir() else Path("/tmp")
    subprocess.run(["import", "-window", "root", str(out_dir / "gold-final.png")],
                   env={**os.environ, "DISPLAY": ":1"}, check=False)
    print(f"gold: commented {ref} on #{ticket}")


if __name__ == "__main__":
    main()
