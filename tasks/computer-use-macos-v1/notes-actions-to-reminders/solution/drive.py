"""notes-actions-to-reminders gold: Reminders only (the gold knows the five
items; an agent has to read them from Notes). New list "Follow-ups", then
one reminder per item with its date typed into the Add Date field.

Layout facts (1280x800 points, the task image's start state): Reminders
opens with a "Never Miss a Reminder" sheet (Continue at 684,582); in a list,
the k-th new reminder's Add Date button is at (572, 243 + 53k) while it is
being edited. Cheat variants: VB_EXTRA=1 adds a colleague's item;
VB_THURSDAY=3/5/26 misreads "by Thursday"; VB_NO_LIST=1 skips the new list.
"""

import os
import sys
import time

os.environ.setdefault("VB_BACKEND", "vnc")
sys.argv = [sys.argv[0], "--backend", "vnc"]
sys.path.insert(0, "/opt/vbdesk")
from vb_computer_mcp import BACKEND as B  # noqa: E402

ITEMS = [
    # Titles short enough not to wrap: a wrapped title moves the Add Date button.
    ("Send funnel numbers to Priya", os.environ.get("VB_THURSDAY", "3/12/26")),
    ("Book usability session", "3/16/26"),
    ("Collect interview feedback", "3/10/26"),
    ("Schedule hiring debrief", "3/18/26"),
    ("Renew staging SSL certificate", "3/20/26"),
]
if os.environ.get("VB_EXTRA") == "1":
    ITEMS.append(("Fix trial-expiry email", "3/13/26"))

B.start(None)
B.key("cmd+space")
time.sleep(1.2)
B.type_text("Reminders")
time.sleep(1.8)
B.key("return")
time.sleep(4)
B.click(684, 582, "left", 1)  # Never Miss a Reminder: Continue
time.sleep(2)
if os.environ.get("VB_NO_LIST") != "1":
    B.key("cmd+shift+n")
    time.sleep(1.5)
    B.type_text("Follow-ups")
    B.key("return")
    time.sleep(1.5)
for k, (title, due) in enumerate(ITEMS):
    B.key("cmd+n")
    time.sleep(1)
    B.type_text(title)
    time.sleep(0.5)
    B.click(572, 243 + 53 * k, "left", 1)  # Add Date
    time.sleep(1)
    B.type_text(due)
    B.key("return")
    time.sleep(1.2)
print("gold done")
