"""settings-workday-setup gold: System Settings from the Dock, Desktop & Dock
(position pop-up, two switches, wallpaper pop-up), then General > Date &
Time (24-hour switch). Pop-up menus are answered by typing their item.

Layout facts (1280x800 points, the task image's start state): System
Settings' Dock icon at (1005,762); sidebar General (339,411) and Desktop &
Dock (364,507). In Desktop & Dock: position pop-up (935,258), auto-hide
switch (961,425), recent apps switch (961,543), magnification slider knob
at (769,193), wallpaper pop-up (935,680), Stage Manager "In Stage Manager"
checkbox (860,640). In General: Date & Time (607,655); 24-hour switch
(961,278). Cheat variants: VB_POSITION=Right; VB_MAGNIFY=1 drags the
magnification slider up; VB_STAGE=1 ticks Stage Manager instead of the
wallpaper setting.
"""

import os
import sys
import time

os.environ.setdefault("VB_BACKEND", "vnc")
sys.argv = [sys.argv[0], "--backend", "vnc"]
sys.path.insert(0, "/opt/vbdesk")
from vb_computer_mcp import BACKEND as B  # noqa: E402

B.start(None)
B.click(1005, 762, "left", 1)  # System Settings in the Dock
time.sleep(4)
B.click(364, 507, "left", 1)  # Desktop & Dock
time.sleep(2)
B.click(935, 258, "left", 1)  # Dock position on screen
time.sleep(1)
B.type_text(os.environ.get("VB_POSITION", "Left"))
B.key("return")
time.sleep(1.5)
B.click(961, 425, "left", 1)  # Automatically hide and show the Dock
time.sleep(1.2)
B.click(961, 543, "left", 1)  # Show suggested and recent apps in Dock
time.sleep(1.2)
if os.environ.get("VB_MAGNIFY") == "1":
    B.drag(769, 193, 870, 193)
    time.sleep(1)
if os.environ.get("VB_STAGE") == "1":
    B.click(860, 640, "left", 1)  # Show items: In Stage Manager
    time.sleep(1.2)
else:
    B.click(935, 680, "left", 1)  # Click wallpaper to show desktop
    time.sleep(1)
    B.type_text("Only")
    B.key("return")
    time.sleep(1.5)
B.click(339, 411, "left", 1)  # General
time.sleep(2)
B.click(607, 655, "left", 1)  # Date & Time
time.sleep(2)
B.click(961, 278, "left", 1)  # 24-hour time
time.sleep(1.5)
print("gold done")
