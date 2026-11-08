#!/usr/bin/env python3
"""Gold trajectory for postbox-newsletter-cleanup, through the UI.

Connects to the Chromium that ensure-display.sh (or computer-1) started.
Walks the Newsletters folder and the Inbox, ticks every Fathom Analytics
newsletter dated before February, and uses the bulk Archive button (accepting
the confirm dialog), repeating a folder until no target is left on it.

VB_ALSO_TOUCH=<id>  also archive that message (validator guard check)
VB_SKIP=<id>        leave that target alone (partial work check)
VB_UNSUB=1          also unsubscribe from the first newsletter (guard check)
VB_TRASH=1          use Delete instead of Archive (wrong-action check)
"""

from __future__ import annotations

import os
import subprocess
from pathlib import Path

from playwright.sync_api import sync_playwright

BASE = "http://127.0.0.1:8010"
TARGETS = {3002, 3005, 3009, 3012, 3016, 3019, 3023, 3026, 3030}


def archive_targets_in(page, folder: str, targets: set[int], action: str) -> int:
    done = 0
    for _ in range(6):  # pages shrink as messages leave the folder
        page.goto(f"{BASE}/mail/{folder}")
        ticked = 0
        for tid in sorted(targets):
            box = page.locator(f"input[name=sel][value='{tid}']")
            if box.count():
                box.check()
                ticked += 1
        if ticked:
            page.click(f"#bulk button[value={action}]")
            page.wait_for_selector("#flash")
            done += ticked
            continue
        nxt = page.locator(".pager a", has_text="Next")
        if nxt.count() == 0:
            break
        # look on the next page before giving up on this folder
        nxt.click()
        page.wait_for_load_state()
        found = [tid for tid in targets if page.locator(f"input[name=sel][value='{tid}']").count()]
        if not found:
            break
        for tid in found:
            page.locator(f"input[name=sel][value='{tid}']").check()
        page.click(f"#bulk button[value={action}]")
        page.wait_for_selector("#flash")
        done += len(found)
    return done


def main() -> None:
    skip = int(os.environ.get("VB_SKIP", "0") or 0)
    targets = {t for t in TARGETS if t != skip}
    also = os.environ.get("VB_ALSO_TOUCH")
    if also:
        targets.add(int(also))
    action = "trash" if os.environ.get("VB_TRASH") else "archive"
    with sync_playwright() as p:
        browser = p.chromium.connect_over_cdp("http://127.0.0.1:9222")
        ctx = browser.contexts[0] if browser.contexts else browser.new_context()
        page = ctx.pages[0] if ctx.pages else ctx.new_page()
        page.on("dialog", lambda d: d.accept())
        page.goto(f"{BASE}/mail/Inbox")
        # read one of them first, as a person would
        page.click("a[href='/message/3001']")  # the newest Fathom mail, on page 1
        page.wait_for_selector("#m3001")
        n = archive_targets_in(page, "Newsletters", targets, action)
        n += archive_targets_in(page, "Inbox", targets, action)
        if os.environ.get("VB_UNSUB"):
            page.goto(f"{BASE}/message/3001")
            page.click("a[href='/unsubscribe/3001']")
            page.click("#unsubscribe")
            page.wait_for_selector("#flash")
        page.goto(f"{BASE}/mail/Archive")
        page.wait_for_timeout(500)
    out_dir = Path("/logs/agent") if Path("/logs/agent").is_dir() else Path("/tmp")
    subprocess.run(["import", "-window", "root", str(out_dir / "gold-final.png")],
                   env={**os.environ, "DISPLAY": ":1"}, check=False)
    print(f"gold: {action}d {n} messages")


if __name__ == "__main__":
    main()
