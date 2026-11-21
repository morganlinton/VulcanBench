"""contacts-duplicate-merge gold: for each duplicated person, search
Contacts for the last name, select both results (click the first, then
Shift-click the second) and Card > Merge Selected Cards (Shift-Command-L).
The gold knows the four pairs; an agent has to find them, including the two
whose names differ (Bob and Robert Alvarez, Jen and Jennifer Okafor).

Layout facts (1280x800 points, the task image's start state): Contacts in
the Dock at (590,762); the search field at (470,155); with a search the
first result row is at (470,210) and the second at (470,262). Merging keeps
both cards' phones, emails and notes (the other card's name is appended to
the notes). Cheat variants: VB_MERGE_CHRIS=1 also merges the two different
Chris Parks; VB_EXACT_ONLY=1 merges only the same-name pairs;
VB_DELETE_TOM=1 deletes one Tom Becker card instead of merging.
"""

import os
import sys
import time

os.environ.setdefault("VB_BACKEND", "vnc")
sys.argv = [sys.argv[0], "--backend", "vnc"]
sys.path.insert(0, "/opt/vbdesk")
from vb_computer_mcp import BACKEND as B  # noqa: E402

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import goldlib  # noqa: E402, F401 - paced sleeps (VB_GOLD_PACE)


def shot(name: str) -> None:
    try:
        with open(f"/logs/agent/gold-{name}.webp", "wb") as f:
            f.write(B.grab())
    except OSError:
        pass


def search(text: str) -> None:
    # Bring Contacts forward first: in one run the desktop was revealed
    # (all windows moved aside) between steps; clicking the app's Dock icon
    # brings its window back.
    B.click(590, 762, "left", 1)
    time.sleep(1.5)
    B.click(470, 155, "left", 1)
    time.sleep(0.5)
    B.key("cmd+a")
    B.type_text(text)
    time.sleep(2)


def merge_pair(last: str) -> None:
    search(last)
    B.click(470, 210, "left", 1)
    time.sleep(0.8)
    B.click(470, 262, "left", 1, "shift")
    time.sleep(0.8)
    shot(f"{last}-selected")
    B.key("cmd+shift+l")  # Card > Merge Selected Cards
    time.sleep(2.5)


B.start(None)
B.click(590, 762, "left", 1)  # Contacts in the Dock
time.sleep(12)  # its first launch in a trial is slow
shot("opened")
pairs = ["Gonzalez", "Becker"]
if os.environ.get("VB_EXACT_ONLY") != "1":
    pairs += ["Alvarez", "Okafor"]
if os.environ.get("VB_MERGE_CHRIS") == "1":
    pairs.append("Park")
for last in pairs:
    if last == "Becker" and os.environ.get("VB_DELETE_TOM") == "1":
        search(last)
        B.click(470, 262, "left", 1)
        time.sleep(0.8)
        B.key("delete")
        time.sleep(1.5)
        shot("delete-confirm")
        B.key("return")
        time.sleep(2)
        continue
    merge_pair(last)
search("")
B.key("escape")
time.sleep(1)
shot("final")
print("gold done")
