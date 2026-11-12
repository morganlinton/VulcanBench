"""finder-installer-cleanup gold: open Downloads, move the five old
installers to the Trash with type-to-select and Command-Delete. Finder is
opened from the Dock (72,762). Cheat variants for the validator: VB_EMPTY_TRASH=1 empties the
Trash afterwards; VB_DELETE_NOW=1 deletes one installer immediately
(Option-Command-Delete); VB_ALSO="Notion" also trashes the recent installer.
"""

import os
import sys
import time

os.environ.setdefault("VB_BACKEND", "vnc")
sys.argv = [sys.argv[0], "--backend", "vnc"]
sys.path.insert(0, "/opt/vbdesk")
from vb_computer_mcp import BACKEND as B  # noqa: E402



def shot(name: str) -> None:
    """Keep a screenshot of each step with the oracle's logs, for diagnosis."""
    try:
        with open(f"/logs/agent/gold-{name}.webp", "wb") as f:
            f.write(B.grab())
    except OSError:
        pass


B.start(None)
time.sleep(8)  # the session has just come up; let Finder and the Dock settle
shot("start")
# Finder from the Dock, then Go > Downloads (Option-Command-L): Spotlight's
# index can still be settling right after boot, so the gold avoids it.
B.click(72, 762, "left", 1)
time.sleep(2.5)
B.key("option+cmd+l")
time.sleep(2.5)
shot("downloads")
B.click(640, 470, "left", 1)  # empty list area below the rows: focus the file list
time.sleep(0.5)
B.key("cmd+2")  # list view
time.sleep(1)
prefixes = ["Zoom-", "GoogleChrome", "Slack", "Docker", "Figma"]
if os.environ.get("VB_ALSO"):
    prefixes.append(os.environ["VB_ALSO"])
for i, prefix in enumerate(prefixes):
    # Clicking the empty area below the rows deselects and puts keyboard
    # focus on the file list (a trial once typed into the sidebar instead).
    B.click(640, 470, "left", 1)
    time.sleep(1.2)  # type-select resets after a pause
    B.type_text(prefix)
    time.sleep(0.8)
    if os.environ.get("VB_DELETE_NOW") == "1" and i == 0:
        B.key("option+cmd+delete")
        time.sleep(1.5)
        B.click(699, 372, "left", 1)  # "This item will be deleted immediately": Delete (not default)
    else:
        B.key("cmd+delete")
    time.sleep(1.2)
    shot(f"after-{i}")
if os.environ.get("VB_EMPTY_TRASH") == "1":
    B.key("cmd+shift+delete")
    time.sleep(1.5)
    B.key("return")  # "permanently erase the items in the Trash": Empty Trash
    time.sleep(3)
print("gold done")
