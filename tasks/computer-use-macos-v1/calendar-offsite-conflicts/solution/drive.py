"""calendar-offsite-conflicts gold: Week view, next week, drag each clashing
occurrence from its day to Thursday at the same time, "Only This Event".

Layout facts (1280x800 points, the task image's start state): Week and the
next-week arrow at (680,56) and (1254,110). Scrolled to the top, the week
grid puts midnight at y=228 and an hour every 35.1 points; one scroll line
is 10 points and a wheel click (the scroll tool's unit) is 3 lines. Day columns: Tue x=586, Wed 741, Thu 896, Fri 1051. The
recurring-event dialog's "Only This Event" is at (710,366) and "All Future
Events" at (710,400). Cheat variants: VB_FUTURE=1 answers "All Future
Events" for Alex; VB_SKIP_PRIYA=1 leaves Priya; VB_ALEX_DAY=fri moves Alex
to Friday.
"""

import os
import sys
import time

os.environ.setdefault("VB_BACKEND", "vnc")
sys.argv = [sys.argv[0], "--backend", "vnc"]
sys.path.insert(0, "/opt/vbdesk")
from vb_computer_mcp import BACKEND as B  # noqa: E402

COL = {"tue": 586, "wed": 741, "thu": 896, "fri": 1051}
B.start(None)
B.key("cmd+space")
time.sleep(1.2)
B.type_text("Calendar")
time.sleep(1.8)
B.key("return")
time.sleep(4)
B.click(680, 56, "left", 1)  # Week
time.sleep(1.5)
B.click(1254, 110, "left", 1)  # next week: 15 to 21 March
time.sleep(1.5)
B.move(741, 450)
B.scroll(741, 450, "up", 20)
B.scroll(741, 450, "up", 20)  # 1200 points up: the top, midnight at y=228
time.sleep(1)

# Alex, Tuesday 10:00 (drawn at y about 590 with the grid at the top).
alex_to = COL[os.environ.get("VB_ALEX_DAY", "thu")]
B.drag(COL["tue"] - 40, 590, alex_to - 40, 590)
time.sleep(1.5)
B.click(710, 400 if os.environ.get("VB_FUTURE") == "1" else 366, "left", 1)
time.sleep(1.5)

if os.environ.get("VB_SKIP_PRIYA") != "1":
    B.scroll(741, 450, "down", 7)  # 210 points: 14:00 moves from 719 to 509
    time.sleep(1)
    B.drag(COL["wed"] - 20, 518, COL["thu"] - 20, 518)
    time.sleep(1.5)
    B.click(710, 366, "left", 1)  # Only This Event
    time.sleep(1.5)
print("gold done")
