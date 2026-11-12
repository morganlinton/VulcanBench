"""settings-corners-shortcuts gold: System Settings, opened once from Terminal.

Desktop & Dock: Minimized window animation -> Scale Effect; Hot Corners...:
bottom-right -> Lock Screen, top-left -> Mission Control chosen with Command
held (the menu is scrolled up first by hovering its top arrow). Each corner
is set from a freshly opened Hot Corners sheet, closed with Escape: after a
pick the sheet's Done button stopped responding in testing, while Escape
closes it and keeps the choice. Keyboard: Keyboard Shortcuts... >
App Shortcuts > + (Notes, "Checklist", Control-Command-L), then Text
Replacements... > + ("addr;"). Settings is reopened on the Keyboard pane
from Terminal (`open x-apple.systempreferences:...`).

Layout facts (1280x800 points, the task image's start state) are the
coordinates below. Cheat variants: VB_NO_CMD=1 picks Mission Control
without Command; VB_ALL_APPS=1 leaves the shortcut on All Applications;
VB_TR_LOCK=1 also sets the top-right corner to Lock Screen.
"""

import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import goldlib as G  # noqa: E402

B = G.B


def pick(x: int, y: int, mods: str = "") -> None:
    """Hover a menu item, then click it (optionally with keys held)."""
    B.move(x, y - 6)
    time.sleep(0.3)
    B.move(x, y)
    time.sleep(0.5)
    B.click(x, y, "left", 1, mods)
    time.sleep(1.5)


def hot_corners() -> None:
    B.click(903, 658, "left", 1)
    time.sleep(2.5)


G.start()
B.click(1005, 762, "left", 1)  # System Settings in the Dock
time.sleep(12)
B.move(640, 400)
time.sleep(1)
B.click(365, 507, "left", 1)  # Desktop & Dock
time.sleep(3)
B.click(920, 298, "left", 1)  # Minimized window animation
time.sleep(1.5)
pick(911, 322)  # Scale Effect
B.scroll(755, 450, "down", 20)
time.sleep(1)
B.scroll(755, 450, "down", 20)
time.sleep(2)
G.shot("dock-bottom")

hot_corners()
B.click(812, 368, "left", 1)  # bottom-right
time.sleep(1.5)
pick(756, 497)  # Lock Screen
B.key("escape")
time.sleep(1.5)

hot_corners()
B.click(467, 322, "left", 1)  # top-left: the menu opens scrolled to its end
time.sleep(1.5)
B.move(455, 20)  # hover the top arrow until the menu scrolls to its start
time.sleep(2.5)
B.move(455, 120)
time.sleep(0.5)
G.shot("tl-menu")
pick(422, 47, "" if os.environ.get("VB_NO_CMD") == "1" else "cmd")  # Mission Control
B.key("escape")
time.sleep(1.5)

if os.environ.get("VB_TR_LOCK") == "1":
    hot_corners()
    B.click(812, 322, "left", 1)  # top-right (Notification Center selected)
    time.sleep(1.5)
    pick(756, 521)  # Lock Screen
    B.key("escape")
    time.sleep(1.5)

# Reopen Settings straight into Keyboard: switching panes in place left the
# previous pane's content on screen for several seconds in testing.
B.key("cmd+q")
time.sleep(3)
G.open_terminal()
G.run('open "x-apple.systempreferences:com.apple.Keyboard-Settings.extension"', wait=10)
G.shot("keyboard-pane")
B.click(901, 352, "left", 1)  # Keyboard Shortcuts...
time.sleep(2.5)
B.click(392, 521, "left", 1)  # App Shortcuts
time.sleep(2)
B.click(537, 254, "left", 1)  # +
time.sleep(2)
if os.environ.get("VB_ALL_APPS") != "1":
    B.click(790, 280, "left", 1)  # Application popup
    time.sleep(1.5)
    B.type_text("Notes")
    time.sleep(1)
    B.key("return")
    time.sleep(1)
B.click(760, 328, "left", 1)  # Menu title
B.type_text("Checklist")
time.sleep(0.5)
B.click(815, 412, "left", 1)  # Keyboard shortcut field
time.sleep(0.5)
B.key("ctrl+cmd+l")
time.sleep(1)
G.shot("app-shortcut")
B.click(836, 483, "left", 1)  # Done (the add sheet)
time.sleep(2)
B.click(922, 579, "left", 1)  # Done (Keyboard Shortcuts)
time.sleep(2)

B.click(904, 500, "left", 1)  # Text Replacements...
time.sleep(2.5)
B.click(436, 371, "left", 1)  # +
time.sleep(1)
B.type_text("addr;")
B.key("tab")
B.type_text("1 Harbor Way, Boston, MA 02110")
B.key("return")
time.sleep(1.5)
G.shot("replacements")
B.click(822, 448, "left", 1)  # Done
time.sleep(2)
B.key("cmd+q")
time.sleep(2)
G.shot("final")
print("gold done")
