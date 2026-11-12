"""notes-signatures-to-contacts gold: Terminal asks Contacts (AppleScript)
to make the three cards with titles, companies, labeled phones, emails and
addresses. The gold knows the fields; an agent has to read the note.

Cheat variants: VB_SWAP=1 swaps Tomas's work and mobile labels;
VB_NO_ADDR=1 leaves Grace's address out; VB_NO_TITLE=1 leaves Priyanka's
job title out.
"""

import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import goldlib as G  # noqa: E402

swap = os.environ.get("VB_SWAP") == "1"
PEOPLE = [
    ("Priyanka", "Desai", "" if os.environ.get("VB_NO_TITLE") == "1" else "Director of Partnerships", "Lumen Health",
     "priyanka.desai@lumenhealth.example", [("work", "(415) 555-0100"), ("mobile", "(415) 555-0132")],
     ("500 Mission St, Suite 1200", "San Francisco", "CA", "94105")),
    ("Tomas", "Reyes", "Senior Data Engineer", "Northgate Analytics", "treyes@northgate.example",
     [("mobile" if swap else "work", "312-555-0177"), ("work" if swap else "mobile", "312-555-0119")], None),
    ("Grace", "Kim", "VP Operations", "Cedar & Pine Clinics", "grace.kim@cedarpine.example",
     [("work", "617.555.0164"), ("mobile", "617.555.0108")],
     None if os.environ.get("VB_NO_ADDR") == "1" else ("88 Harrison Ave", "Boston", "MA", "02111")),
]

G.start()
G.open_terminal()
G.authorize("Contacts", data_prompt=True)
for fn, ln, title, org, email, phones, addr in PEOPLE:
    lines = [
        'tell application "Contacts"',
        f'set p to make new person with properties {{first name:"{fn}", last name:"{ln}", organization:"{org}"}}',
    ]
    if title:
        lines.append(f'set job title of p to "{title}"')
    lines.append(f'make new email at end of emails of p with properties {{label:"work", value:"{email}"}}')
    for label, num in phones:
        lines.append(f'make new phone at end of phones of p with properties {{label:"{label}", value:"{num}"}}')
    if addr:
        st, city, state, z = addr
        lines.append(
            f'make new address at end of addresses of p with properties {{label:"work", street:"{st}", city:"{city}", state:"{state}", zip:"{z}"}}'
        )
    lines += ["save", "end tell"]
    G.run("osascript " + " ".join(f"-e '{x}'" for x in lines), wait=3)
G.shot("final")
print("gold done")
