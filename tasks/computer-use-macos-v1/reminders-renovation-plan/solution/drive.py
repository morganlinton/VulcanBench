"""reminders-renovation-plan gold: Terminal asks Reminders (AppleScript) to
make the Ordering list, move the three Order reminders into it, flag two,
set high priority on one and complete one.

Cheat variants: VB_MOVE_TWO=1 moves only two; VB_DELETE_PAINT=1 deletes the
paint reminder instead of completing it; VB_PRIORITY=5 sets medium.
"""

import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import goldlib as G  # noqa: E402

K = 'list "Kitchen renovation"'
orders = ["Order cabinets", "Order countertop", "Order sink and faucet"]
if os.environ.get("VB_MOVE_TWO") == "1":
    orders = orders[:2]
prio = os.environ.get("VB_PRIORITY", "1")
paint = (
    f'delete (first reminder of {K} whose name is "Pick paint color")'
    if os.environ.get("VB_DELETE_PAINT") == "1"
    else f'set completed of (first reminder of {K} whose name is "Pick paint color") to true'
)
lines = ['tell application "Reminders"', 'make new list with properties {name:"Ordering"}']
lines += [f'move (first reminder of {K} whose name is "{t}") to list "Ordering"' for t in orders]
lines += [
    f'set flagged of (first reminder of {K} whose name is "Choose a contractor") to true',
    f'set flagged of (first reminder of {K} whose name is "Book electrician for new outlets") to true',
    f'set priority of (first reminder of {K} whose name is "Get three contractor quotes") to {prio}',
    paint,
    "end tell",
]

G.start()
G.open_terminal()
G.run("osascript -e 'tell application \"Reminders\" to count lists'", wait=4)
G.allow_prompt(*G.AUTOMATION_ALLOW, name="automation-prompt")
time.sleep(3)
G.allow_prompt(*G.DATA_ALLOW, name="reminders-data-prompt")
time.sleep(3)
G.run("osascript " + " ".join(f"-e '{x}'" for x in lines), wait=5)
G.run("osascript -e 'tell application \"Reminders\" to get name of reminders of list \"Ordering\"'", wait=3)
G.shot("final")
print("gold done")
