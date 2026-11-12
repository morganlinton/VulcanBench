"""notes-smoke gold: open Notes, select Trips, new note, type the list.

Coordinates are 1280x800 points on the task image's start state (Notes
window at its default place; sidebar: All on My Mac, Notes, Trips, Work).
Cheat variants for the validator: VB_SKIP_FOLDER=1 leaves the note in the
default folder; VB_ITEMS overrides the typed lines.
"""

import os
import sys
import time

os.environ.setdefault("VB_BACKEND", "vnc")
sys.argv = [sys.argv[0], "--backend", "vnc"]
sys.path.insert(0, "/opt/vbdesk")
from vb_computer_mcp import BACKEND  # noqa: E402

BACKEND.start(None)
B = BACKEND
B.key("cmd+space")
time.sleep(1.2)
B.type_text("Notes")
time.sleep(1.5)
B.key("return")
time.sleep(4)
if os.environ.get("VB_SKIP_FOLDER") != "1":
    B.click(195, 206, "left", 1)  # Trips
    time.sleep(1)
B.key("cmd+n")
time.sleep(1.5)
items = os.environ.get("VB_ITEMS", "passport\nphone charger\nrain jacket")
B.type_text("Packing list\n" + items)
time.sleep(2)
print("gold done")
