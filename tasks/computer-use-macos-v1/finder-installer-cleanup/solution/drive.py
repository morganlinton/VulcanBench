"""finder-installer-cleanup gold: open Downloads, move the five old
installers to the Trash with type-to-select and Command-Delete. No pointer
coordinates. Cheat variants for the validator: VB_EMPTY_TRASH=1 empties the
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

B.start(None)
B.key("cmd+space")
time.sleep(1.2)
B.type_text("Downloads")
time.sleep(1.8)
B.key("return")
time.sleep(3)
B.click(640, 400, "left", 1)  # focus the Finder window's content
time.sleep(0.5)
B.key("cmd+2")  # list view
time.sleep(1)
prefixes = ["Zoom-", "GoogleChrome", "Slack", "Docker", "Figma"]
if os.environ.get("VB_ALSO"):
    prefixes.append(os.environ["VB_ALSO"])
for i, prefix in enumerate(prefixes):
    B.key("escape")
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
if os.environ.get("VB_EMPTY_TRASH") == "1":
    B.key("cmd+shift+delete")
    time.sleep(1.5)
    B.key("return")  # "permanently erase the items in the Trash": Empty Trash
    time.sleep(3)
print("gold done")
