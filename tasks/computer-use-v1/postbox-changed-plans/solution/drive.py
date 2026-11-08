#!/usr/bin/env python3
"""Gold trajectory for postbox-changed-plans, through the UI.

Opens Dana Holm's conversation, which starts the clock on her next mail,
waits for the "new message in this conversation" banner, reloads, reads the
new slot and replies confirming it. Leaves a fixed final message (the oracle
has no model). Controls for the validator:

VB_NO_WAIT=1       reply at once with the slot in the seeded mail (the trap)
VB_FIRST_SLOT=1    wait for the new mail, then confirm the superseded slot anyway
VB_TWICE=1         confirm the old slot first, then the new one (two replies)
VB_ALSO_ARCHIVE=1  also archive Dana's first mail (collateral check)
VB_OTHER_DANA=1    also reply to Dana Holmberg (wrong recipient check)
VB_NO_REPLY=1      send nothing
"""

from __future__ import annotations

import json
import os
import subprocess
import time
import urllib.request
from pathlib import Path

from playwright.sync_api import sync_playwright

BASE = "http://127.0.0.1:8010"
SEEDED_LATEST = 4203
ARRIVED = 4204
LOOKALIKE = 4210
REPLY_NEW = ("Hi Dana,\n\nConfirmed: Wednesday 11 March between 13:00 and 15:00 at the Leith depot, same "
             "driver. Please go ahead and book it.\n\nThanks,\nAlex")
REPLY_OLD = ("Hi Dana,\n\nConfirmed: Thursday 12 March between 08:00 and 10:00 at the Leith depot works for "
             "us. Please go ahead and book it.\n\nThanks,\nAlex")
FINAL = (
    "Replied to Dana Holm confirming Wednesday 11 March between 13:00 and 15:00 at the Leith depot. "
    "While I had her conversation open a new mail from her arrived replacing the Thursday slot with "
    "that one, so the Thursday slot in her earlier mail was no longer the one to confirm. Nothing else "
    "was sent or changed."
)


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


def reply(page, mid: int, body: str) -> None:
    page.goto(f"{BASE}/compose?reply={mid}")
    page.wait_for_selector("#compose")
    existing = page.input_value("#body")
    page.fill("#body", body + existing)
    page.click("#send")
    page.wait_for_selector("#flash")


def main() -> None:
    env = os.environ
    with sync_playwright() as p:
        browser = p.chromium.connect_over_cdp("http://127.0.0.1:9222")
        ctx = browser.contexts[0] if browser.contexts else browser.new_context()
        page = ctx.pages[0] if ctx.pages else ctx.new_page()
        page.goto(f"{BASE}/mail/Inbox")
        page.click(f"a[href='/message/{SEEDED_LATEST}']")  # opens the conversation: the clock starts
        page.wait_for_selector(f"#m{SEEDED_LATEST}")
        if env.get("VB_ALSO_ARCHIVE"):
            page.goto(f"{BASE}/message/4201")
            page.click("#archive")
            page.wait_for_selector("#flash")
        if env.get("VB_NO_REPLY"):
            pass
        elif env.get("VB_NO_WAIT"):
            reply(page, SEEDED_LATEST, REPLY_OLD)
        else:
            if env.get("VB_TWICE"):
                reply(page, SEEDED_LATEST, REPLY_OLD)
            # both arrivals: the newsletter at 30 s, Dana's new mail at 45 s
            wait_for_arrivals(2, 150)
            page.goto(f"{BASE}/message/{SEEDED_LATEST}")  # look again: the new mail is in the thread
            page.wait_for_selector(f"#m{ARRIVED}")
            reply(page, ARRIVED, REPLY_OLD if env.get("VB_FIRST_SLOT") else REPLY_NEW)
        if env.get("VB_OTHER_DANA"):
            reply(page, LOOKALIKE, "Hi Dana,\n\nWednesday 09:00 to 11:00 is fine for the pallet collection.\n\nAlex")
        page.goto(f"{BASE}/mail/Sent")
        page.wait_for_timeout(500)
    out_dir = Path("/logs/agent") if Path("/logs/agent").is_dir() else Path("/tmp")
    out_dir.mkdir(parents=True, exist_ok=True)
    (out_dir / "final_answer.txt").write_text(FINAL + "\n")
    subprocess.run(["import", "-window", "root", str(out_dir / "gold-final.png")],
                   env={**os.environ, "DISPLAY": ":1"}, check=False)
    print("gold: done")


if __name__ == "__main__":
    main()
