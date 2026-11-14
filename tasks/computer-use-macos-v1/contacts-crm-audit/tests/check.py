#!/usr/bin/env python3
"""contacts-crm-audit checker: the Contacts store copied from the stopped VM.

    check.py /var/lib/vbmac

A card is known by its email addresses (compared in lowercase). FINAL is
each of the 68 people after the merges and job changes: first, last,
organization, job title, all email addresses, all phone numbers. A merged
card is found by its shared address, whatever name it kept.

Sub-metrics (reward is 1 only when all are 1):
  artifact_present    the Contacts store was collected
  email_dupes_merged  one card each for jon.park@, liz.moreno@, rob.tran@ (the
                      two cards differ only in case) and katherine.oneill@ (Kate
                      O'Neill's second address), holding every phone number and
                      address of the pair
  namesakes_kept      two Sam Lee, two Maria Lopez and two Dan Kim cards (each
                      with its own address), and Chris Hall and Chris Hale apart
  job_changes         the ten people have their new organization and title
  correction_heeded   Dev Patel is still Site Reliability Engineer at Cobalt
  right_namesake      the Northwind Sam Lee and the Brightwave Dan Kim are unchanged
  groups_built        groups (or Smart Lists) Northwind, Brightwave, Cobalt and
                      Lumen hold exactly the people at that company now
  others_untouched    68 people plus the owner's own Jordan Lee card (69 cards),
                      and everyone else keeps their name, organization and title
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
    "correction_heeded",
    "right_namesake",
    "groups_built",
    "others_untouched",
    "ui_path",
)
FINAL = {
    "aaron@greenleaf.example": ["Aaron", "Wells", "Greenleaf Landscaping", "Owner", ["aaron@greenleaf.example"], ["(617) 555-0120"]],
    "ana.silva@cobaltcloud.example": ["Ana", "Silva", "Quarry Stone Studio", "Studio Manager", ["ana.silva@cobaltcloud.example"], ["(628) 555-0173"]],
    "bea.torres@mail.example": ["Bea", "Torres", "", "", ["bea.torres@mail.example"], ["(617) 555-0121"]],
    "ben@ortizplumbing.example": ["Ben", "Ortiz", "Ortiz Plumbing", "Owner", ["ben@ortizplumbing.example"], ["(617) 555-0122"]],
    "carl@pinecrest.example": ["Carl", "Jensen", "Brightwave Printing", "Event Coordinator", ["carl@pinecrest.example"], []],
    "chris.hale@brightwave.example": ["Chris", "Hale", "Brightwave Printing", "Sales Director", ["chris.hale@brightwave.example"], ["(617) 555-0153"]],
    "chris.hall@northwind.example": ["Chris", "Hall", "Northwind Traders", "Account Executive", ["chris.hall@northwind.example"], ["(617) 555-0105"]],
    "cora.hayes@mail.example": ["Cora", "Hayes", "", "", ["cora.hayes@mail.example"], []],
    "dan.kim@brightwave.example": ["Dan", "Kim", "Brightwave Printing", "Press Operator", ["dan.kim@brightwave.example"], ["(617) 555-0158"]],
    "dan.kim@lumen.example": ["Dan", "Kim", "Brightwave Printing", "Estimator", ["dan.kim@lumen.example"], ["(401) 555-0142"]],
    "dana@harboranalytics.example": ["Dana", "Whitfield", "Harbor Analytics", "Operations Manager", ["dana@harboranalytics.example"], []],
    "dev.patel@cobaltcloud.example": ["Dev", "Patel", "Cobalt Cloud Hosting", "Site Reliability Engineer", ["dev.patel@cobaltcloud.example"], []],
    "drew@harboranalytics.example": ["Drew", "Mills", "Harbor Analytics", "Designer", ["drew@harboranalytics.example"], []],
    "elena.novak@mail.example": ["Elena", "Novak", "", "", ["elena.novak@mail.example"], ["(212) 555-0161"]],
    "eve@atelierlumiere.example": ["Eve", "Santos", "Atelier Lumiere", "Studio Assistant", ["eve@atelierlumiere.example"], []],
    "felix@lumen.example": ["Felix", "Grant", "Lumen Electric", "Electrician", ["felix@lumen.example"], []],
    "finn.doyle@mail.example": ["Finn", "Doyle", "", "", ["finn.doyle@mail.example"], ["(617) 555-0123"]],
    "gina@quarrystone.example": ["Gina", "Rossi", "Quarry Stone Studio", "Owner", ["gina@quarrystone.example"], ["(401) 555-0130"]],
    "greg.dunn@brightwave.example": ["Greg", "Dunn", "Brightwave Printing", "Press Operator", ["greg.dunn@brightwave.example"], []],
    "gwen@baysidedental.example": ["Gwen", "Price", "Bayside Dental", "Dentist", ["gwen@baysidedental.example"], ["(617) 555-0124"]],
    "hank.miller@mail.example": ["Hank", "Miller", "", "", ["hank.miller@mail.example"], ["(617) 555-0131"]],
    "hugo.weiss@cobaltcloud.example": ["Hugo", "Weiss", "Cobalt Cloud Hosting", "Network Engineer", ["hugo.weiss@cobaltcloud.example"], []],
    "ian.shaw@mail.example": ["Ian", "Shaw", "", "", ["ian.shaw@mail.example"], []],
    "isla@atelierlumiere.example": ["Isla", "Burke", "Atelier Lumiere", "Photographer", ["isla@atelierlumiere.example"], []],
    "ivy.walsh@brightwave.example": ["Ivy", "Walsh", "Brightwave Printing", "Office Manager", ["ivy.walsh@brightwave.example"], ["(617) 555-0155"]],
    "jade@harboranalytics.example": ["Jade", "Wu", "Harbor Analytics", "Data Scientist", ["jade@harboranalytics.example"], []],
    "jamal@harboranalytics.example": ["Jamal", "Reed", "Cobalt Cloud Hosting", "Data Engineer", ["jamal@harboranalytics.example"], []],
    "jon.park@northwind.example": ["Jon", "Park", "Northwind Traders", "Procurement Lead", ["jon.park@northwind.example"], ["(617) 555-0101", "(617) 555-0199"]],
    "kara.singh@mail.example": ["Kara", "Singh", "", "", ["kara.singh@mail.example"], []],
    "katherine.oneill@mail.example": ["Kate", "O'Neill", "Brightwave Printing", "Head of Client Services", ["kate.oneill@brightwave.example", "katherine.oneill@mail.example"], ["(617) 555-0156", "(617) 555-0190"]],
    "ken.ito@northwind.example": ["Ken", "Ito", "Northwind Traders", "Logistics Coordinator", ["ken.ito@northwind.example"], []],
    "kyle@greenleaf.example": ["Kyle", "Ross", "Greenleaf Landscaping", "Crew Member", ["kyle@greenleaf.example"], []],
    "lara.byrne@mail.example": ["Lara", "Byrne", "", "", ["lara.byrne@mail.example"], []],
    "lena.ortiz@brightwave.example": ["Lena", "Ortiz", "Brightwave Printing", "Account Manager", ["lena.ortiz@brightwave.example"], ["(617) 555-0149"]],
    "leo@greenleaf.example": ["Leo", "Fischer", "Greenleaf Landscaping", "Crew Lead", ["leo@greenleaf.example"], []],
    "lily.chang@cobaltcloud.example": ["Lily", "Chang", "Cobalt Cloud Hosting", "Recruiter", ["lily.chang@cobaltcloud.example"], []],
    "liz.moreno@cobaltcloud.example": ["Liz", "Moreno", "Cobalt Cloud Hosting", "Billing Manager", ["liz.moreno@cobaltcloud.example"], ["(628) 555-0176", "(628) 555-0177"]],
    "maria.lopez@cobaltcloud.example": ["Maria", "Lopez", "Cobalt Cloud Hosting", "Product Manager", ["maria.lopez@cobaltcloud.example"], ["(628) 555-0179"]],
    "maria.lopez@northwind.example": ["Maria", "Lopez", "Northwind Traders", "Accountant", ["maria.lopez@northwind.example"], ["(617) 555-0109"]],
    "maya@pinecrest.example": ["Maya", "Cohen", "Pinecrest Catering", "Manager", ["maya@pinecrest.example"], ["(617) 555-0135"]],
    "mei.chen@northwind.example": ["Mei", "Chen", "Northwind Traders", "COO", ["mei.chen@northwind.example"], ["(617) 555-0107"]],
    "mhale@cobaltcloud.example": ["Marcus", "Hale", "Cobalt Cloud Hosting", "Support Lead", ["mhale@cobaltcloud.example"], ["(628) 555-0171"]],
    "milo@quarrystone.example": ["Milo", "Hart", "Quarry Stone Studio", "Mason", ["milo@quarrystone.example"], []],
    "nadia.rahman@brightwave.example": ["Nadia", "Rahman", "Brightwave Printing", "Designer", ["nadia.rahman@brightwave.example"], []],
    "nina@baysidedental.example": ["Nina", "Volk", "Bayside Dental", "Hygienist", ["nina@baysidedental.example"], []],
    "nora.quinn@mail.example": ["Nora", "Quinn", "", "", ["nora.quinn@mail.example"], []],
    "omar.aziz@mail.example": ["Omar", "Aziz", "", "", ["omar.aziz@mail.example"], ["(212) 555-0162"]],
    "oscar@lumen.example": ["Oscar", "Diaz", "Lumen Electric", "Office Manager", ["oscar@lumen.example"], []],
    "owen.brooks@northwind.example": ["Owen", "Brooks", "Northwind Traders", "Warehouse Manager", ["owen.brooks@northwind.example"], []],
    "paul.novak@northwind.example": ["Paul", "Novak", "Northwind Traders", "IT Manager", ["paul.novak@northwind.example"], []],
    "pia@harborline.example": ["Pia", "Lund", "Harborline Supply Co.", "Buyer", ["pia@harborline.example"], []],
    "priya@harboranalytics.example": ["Priya", "Raman", "Harbor Analytics", "Product Lead", ["priya@harboranalytics.example"], ["(617) 555-0138"]],
    "quinn.adler@mail.example": ["Quinn", "Adler", "", "", ["quinn.adler@mail.example"], []],
    "reed.cole@mail.example": ["Reed", "Cole", "", "", ["reed.cole@mail.example"], []],
    "rita.gomez@northwind.example": ["Rita", "Gomez", "Northwind Traders", "CFO", ["rita.gomez@northwind.example"], []],
    "rob.tran@lumen.example": ["Rob", "Tran", "Lumen Electric", "Project Engineer", ["rob.tran@lumen.example"], ["(401) 555-0140", "(401) 555-0191"]],
    "rosa@harborline.example": ["Rosa", "Delgado", "Harborline Supply Co.", "Accounts Receivable", ["rosa@harborline.example"], ["(207) 555-0134"]],
    "ruth.adams@brightwave.example": ["Ruth", "Adams", "Brightwave Printing", "Finance Manager", ["ruth.adams@brightwave.example"], []],
    "sam.lee@cobaltcloud.example": ["Sam", "Lee", "Cobalt Cloud Hosting", "Support Lead", ["sam.lee@cobaltcloud.example"], ["(628) 555-0172"]],
    "sam.lee@northwind.example": ["Sam", "Lee", "Northwind Traders", "Logistics Analyst", ["sam.lee@northwind.example"], ["(617) 555-0102"]],
    "sarah.kim@brightwave.example": ["Sarah", "Kim", "Northwind Traders", "Head of Procurement", ["sarah.kim@brightwave.example"], ["(617) 555-0148"]],
    "sofia@pinecrest.example": ["Sofia", "Marin", "Pinecrest Catering", "Server", ["sofia@pinecrest.example"], []],
    "tess.moore@northwind.example": ["Tess", "Moore", "Northwind Traders", "Buyer", ["tess.moore@northwind.example"], ["(617) 555-0108"]],
    "theo@ortizplumbing.example": ["Theo", "Banks", "Ortiz Plumbing", "Apprentice", ["theo@ortizplumbing.example"], []],
    "tom.becker@cobaltcloud.example": ["Tom", "Becker", "Cobalt Cloud Hosting", "VP Engineering", ["tom.becker@cobaltcloud.example"], ["(628) 555-0170"]],
    "victor.lang@brightwave.example": ["Victor", "Lang", "Brightwave Printing", "Production Lead", ["victor.lang@brightwave.example"], []],
    "wes@lumen.example": ["Wes", "Holt", "Lumen Electric", "Apprentice", ["wes@lumen.example"], []],
    "zoe@lumen.example": ["Zoe", "Park", "Lumen Electric", "Office Admin", ["zoe@lumen.example"], []],
}  # fmt: skip
DUPES = ["jon.park@northwind.example", "katherine.oneill@mail.example", "liz.moreno@cobaltcloud.example", "rob.tran@lumen.example"]
CHANGED = ["ana.silva@cobaltcloud.example", "carl@pinecrest.example", "dan.kim@lumen.example", "jamal@harboranalytics.example", "katherine.oneill@mail.example", "liz.moreno@cobaltcloud.example", "mei.chen@northwind.example", "sam.lee@cobaltcloud.example", "sarah.kim@brightwave.example", "tom.becker@cobaltcloud.example"]
GROUPS = {"Northwind": "Northwind Traders", "Brightwave": "Brightwave Printing", "Cobalt": "Cobalt Cloud Hosting", "Lumen": "Lumen Electric"}
NAMESAKES = {
    ("Sam", "Lee"): {"sam.lee@northwind.example", "sam.lee@cobaltcloud.example"},
    ("Maria", "Lopez"): {"maria.lopez@northwind.example", "maria.lopez@cobaltcloud.example"},
    ("Dan", "Kim"): {"dan.kim@brightwave.example", "dan.kim@lumen.example"},
}
UNCHANGED = {
    "dev.patel@cobaltcloud.example": ("Cobalt Cloud Hosting", "Site Reliability Engineer"),
    "sam.lee@northwind.example": ("Northwind Traders", "Logistics Analyst"),
    "dan.kim@brightwave.example": ("Brightwave Printing", "Press Operator"),
}


def digits(n: str) -> str:
    return "".join(c for c in n if c.isdigit())[-10:]


def key(c: M.Contact) -> str | None:
    """The FINAL key a card belongs to: a shared (dupe) address first."""
    keys = c.email_keys
    for k in DUPES:
        if k in keys:
            return k
    hits = [k for k in keys if k in FINAL]
    return hits[0] if len(hits) == 1 else None


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
    extra["cards"] = len(cards)
    by_key: dict[str, list[M.Contact]] = {}
    for c in cards:
        k = key(c)
        if k:
            by_key.setdefault(k, []).append(c)

    def card(k: str) -> M.Contact | None:
        hits = by_key.get(k, [])
        return hits[0] if len(hits) == 1 else None

    ok = True
    for k in DUPES:
        c = card(k)
        want_ph = {digits(n) for n in FINAL[k][5]}
        want_em = set(FINAL[k][4])
        ok = ok and c is not None and want_ph <= {digits(n) for _, n in c.phones} and want_em <= c.email_keys
    m["email_dupes_merged"] = int(ok)
    m["namesakes_kept"] = int(
        all(
            {e for c in cards if (c.first, c.last) == name for e in c.email_keys} == emails
            and len([c for c in cards if (c.first, c.last) == name]) == 2
            for name, emails in NAMESAKES.items()
        )
        and len([c for c in cards if c.name == "Chris Hall"]) == 1
        and len([c for c in cards if c.name == "Chris Hale"]) == 1
    )

    def job(k: str) -> tuple[str, str] | None:
        c = card(k)
        return (c.org, c.title) if c else None

    extra["changed"] = {k: job(k) for k in CHANGED}
    m["job_changes"] = int(all(job(k) == (FINAL[k][2], FINAL[k][3]) for k in CHANGED))
    m["correction_heeded"] = int(job("dev.patel@cobaltcloud.example") == UNCHANGED["dev.patel@cobaltcloud.example"])
    m["right_namesake"] = int(
        all(job(k) == UNCHANGED[k] for k in ("sam.lee@northwind.example", "dan.kim@brightwave.example"))
    )
    smart = M.smart_groups(fs)
    gok = True
    extra["groups"] = {}
    for g, org in GROUPS.items():
        want = sorted(k for k, v in FINAL.items() if v[2] == org)
        members = sorted({key(c) or "?" for c in cards if g in c.groups})
        if members != want and g in smart:  # a Smart List meets "everyone who works there now"
            members = sorted({key(c) or "?" for c in cards if smart[g].matches(c)})
            extra.setdefault("smart_groups", {})[g] = f"{smart[g].field} {smart[g].comparison} {smart[g].value!r}"
        extra["groups"][g] = members
        gok = gok and members == want
    m["groups_built"] = int(gok)
    owner = [c for c in cards if c.name == "Jordan Lee" and not c.email_keys]  # base-config.sh persona
    oth = len(cards) == 69 and len(owner) == 1
    for k, (first, last, org, title, _, _) in FINAL.items():
        if k in DUPES or k in CHANGED:
            continue
        c = card(k)
        oth = oth and c is not None and (c.first, c.last, c.org, c.title) == (first, last, org, title)
    m["others_untouched"] = int(oth)
    events_log = M.input_events(root / "input.log")
    m["ui_path"] = int(bool({e.get("kind") for e in events_log} & {"click", "drag"}))
    extra["input_events"] = len(events_log)
    cu.emit(m, METRICS, **extra)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
