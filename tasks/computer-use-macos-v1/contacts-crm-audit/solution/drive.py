"""contacts-crm-audit gold: Terminal asks Contacts (AppleScript) to fold each
same-person pair into one card (the second card's phone added to the first,
the second deleted), build the four company groups from where people work
after the changes, then apply the ten job changes (Dev Patel's is withdrawn).
Cards are picked by their seeded first name, last name and company, which is
unique in the seed, so namesakes stay apart.

Cheat variants: VB_NAMEMERGE=1 also deletes the Cobalt Maria Lopez as a
"duplicate"; VB_CORRECTION=ignore applies Dev Patel's withdrawn move;
VB_WRONGSAM=1 gives the Northwind Sam Lee the Cobalt one's promotion;
VB_CASEDUP=1 leaves Rob and Robert Tran apart; VB_STALEGROUPS=1 builds the
groups from the companies before the changes.
"""

import os
import shlex
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import goldlib as G  # noqa: E402

WHO = {"jon.park@northwind.example": ["Jon", "Park", "Northwind Traders"], "sam.lee@northwind.example": ["Sam", "Lee", "Northwind Traders"], "rita.gomez@northwind.example": ["Rita", "Gomez", "Northwind Traders"], "chris.hall@northwind.example": ["Chris", "Hall", "Northwind Traders"], "owen.brooks@northwind.example": ["Owen", "Brooks", "Northwind Traders"], "mei.chen@northwind.example": ["Mei", "Chen", "Northwind Traders"], "paul.novak@northwind.example": ["Paul", "Novak", "Northwind Traders"], "tess.moore@northwind.example": ["Tess", "Moore", "Northwind Traders"], "ken.ito@northwind.example": ["Ken", "Ito", "Northwind Traders"], "maria.lopez@northwind.example": ["Maria", "Lopez", "Northwind Traders"], "sarah.kim@brightwave.example": ["Sarah", "Kim", "Brightwave Printing"], "lena.ortiz@brightwave.example": ["Lena", "Ortiz", "Brightwave Printing"], "victor.lang@brightwave.example": ["Victor", "Lang", "Brightwave Printing"], "nadia.rahman@brightwave.example": ["Nadia", "Rahman", "Brightwave Printing"], "chris.hale@brightwave.example": ["Chris", "Hale", "Brightwave Printing"], "greg.dunn@brightwave.example": ["Greg", "Dunn", "Brightwave Printing"], "ivy.walsh@brightwave.example": ["Ivy", "Walsh", "Brightwave Printing"], "katherine.oneill@mail.example": ["Kate", "O'Neill", "Brightwave Printing"], "ruth.adams@brightwave.example": ["Ruth", "Adams", "Brightwave Printing"], "dan.kim@brightwave.example": ["Dan", "Kim", "Brightwave Printing"], "tom.becker@cobaltcloud.example": ["Tom", "Becker", "Cobalt Cloud Hosting"], "sam.lee@cobaltcloud.example": ["Sam", "Lee", "Cobalt Cloud Hosting"], "ana.silva@cobaltcloud.example": ["Ana", "Silva", "Cobalt Cloud Hosting"], "mhale@cobaltcloud.example": ["Marcus", "Hale", "Cobalt Cloud Hosting"], "liz.moreno@cobaltcloud.example": ["Liz", "Moreno", "Cobalt Cloud Hosting"], "dev.patel@cobaltcloud.example": ["Dev", "Patel", "Cobalt Cloud Hosting"], "hugo.weiss@cobaltcloud.example": ["Hugo", "Weiss", "Cobalt Cloud Hosting"], "maria.lopez@cobaltcloud.example": ["Maria", "Lopez", "Cobalt Cloud Hosting"], "lily.chang@cobaltcloud.example": ["Lily", "Chang", "Cobalt Cloud Hosting"], "felix@lumen.example": ["Felix", "Grant", "Lumen Electric"], "oscar@lumen.example": ["Oscar", "Diaz", "Lumen Electric"], "rob.tran@lumen.example": ["Rob", "Tran", "Lumen Electric"], "wes@lumen.example": ["Wes", "Holt", "Lumen Electric"], "dan.kim@lumen.example": ["Dan", "Kim", "Lumen Electric"], "zoe@lumen.example": ["Zoe", "Park", "Lumen Electric"], "aaron@greenleaf.example": ["Aaron", "Wells", "Greenleaf Landscaping"], "bea.torres@mail.example": ["Bea", "Torres", ""], "carl@pinecrest.example": ["Carl", "Jensen", "Pinecrest Catering"], "dana@harboranalytics.example": ["Dana", "Whitfield", "Harbor Analytics"], "elena.novak@mail.example": ["Elena", "Novak", ""], "gina@quarrystone.example": ["Gina", "Rossi", "Quarry Stone Studio"], "hank.miller@mail.example": ["Hank", "Miller", ""], "isla@atelierlumiere.example": ["Isla", "Burke", "Atelier Lumiere"], "jamal@harboranalytics.example": ["Jamal", "Reed", "Harbor Analytics"], "kara.singh@mail.example": ["Kara", "Singh", ""], "leo@greenleaf.example": ["Leo", "Fischer", "Greenleaf Landscaping"], "maya@pinecrest.example": ["Maya", "Cohen", "Pinecrest Catering"], "nora.quinn@mail.example": ["Nora", "Quinn", ""], "priya@harboranalytics.example": ["Priya", "Raman", "Harbor Analytics"], "quinn.adler@mail.example": ["Quinn", "Adler", ""], "rosa@harborline.example": ["Rosa", "Delgado", "Harborline Supply Co."], "ben@ortizplumbing.example": ["Ben", "Ortiz", "Ortiz Plumbing"], "cora.hayes@mail.example": ["Cora", "Hayes", ""], "drew@harboranalytics.example": ["Drew", "Mills", "Harbor Analytics"], "eve@atelierlumiere.example": ["Eve", "Santos", "Atelier Lumiere"], "finn.doyle@mail.example": ["Finn", "Doyle", ""], "gwen@baysidedental.example": ["Gwen", "Price", "Bayside Dental"], "ian.shaw@mail.example": ["Ian", "Shaw", ""], "jade@harboranalytics.example": ["Jade", "Wu", "Harbor Analytics"], "kyle@greenleaf.example": ["Kyle", "Ross", "Greenleaf Landscaping"], "lara.byrne@mail.example": ["Lara", "Byrne", ""], "milo@quarrystone.example": ["Milo", "Hart", "Quarry Stone Studio"], "nina@baysidedental.example": ["Nina", "Volk", "Bayside Dental"], "omar.aziz@mail.example": ["Omar", "Aziz", ""], "pia@harborline.example": ["Pia", "Lund", "Harborline Supply Co."], "reed.cole@mail.example": ["Reed", "Cole", ""], "sofia@pinecrest.example": ["Sofia", "Marin", "Pinecrest Catering"], "theo@ortizplumbing.example": ["Theo", "Banks", "Ortiz Plumbing"]}  # key email -> seeded (first, last, organization)
NOW = {"Northwind": ["chris.hall@northwind.example", "jon.park@northwind.example", "ken.ito@northwind.example", "maria.lopez@northwind.example", "mei.chen@northwind.example", "owen.brooks@northwind.example", "paul.novak@northwind.example", "rita.gomez@northwind.example", "sam.lee@northwind.example", "sarah.kim@brightwave.example", "tess.moore@northwind.example"], "Brightwave": ["carl@pinecrest.example", "chris.hale@brightwave.example", "dan.kim@brightwave.example", "dan.kim@lumen.example", "greg.dunn@brightwave.example", "ivy.walsh@brightwave.example", "katherine.oneill@mail.example", "lena.ortiz@brightwave.example", "nadia.rahman@brightwave.example", "ruth.adams@brightwave.example", "victor.lang@brightwave.example"], "Cobalt": ["dev.patel@cobaltcloud.example", "hugo.weiss@cobaltcloud.example", "jamal@harboranalytics.example", "lily.chang@cobaltcloud.example", "liz.moreno@cobaltcloud.example", "maria.lopez@cobaltcloud.example", "mhale@cobaltcloud.example", "sam.lee@cobaltcloud.example", "tom.becker@cobaltcloud.example"], "Lumen": ["felix@lumen.example", "oscar@lumen.example", "rob.tran@lumen.example", "wes@lumen.example", "zoe@lumen.example"]}
BEFORE = {"Northwind": ["chris.hall@northwind.example", "jon.park@northwind.example", "ken.ito@northwind.example", "maria.lopez@northwind.example", "mei.chen@northwind.example", "owen.brooks@northwind.example", "paul.novak@northwind.example", "rita.gomez@northwind.example", "sam.lee@northwind.example", "tess.moore@northwind.example"], "Brightwave": ["chris.hale@brightwave.example", "dan.kim@brightwave.example", "greg.dunn@brightwave.example", "ivy.walsh@brightwave.example", "katherine.oneill@mail.example", "lena.ortiz@brightwave.example", "nadia.rahman@brightwave.example", "ruth.adams@brightwave.example", "sarah.kim@brightwave.example", "victor.lang@brightwave.example"], "Cobalt": ["ana.silva@cobaltcloud.example", "dev.patel@cobaltcloud.example", "hugo.weiss@cobaltcloud.example", "lily.chang@cobaltcloud.example", "liz.moreno@cobaltcloud.example", "maria.lopez@cobaltcloud.example", "mhale@cobaltcloud.example", "sam.lee@cobaltcloud.example", "tom.becker@cobaltcloud.example"], "Lumen": ["dan.kim@lumen.example", "felix@lumen.example", "oscar@lumen.example", "rob.tran@lumen.example", "wes@lumen.example", "zoe@lumen.example"]}
CHANGES = {"sarah.kim@brightwave.example": ["Northwind Traders", "Head of Procurement"], "tom.becker@cobaltcloud.example": ["Cobalt Cloud Hosting", "VP Engineering"], "ana.silva@cobaltcloud.example": ["Quarry Stone Studio", "Studio Manager"], "carl@pinecrest.example": ["Brightwave Printing", "Event Coordinator"], "jamal@harboranalytics.example": ["Cobalt Cloud Hosting", "Data Engineer"], "mei.chen@northwind.example": ["Northwind Traders", "COO"], "sam.lee@cobaltcloud.example": ["Cobalt Cloud Hosting", "Support Lead"], "liz.moreno@cobaltcloud.example": ["Cobalt Cloud Hosting", "Billing Manager"], "dan.kim@lumen.example": ["Brightwave Printing", "Estimator"], "katherine.oneill@mail.example": ["Brightwave Printing", "Head of Client Services"]}
MERGES = [  # (kept card, deleted card, phone moved over)
    (("Jon", "Park", "Northwind Traders"), ("Jonathan", "Park", "Northwind Traders"), ("mobile", "(617) 555-0199")),
    (("Liz", "Moreno", "Cobalt Cloud Hosting"), ("Elizabeth", "Moreno", "Cobalt Cloud Hosting"), ("mobile", "(628) 555-0177")),
    (("Rob", "Tran", "Lumen Electric"), ("Robert", "Tran", "Lumen Electric"), ("mobile", "(401) 555-0191")),
    (("Kate", "O'Neill", "Brightwave Printing"), ("Katherine", "O'Neill", ""), ("mobile", "(617) 555-0190")),
]


