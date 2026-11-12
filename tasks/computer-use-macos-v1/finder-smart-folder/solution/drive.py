"""finder-smart-folder gold: Finder only. Documents window (Shift-Command-O),
File > New Smart Folder, scope "Documents", + Kind is PDF, + Name contains
"invoice", Save as "Client invoices" with Add To Sidebar left on.

Layout facts (1280x800 points, the task image's start state): File menu
(124,14), New Smart Folder (169,120); in the smart folder window the scope
button "Documents" is at (575,192), the row + buttons at (1102,192) and
(1102,224); a row's attribute, operator and value controls are at x 429,
506 and 553 (text field 672); Save (1060,192), the save sheet's Add To
Sidebar (619,408) and Save (793,457). Popup menus open with the checked
item at the control. Cheat variants: VB_SCOPE=mac keeps "This Mac";
VB_NO_KIND=1 makes the only row Name contains; VB_NO_SIDEBAR=1 unticks
Add To Sidebar.
"""

import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import goldlib as G  # noqa: E402

B = G.B


def pick(x: int, y: int) -> None:
    B.move(x, y - 5)
    time.sleep(0.4)
    B.click(x, y, "left", 1)
    time.sleep(1.2)


def name_contains(row_y: int, menu_checked_y: int) -> None:
    """Turn the row at row_y into Name contains "invoice"; its attribute
    menu opens with the current attribute at the row (menu_checked_y)."""
    B.click(429, row_y, "left", 1)
    time.sleep(1.5)
    pick(423, row_y + (323 - menu_checked_y))  # Name
    B.click(506, row_y, "left", 1)  # operator, "is" checked
    time.sleep(1.5)
    pick(510, row_y - 65)  # contains
    B.click(672, row_y, "left", 1)
    B.type_text("invoice")
    time.sleep(2)


G.start()
B.click(72, 762, "left", 1)  # Finder in the Dock
time.sleep(4)
B.key("cmd+shift+o")  # Documents
time.sleep(4)
# Focus the Finder window first: a click meant for the File menu that lands
# on the desktop can hit a widget (one run opened Weather).
B.click(640, 118, "left", 1)  # the Documents window's title bar
time.sleep(1)
G.shot("documents")
B.click(124, 14, "left", 1)  # File
time.sleep(1.5)
B.click(169, 120, "left", 1)  # New Smart Folder
time.sleep(3)
if os.environ.get("VB_SCOPE") != "mac":
    B.click(575, 192, "left", 1)  # Search: "Documents"
    time.sleep(1.5)
B.click(1102, 192, "left", 1)  # + : Kind is Any
time.sleep(1.5)
if os.environ.get("VB_NO_KIND") == "1":
    name_contains(224, 234)  # the menu's Kind row sits at the control
else:
    B.click(553, 224, "left", 1)  # Kind value
    time.sleep(1.5)
    pick(517, 423)  # PDF
    B.click(1102, 224, "left", 1)  # + : Last opened date
    time.sleep(1.5)
    name_contains(256, 256)
G.shot("criteria")
B.click(1060, 192, "left", 1)  # Save
time.sleep(3)  # the sheet slides in; keys typed earlier are lost
B.key("cmd+a")
B.type_text("Client invoices")
time.sleep(0.5)
if os.environ.get("VB_NO_SIDEBAR") == "1":
    B.click(619, 408, "left", 1)
    time.sleep(0.5)
G.shot("save-sheet")
B.click(793, 457, "left", 1)
time.sleep(3)
G.shot("final")
print("gold done")
