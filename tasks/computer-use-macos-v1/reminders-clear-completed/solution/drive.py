"""reminders-clear-completed gold: Terminal asks Reminders (AppleScript) to
delete the six completed Work reminders that are not for the audit, one by
one (the list's Clear would take the audit ones too).

Cheat variants: VB_CLEAR=list deletes every completed Work reminder;
VB_CLEAR=all deletes every completed reminder in every list; VB_SKIP=2
leaves the last two behind; VB_REOPEN=1 also marks the audit ones not done.
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import goldlib as G  # noqa: E402

W = 'list "Work"'
DONE = [
    "Submit February timesheet",
    "Send onboarding pack to Marcus",
    "Order new monitor",
    "Update the team wiki page",
    "Schedule 1:1 with Elena",
    "Cancel the old Zoom plan",
]
AUDIT = [
    "Send expense receipts to the auditors",
    "Export January bank statements for the audit",
    "Sign the audit engagement letter",
]
clear = os.environ.get("VB_CLEAR")
skip = int(os.environ.get("VB_SKIP", "0"))
lines = ['tell application "Reminders"']
if clear == "list":
    lines.append(f"delete (every reminder of {W} whose completed is true)")
elif clear == "all":
    lines.append("delete (every reminder whose completed is true)")
else:
    lines += [f'delete (first reminder of {W} whose name is "{t}")' for t in DONE[: len(DONE) - skip]]
if os.environ.get("VB_REOPEN") == "1":
    lines += [f'set completed of (first reminder of {W} whose name is "{t}") to false' for t in AUDIT]
lines.append("end tell")

G.start()
G.open_terminal()
G.authorize("Reminders", data_prompt=True)
G.run("osascript " + " ".join(f"-e '{x}'" for x in lines), wait=5)
G.run(f"osascript -e 'tell application \"Reminders\" to get name of reminders of {W}'", wait=3)
G.shot("final")
print("gold done")
