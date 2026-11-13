"""calendar-workshop-update gold: Terminal waits for Priya's correction to
appear in /Users/Shared/Workshop (the launch agent writes it 120 s after
Calendar or TextEdit first runs; the gold's authorize step starts Calendar), then asks Calendar (AppleScript) for the four corrected sessions.
Waiting in the same command line proves the trigger fires.

Cheat variants: VB_STALE=1 makes the draft's sessions without waiting;
VB_NOMOVE=1 applies the new times but leaves session 3 on the 24th;
VB_DUP=1 makes the draft's sessions first and then the corrected ones
without deleting the draft's.
"""

import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import goldlib as G  # noqa: E402

CORR = "/Users/Shared/Workshop/Workshop plan - correction from Priya.txt"
DRAFT = [(17, (9, 30), (11, 0)), (19, (9, 30), (11, 0)), (24, (9, 30), (11, 0)), (26, (9, 30), (11, 0))]
FINAL = [(17, (10, 0), (11, 30)), (19, (10, 0), (11, 30)), (25, (10, 0), (11, 30)), (26, (10, 0), (11, 30))]
if os.environ.get("VB_NOMOVE") == "1":
    FINAL[2] = (24, (10, 0), (11, 30))


def make(plan: list[tuple[int, tuple[int, int], tuple[int, int]]]) -> str:
    lines = ['tell application "Calendar"']
    for n, (d, s, e) in enumerate(plan, 1):
        lines.append(
            f'make new event at end of events of calendar "Work" with properties {{summary:"Onboarding: session {n}", '
            f"start date:{G.as_date(2026, 3, d, *s)}, end date:{G.as_date(2026, 3, d, *e)}}}"
        )
    lines.append("end tell")
    return "osascript " + " ".join(f"-e '{x}'" for x in lines)


G.start()
G.open_terminal()
G.authorize("Calendar", data_prompt=True)
if os.environ.get("VB_STALE") == "1":
    G.run(make(DRAFT), wait=6)
else:
    if os.environ.get("VB_DUP") == "1":
        G.run(make(DRAFT), wait=6)
    G.run(f'until [ -f "{CORR}" ]; do sleep 5; done; echo correction-arrived; ' + make(FINAL), wait=5)
    time.sleep(200)  # the correction lands 120 s after Calendar first ran
    G.shot("after-correction")
G.run("osascript -e 'tell application \"Calendar\" to get summary of events of calendar \"Work\"'", wait=3)
G.shot("final")
print("gold done")
