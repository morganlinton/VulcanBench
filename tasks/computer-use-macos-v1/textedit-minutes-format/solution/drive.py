"""textedit-minutes-format gold: Terminal writes the formatted minutes as
HTML and converts it to RTF with textutil, which uses the same AppKit
rich-text writer TextEdit saves with (bold runs, \\fs36 for 18 point, \\lsN
lists). textutil reads CSS points at 4/3 scale, so 13.5pt and 9pt give 18
and 12 point.

Cheat variants: VB_BIG=1 makes the heading 24 point; VB_DASH=1 writes the
items as "- " lines instead of a list; VB_ALLBOLD=1 also bolds the
attendees line.
"""

import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import goldlib as G  # noqa: E402

size = "18pt" if os.environ.get("VB_BIG") == "1" else "13.5pt"
att = "<p><b>Attendees: Jordan, Priya, Marcus, Elena</b></p>" if os.environ.get("VB_ALLBOLD") == "1" else (
    "<p>Attendees: Jordan, Priya, Marcus, Elena</p>")


def items(xs: list[str]) -> str:
    if os.environ.get("VB_DASH") == "1":
        return "".join(f"<p>- {x}</p>" for x in xs)
    return "<ul>" + "".join(f"<li>{x}</li>" for x in xs) + "</ul>"


html = (
    '<html><body style="font-family: Helvetica; font-size: 9pt">'
    f'<p><b><span style="font-size:{size}">Product sync, 9 March 2026</span></b></p>{att}'
    "<p><b>Decisions</b></p>" + items(["Ship the onboarding redesign on 23 March.", "Keep the trial at 14 days."])
    + "<p><b>Action items</b></p>"
    + items(["Priya: update the pricing page copy.", "Marcus: fix the trial-expiry email.", "Elena: draft the Q2 pricing memo."])
    + "<p>Next meeting: Monday 16 March</p></body></html>"
)

G.start()
G.open_terminal()
G.run(f"printf '%s' '{html}' > /tmp/minutes.html && echo written", wait=2)
G.run('textutil -convert rtf -output ~/Documents/"Minutes 2026-03-09.rtf" /tmp/minutes.html', wait=4)
G.allow_prompt(*G.FOLDER_ALLOW, name="documents-prompt")
time.sleep(2)
G.run('head -c 300 ~/Documents/"Minutes 2026-03-09.rtf"', wait=2)
G.shot("final")
print("gold done")
