"""notes-folder-cleanup gold: Terminal asks Notes (AppleScript) to move the
twelve notes to their folders and delete the three empty notes (Notes moves
them to Recently Deleted). The gold knows which note goes where; an agent
has to read them.

Cheat variants: VB_TOKYO=recipes files the Tokyo restaurant list as a
recipe; VB_KEEP_EMPTY=1 leaves the empty notes; VB_DINNER=1 also files the
dinner party guest list under Recipes.
"""

import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import goldlib as G  # noqa: E402

MOVES = {
    "Recipes": ["Lemon ricotta pancakes", "Weeknight chili", "Grandma's banana bread", "Miso salmon"],
    "Travel": ["Packing list: Japan", "Lisbon trip ideas", "Rental car confirmation"],
    "Work": ["1:1 notes with Priya", "Q2 hiring plan", "Offsite agenda draft", "Vendor contract questions"],
}
MOVES["Recipes" if os.environ.get("VB_TOKYO") == "recipes" else "Travel"].append("Tokyo restaurant list")
if os.environ.get("VB_DINNER") == "1":
    MOVES["Recipes"].append("Dinner party guest list")


def notes(lines: list[str], wait: float = 3) -> None:
    """One osascript with a `tell application "Notes"` block of `lines`.
    Titles with an apostrophe are matched with `name begins with`."""
    body = " ".join(f"-e {chr(39)}{ln}{chr(39)}" for ln in lines)
    G.run(f"osascript -e 'tell application \"Notes\"' {body} -e 'end tell'", wait=wait)


G.start()
G.open_terminal()
first = True
for folder, titles in MOVES.items():
    lines = []
    for t in titles:
        sel = f'name is "{t}"' if "'" not in t else f'name begins with "{t.split(chr(39))[0]}"'
        lines.append(f'move (first note of folder "Notes" whose {sel}) to folder "{folder}"')
    notes([ln.replace("'", "") for ln in lines], wait=4 if first else 3)
    if first:
        G.allow_prompt(*G.AUTOMATION_ALLOW, name="automation-prompt")
        time.sleep(4)
        first = False
if os.environ.get("VB_KEEP_EMPTY") != "1":
    notes(['repeat with n in (every note of folder "Notes" whose plaintext is "")', "delete n", "end repeat"], wait=4)
G.run("osascript -e 'tell application \"Notes\" to get name of every note of folder \"Notes\"'", wait=3)
G.shot("final")
print("gold done")
