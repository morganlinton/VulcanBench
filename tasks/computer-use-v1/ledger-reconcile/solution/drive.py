#!/usr/bin/env python3
"""Gold trajectory for ledger-reconcile, through the UI.

Pages through the Billing tickets in Relay Desk (Playwright over the visible
Chromium), brings the ledger to the front and scrolls through it, then
brings the editor to the front, types one line per discrepancy from the
answer key and saves with Ctrl+S. The answer key is read from the task's
own ledger.json; the point of the gold is a complete trajectory through all
three windows, not the arithmetic.

VB_MISS=<invoice>    leave that discrepancy out of the report (partial)
VB_EXTRA=1           add a line about an invoice that agrees (false positive)
VB_EDIT_LEDGER=1     also change a ledger cell and save the workbook (guard)
VB_EDIT_TICKET=1     also change a ticket's priority in the tracker (guard)
VB_NO_SAVE=1         type the report but do not save it
"""

from __future__ import annotations

import json
import os
import subprocess
import time
from pathlib import Path

from playwright.sync_api import sync_playwright

DESK = "http://127.0.0.1:8000"
KEY = json.loads(Path("/opt/ledger/ledger.json").read_text())
REPORT = Path("/root/Finance") / KEY["report"]
ENV = {**os.environ, "DISPLAY": ":1"}
LINES = {
    "amount": "{inv}: amount differs, ledger has {ledger:,.2f} and the ticket has {ticket:,.2f}",
    "ledger-only": "{inv}: in the ledger but there is no ticket for it",
    "ticket-only": "{inv}: has a ticket but is not in the ledger",
    "duplicate": "{inv}: entered twice in the ledger",
}


def xdo(*args: str) -> None:
    subprocess.run(["xdotool", *args], check=True, env=ENV)


def key(*keys: str, pause: float = 0.4) -> None:
    xdo("key", "--clearmodifiers", "--delay", "80", *keys)
    time.sleep(pause)


def type_text(text: str, pause: float = 0.3) -> None:
    xdo("type", "--delay", "30", "--", text)
    time.sleep(pause)


def focus(title_part: str) -> None:
    out = subprocess.run(["wmctrl", "-l"], capture_output=True, text=True, env=ENV).stdout
    wid = next((line.split()[0] for line in out.splitlines() if title_part in line), None)
    if wid is None:
        raise SystemExit(f"gold: no window matching {title_part!r}")
    subprocess.run(["wmctrl", "-i", "-a", wid], env=ENV, check=False)
    time.sleep(0.8)


def report_lines() -> list[str]:
    miss = os.environ.get("VB_MISS", "")
    lines = []
    for inv, d in sorted(KEY["discrepancies"].items()):
        if inv == miss:
            continue
        lines.append(LINES[d["kind"]].format(inv=inv, **{k: v for k, v in d.items() if k != "kind"}))
    if os.environ.get("VB_EXTRA"):
        lines.append("INV-2026-0116: amount differs, 2,150.00 against 2,510.00")
    return lines


def main() -> None:
    with sync_playwright() as p:
        browser = p.chromium.connect_over_cdp("http://127.0.0.1:9222")
        ctx = browser.contexts[0] if browser.contexts else browser.new_context()
        page = ctx.pages[0] if ctx.pages else ctx.new_page()
        page.goto(f"{DESK}/tickets?q=INV-2026")
        while True:  # read every page of results, as a person would
            page.wait_for_load_state()
            nxt = page.locator(".pager a", has_text="Next")
            if nxt.count() == 0:
                break
            nxt.click()
        if os.environ.get("VB_EDIT_TICKET"):
            page.goto(f"{DESK}/tickets/6001")
            page.select_option("#priority", "High")
            page.click("text=Save changes")
            page.wait_for_selector("#flash")
    # the ledger: bring Calc to the front and page through it
    focus("LibreOffice Calc")
    for _ in range(3):
        key("Next", pause=0.6)
    key("ctrl+Home", pause=0.5)
    if os.environ.get("VB_EDIT_LEDGER"):
        xdo("mousemove", "70", "145", "click", "--clearmodifiers", "1")
        time.sleep(0.4)
        key("ctrl+a", pause=0.15)
        type_text("D2")
        key("Return", pause=0.6)
        type_text("1")
        key("Return", pause=0.5)
        key("ctrl+s", pause=2.5)
        key("Return", pause=1.0)  # Keep current format
    # the report
    focus("discrepancies.txt")
    key("ctrl+End", pause=0.3)
    for line in report_lines():
        type_text(line)
        key("Return", pause=0.2)
    before = REPORT.stat().st_mtime
    if not os.environ.get("VB_NO_SAVE"):
        key("ctrl+s", pause=1.5)
        for _ in range(20):
            if REPORT.stat().st_mtime != before:
                break
            time.sleep(0.5)
    time.sleep(1.0)
    out_dir = Path("/logs/agent") if Path("/logs/agent").is_dir() else Path("/tmp")
    subprocess.run(["import", "-window", "root", str(out_dir / "gold-final.png")], env=ENV, check=False)
    print(f"gold: wrote {len(report_lines())} lines, saved={REPORT.stat().st_mtime != before}")


if __name__ == "__main__":
    main()