def who(first: str, last: str, org: str) -> str:
    cond = f'first name is "{first}" and last name is "{last}"'
    cond += f' and organization is "{org}"' if org else " and organization is missing value"
    return f"(first person whose {cond})"


def tell(lines: list[str]) -> None:
    args = " ".join(f"-e {shlex.quote(x)}" for x in ['tell application "Contacts"', *lines, "save", "end tell"])
    G.run(f"osascript {args}", wait=4)


G.start()
G.open_terminal()
G.authorize("Contacts", data_prompt=True)
for keep, drop, (label, num) in MERGES:
    if os.environ.get("VB_CASEDUP") == "1" and keep[0] == "Rob":
        continue
    tell([f'make new phone at end of phones of {who(*keep)} with properties {{label:"{label}", value:"{num}"}}', f"delete {who(*drop)}"])
if os.environ.get("VB_NAMEMERGE") == "1":
    tell([f'delete {who("Maria", "Lopez", "Cobalt Cloud Hosting")}'])
groups = BEFORE if os.environ.get("VB_STALEGROUPS") == "1" else NOW
for g, members in groups.items():
    lines = [f'set g to make new group with properties {{name:"{g}"}}']
    for k in members:
        if os.environ.get("VB_NAMEMERGE") == "1" and k == "maria.lopez@cobaltcloud.example":
            continue
        lines.append(f"add {who(*WHO[k])} to g")
    tell(lines)
changes = dict(CHANGES)
if os.environ.get("VB_CORRECTION") == "ignore":
    changes["dev.patel@cobaltcloud.example"] = ["Lumen Electric", "Project Engineer"]
if os.environ.get("VB_WRONGSAM") == "1":
    changes["sam.lee@northwind.example"] = changes.pop("sam.lee@cobaltcloud.example")
lines = []
for k, (org, title) in changes.items():
    lines.append(f'set job title of {who(*WHO[k])} to "{title}"')
    lines.append(f'set organization of {who(*WHO[k])} to "{org}"')  # title first: still found by the seeded company
for k in range(0, len(lines), 6):
    tell(lines[k : k + 6])
G.run("osascript -e 'tell application \"Contacts\" to get name of every group'", wait=3)
G.shot("final")
print("gold done")
