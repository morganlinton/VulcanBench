#!/usr/bin/env python3
"""contacts-crm-cleanup checker: the Contacts store copied from the stopped VM.

    check.py /var/lib/vbmac

Cards are identified by email address (unique in the seed except for the two
same-person pairs). PEOPLE is the seed as built (first, last, organization,
job title, email, phone numbers); CHANGES the six job changes in the note.

Sub-metrics (reward is 1 only when all are 1):
  artifact_present    the Contacts store was collected
  email_dupes_merged  one card each for jon.park@ and liz.moreno@, holding both
                      phone numbers of the pair
  namesakes_kept      both Sam Lee cards (two emails) and both Chris Hall and
                      Chris Hale are still separate cards
  job_changes         the six people have their new organization and title
  groups_built        groups Northwind, Brightwave and Cobalt hold exactly the
                      people whose organization is that company after the changes;
                      a Smart List of that name counts when its rule selects them
  others_untouched    38 people plus the owner's own Jordan Lee card (39 cards),
                      and everyone else keeps their organization and title
  ui_path             the MCP server logged pointer input
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, "/opt/vbdesk")
import cu_check as cu  # noqa: E402
import mac_state as M  # noqa: E402

METRICS = (
    "artifact_present",
    "email_dupes_merged",
    "namesakes_kept",
    "job_changes",
    "groups_built",
    "others_untouched",
    "ui_path",
)
PEOPLE = [
    ["Jon", "Park", "Northwind Traders", "Procurement Lead", "jon.park@northwind.example", ["(617) 555-0101"]],
    ["Jonathan", "Park", "Northwind Traders", "Procurement Lead", "jon.park@northwind.example", ["(617) 555-0199"]],
    ["Sam", "Lee", "Northwind Traders", "Logistics Analyst", "sam.lee@northwind.example", ["(617) 555-0102"]],
    ["Rita", "Gomez", "Northwind Traders", "CFO", "rita.gomez@northwind.example", []],
    ["Chris", "Hall", "Northwind Traders", "Account Executive", "chris.hall@northwind.example", ["(617) 555-0105"]],
    ["Owen", "Brooks", "Northwind Traders", "Warehouse Manager", "owen.brooks@northwind.example", []],
    ["Mei", "Chen", "Northwind Traders", "Operations Director", "mei.chen@northwind.example", ["(617) 555-0107"]],
    ["Paul", "Novak", "Northwind Traders", "IT Manager", "paul.novak@northwind.example", []],
    ["Sarah", "Kim", "Brightwave Printing", "Account Manager", "sarah.kim@brightwave.example", ["(617) 555-0148"]],
    ["Lena", "Ortiz", "Brightwave Printing", "Account Manager", "lena.ortiz@brightwave.example", ["(617) 555-0149"]],
    ["Victor", "Lang", "Brightwave Printing", "Production Lead", "victor.lang@brightwave.example", []],
    ["Nadia", "Rahman", "Brightwave Printing", "Designer", "nadia.rahman@brightwave.example", []],
    ["Chris", "Hale", "Brightwave Printing", "Sales Director", "chris.hale@brightwave.example", ["(617) 555-0153"]],
    ["Greg", "Dunn", "Brightwave Printing", "Press Operator", "greg.dunn@brightwave.example", []],
    ["Ivy", "Walsh", "Brightwave Printing", "Office Manager", "ivy.walsh@brightwave.example", ["(617) 555-0155"]],
    ["Tom", "Becker", "Cobalt Cloud Hosting", "Engineering Manager", "tom.becker@cobaltcloud.example", ["(628) 555-0170"]],
    ["Sam", "Lee", "Cobalt Cloud Hosting", "Support Engineer", "sam.lee@cobaltcloud.example", ["(628) 555-0172"]],
    ["Ana", "Silva", "Cobalt Cloud Hosting", "Solutions Architect", "ana.silva@cobaltcloud.example", ["(628) 555-0173"]],
    ["Marcus", "Hale", "Cobalt Cloud Hosting", "Support Lead", "mhale@cobaltcloud.example", ["(628) 555-0171"]],
    ["Liz", "Moreno", "Cobalt Cloud Hosting", "Billing Specialist", "liz.moreno@cobaltcloud.example", ["(628) 555-0176"]],
    ["Elizabeth", "Moreno", "Cobalt Cloud Hosting", "Billing", "liz.moreno@cobaltcloud.example", ["(628) 555-0177"]],
    ["Dev", "Patel", "Cobalt Cloud Hosting", "Site Reliability Engineer", "dev.patel@cobaltcloud.example", []],
    ["Aaron", "Wells", "Greenleaf Landscaping", "Owner", "aaron@greenleaf.example", ["(617) 555-0120"]],
    ["Bea", "Torres", "", "", "bea.torres@mail.example", ["(617) 555-0121"]],
    ["Carl", "Jensen", "Pinecrest Catering", "Chef", "carl@pinecrest.example", []],
    ["Dana", "Whitfield", "Harbor Analytics", "Operations Manager", "dana@harboranalytics.example", []],
    ["Elena", "Novak", "", "", "elena.novak@mail.example", ["(212) 555-0161"]],
    ["Felix", "Grant", "Lumen Electric", "Electrician", "felix@lumen.example", []],
    ["Gina", "Rossi", "Quarry Stone Studio", "Owner", "gina@quarrystone.example", ["(401) 555-0130"]],
    ["Hank", "Miller", "", "", "hank.miller@mail.example", ["(617) 555-0131"]],
    ["Isla", "Burke", "Atelier Lumiere", "Photographer", "isla@atelierlumiere.example", []],
    ["Jamal", "Reed", "Harbor Analytics", "Analyst", "jamal@harboranalytics.example", []],
    ["Kara", "Singh", "", "", "kara.singh@mail.example", []],
    ["Leo", "Fischer", "Greenleaf Landscaping", "Crew Lead", "leo@greenleaf.example", []],
    ["Maya", "Cohen", "Pinecrest Catering", "Manager", "maya@pinecrest.example", ["(617) 555-0135"]],
    ["Nora", "Quinn", "", "", "nora.quinn@mail.example", []],
    ["Oscar", "Diaz", "Lumen Electric", "Office Manager", "oscar@lumen.example", []],
    ["Priya", "Raman", "Harbor Analytics", "Product Lead", "priya@harboranalytics.example", ["(617) 555-0138"]],
    ["Quinn", "Adler", "", "", "quinn.adler@mail.example", []],
    ["Rosa", "Delgado", "Harborline Supply Co.", "Accounts Receivable", "rosa@harborline.example", ["(207) 555-0134"]],
]  # fmt: skip
CHANGES = {
    "sarah.kim@brightwave.example": ("Northwind Traders", "Head of Procurement"),
    "tom.becker@cobaltcloud.example": ("Cobalt Cloud Hosting", "VP Engineering"),
    "ana.silva@cobaltcloud.example": ("Quarry Stone Studio", "Studio Manager"),
    "carl@pinecrest.example": ("Brightwave Printing", "Event Coordinator"),
    "jamal@harboranalytics.example": ("Cobalt Cloud Hosting", "Data Engineer"),
    "mei.chen@northwind.example": ("Northwind Traders", "COO"),
}  # fmt: skip
GROUPS = {"Northwind": "Northwind Traders", "Brightwave": "Brightwave Printing", "Cobalt": "Cobalt Cloud Hosting"}
DUPES = ("jon.park@northwind.example", "liz.moreno@cobaltcloud.example")


def digits(n: str) -> str:
    return "".join(c for c in n if c.isdigit())[-10:]


def main(argv: list[str]) -> int:
    root = Path(argv[1])
    export = root / "export"
    fs = export / "fs"
    m = {k: 0 for k in METRICS}
    extra: dict[str, object] = {"shutdown": M.manifest(export).get("shutdown", "none")}
    if not (fs / M.CONTACTS_DB).is_file():
        cu.emit(m, METRICS, **extra)
        return 0
    m["artifact_present"] = 1
    cards = M.contacts(fs)
    by_email: dict[str, list[M.Contact]] = {}
    for c in cards:
        for e in c.email_keys:
            by_email.setdefault(e, []).append(c)
    extra["cards"] = len(cards)
    ok = True
    for e in DUPES:
        hits = by_email.get(e, [])
        want = {digits(n) for p in PEOPLE if p[4] == e for n in p[5]}
        have = {digits(n) for _, n in hits[0].phones} if len(hits) == 1 else set()
        ok = ok and len(hits) == 1 and want <= have
    m["email_dupes_merged"] = int(ok)
    sams = [c for c in cards if c.first == "Sam" and c.last == "Lee"]
    m["namesakes_kept"] = int(
        len(sams) == 2
        and {e for c in sams for e in c.email_keys} == {"sam.lee@northwind.example", "sam.lee@cobaltcloud.example"}
        and len([c for c in cards if c.name == "Chris Hall"]) == 1
        and len([c for c in cards if c.name == "Chris Hale"]) == 1
    )
    final = {p[4]: CHANGES.get(p[4], (p[2], p[3])) for p in PEOPLE}

    def card(e: str) -> M.Contact | None:
        hits = by_email.get(e, [])
        return hits[0] if len(hits) == 1 else None

    got = {e: card(e) for e in final}
    extra["changed"] = {e: (c.org, c.title) if c else None for e, c in got.items() if e in CHANGES}
    m["job_changes"] = int(all(got[e] is not None and (got[e].org, got[e].title) == want for e, want in CHANGES.items()))
    gok = True
    extra["groups"] = {}
    smart = M.smart_groups(fs)
    for g, org in GROUPS.items():
        want = sorted(e for e, (o, _) in final.items() if o == org)
        members = sorted(e for c in cards if g in c.groups for e in c.email_keys)
        if members != want and g in smart:  # a Smart List meets "everyone who works there now"
            members = sorted(e for c in cards if smart[g].matches(c) for e in c.email_keys)
            extra.setdefault("smart_groups", {})[g] = f"{smart[g].field} {smart[g].comparison} {smart[g].value!r}"
        extra["groups"][g] = members
        gok = gok and members == want
    m["groups_built"] = int(gok)
    owner = [c for c in cards if c.name == "Jordan Lee" and not c.email_keys]  # base-config.sh persona
    oth = len(cards) == 39 and len(owner) == 1
    for p in PEOPLE:
        e = p[4]
        if e in CHANGES or e in DUPES:
            continue
        c = got.get(e)
        oth = oth and c is not None and (c.org, c.title) == (p[2], p[3]) and (c.first, c.last) == (p[0], p[1])
    m["others_untouched"] = int(oth)
    events_log = M.input_events(root / "input.log")
    m["ui_path"] = int(bool({e.get("kind") for e in events_log} & {"click", "drag"}))
    extra["input_events"] = len(events_log)
    cu.emit(m, METRICS, **extra)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
