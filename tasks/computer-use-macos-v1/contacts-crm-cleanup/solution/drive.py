"""contacts-crm-cleanup gold: Terminal asks Contacts (AppleScript) to fold
each same-email pair into one card (the second card's phone added to the
first, the second deleted), apply the six job changes, and build the three
company groups from where people work after the changes.

Cheat variants: VB_NAMEMERGE=1 also deletes the Cobalt Sam Lee as a
"duplicate" of the Northwind one; VB_STALEGROUPS=1 builds the groups from
the companies before the changes; VB_NOMERGE=1 leaves both pairs;
VB_TITLEONLY=1 changes titles but not companies.
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import goldlib as G  # noqa: E402

NW, BW, CB = "Northwind Traders", "Brightwave Printing", "Cobalt Cloud Hosting"
CHANGES = [
    ("Sarah", "Kim", NW, "Head of Procurement"),
    ("Tom", "Becker", CB, "VP Engineering"),
    ("Ana", "Silva", "Quarry Stone Studio", "Studio Manager"),
    ("Carl", "Jensen", BW, "Event Coordinator"),
    ("Jamal", "Reed", CB, "Data Engineer"),
    ("Mei", "Chen", NW, "COO"),
]
NOW = {  # (first, last, organization) of each group's members after the changes
    "Northwind": [("Jon", "Park", NW), ("Sam", "Lee", NW), ("Rita", "Gomez", NW), ("Chris", "Hall", NW),
                  ("Owen", "Brooks", NW), ("Mei", "Chen", NW), ("Paul", "Novak", NW), ("Sarah", "Kim", NW)],
    "Brightwave": [("Lena", "Ortiz", BW), ("Victor", "Lang", BW), ("Nadia", "Rahman", BW), ("Chris", "Hale", BW),
                   ("Greg", "Dunn", BW), ("Ivy", "Walsh", BW), ("Carl", "Jensen", BW)],
    "Cobalt": [("Tom", "Becker", CB), ("Sam", "Lee", CB), ("Marcus", "Hale", CB), ("Liz", "Moreno", CB),
               ("Dev", "Patel", CB), ("Jamal", "Reed", CB)],
}  # fmt: skip
BEFORE = {
    "Northwind": [m for m in NOW["Northwind"] if m[0] != "Sarah"],
    "Brightwave": [m for m in NOW["Brightwave"] if m[0] != "Carl"] + [("Sarah", "Kim", NW)],
    "Cobalt": [m for m in NOW["Cobalt"] if m[0] != "Jamal"] + [("Ana", "Silva", "Quarry Stone Studio")],
}


def who(first: str, last: str, org: str | None = None) -> str:
    cond = f'first name is "{first}" and last name is "{last}"'
    if org:
        cond += f' and organization is "{org}"'
    return f"(first person whose {cond})"


def tell(lines: list[str]) -> None:
    G.run("osascript -e 'tell application \"Contacts\"' " + " ".join(f"-e '{x}'" for x in lines) + " -e 'save' -e 'end tell'", wait=4)


G.start()
G.open_terminal()
G.authorize("Contacts", data_prompt=True)
if os.environ.get("VB_NOMERGE") != "1":
    tell([
        f'make new phone at end of phones of {who("Jon", "Park")} with properties {{label:"mobile", value:"(617) 555-0199"}}',
        f'delete {who("Jonathan", "Park")}',
        f'make new phone at end of phones of {who("Liz", "Moreno")} with properties {{label:"mobile", value:"(628) 555-0177"}}',
        f'delete {who("Elizabeth", "Moreno")}',
    ])
if os.environ.get("VB_NAMEMERGE") == "1":
    tell([f'delete {who("Sam", "Lee", CB)}'])
title_only = os.environ.get("VB_TITLEONLY") == "1"
lines = []
for f, l, org, title in CHANGES:
    lines.append(f'set job title of {who(f, l)} to "{title}"')
    if not title_only:
        lines.append(f'set organization of {who(f, l)} to "{org}"')
tell(lines)
groups = BEFORE if os.environ.get("VB_STALEGROUPS") == "1" else NOW
for g, members in groups.items():
    lines = [f'set g to make new group with properties {{name:"{g}"}}']
    for f, l, org in members:
        if title_only and (f, l) in {("Sarah", "Kim"), ("Carl", "Jensen"), ("Jamal", "Reed")}:
            org = None  # their organization was not changed
        lines.append(f"add {who(f, l, org if (f, l) == ('Sam', 'Lee') else None)} to g")
    tell(lines)
G.run("osascript -e 'tell application \"Contacts\" to get name of every group'", wait=3)
G.shot("final")
print("gold done")
