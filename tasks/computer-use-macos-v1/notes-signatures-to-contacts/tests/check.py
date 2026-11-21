#!/usr/bin/env python3
"""notes-signatures-to-contacts checker: the Contacts store copied from the
stopped VM.

    check.py /var/lib/vbmac

Phone numbers are compared by their last ten digits; a work number may carry
the label work, main or a custom office/direct label, a mobile number
mobile, iPhone or a custom cell label. Text fields ignore case and spacing.
A street may be on one line or two ("500 Mission St" and "Suite 1200").

Sub-metrics (reward is 1 only when all are 1):
  artifact_present  the Contacts store was collected
  names_titles      one card each for Priyanka Desai, Tomas Reyes and Grace
                    Kim, with the job title and company from the signature
  emails            each card has its email address
  phones_labeled    each card has both numbers, the work number under a work
                    label and the mobile under a mobile label
  addresses         Priyanka's and Grace's cards have the office address
                    (street, city, state, ZIP)
  others_untouched  the existing cards are unchanged
  ui_path           the MCP server logged pointer input
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, "/opt/vbdesk")
import cu_check as cu  # noqa: E402
import mac_state as M  # noqa: E402

METRICS = (
    "artifact_present",
    "names_titles",
    "emails",
    "phones_labeled",
    "addresses",
    "others_untouched",
    "ui_path",
)
WORK = {"work", "main", "office", "direct", "work phone"}
MOBILE = {"mobile", "iphone", "cell", "cell phone", "mobile phone"}
PEOPLE = {
    "Priyanka Desai": {
        "title": "director of partnerships", "org": "lumen health",
        "email": "priyanka.desai@lumenhealth.example", "work": "4155550100", "mobile": "4155550132",
        "addr": (("500 mission st", "1200"), "san francisco", "ca", "94105"),
    },
    "Tomas Reyes": {
        "title": "senior data engineer", "org": "northgate analytics",
        "email": "treyes@northgate.example", "work": "3125550177", "mobile": "3125550119",
        "addr": None,
    },
    "Grace Kim": {
        "title": "vp operations", "org": "cedar & pine clinics",
        "email": "grace.kim@cedarpine.example", "work": "6175550164", "mobile": "6175550108",
        "addr": (("88 harrison ave",), "boston", "ma", "02111"),
    },
}


def n(s: str) -> str:
    return " ".join(s.lower().replace(".", "").replace(",", " ").split())


def phone_ok(c: M.Contact, number: str, labels: set[str]) -> bool:
    return any(M.phone_key(num) == number and lab in labels for lab, num in c.phones)


def addr_ok(c: M.Contact, want: tuple) -> bool:  # type: ignore[type-arg]
    parts, city, state, zipc = want
    for a in c.addresses:
        street = n(a["street"].replace("\n", " "))
        if all(n(p) in street for p in parts) and n(a["city"]) == city:
            if n(a["state"]) in (state, {"ca": "california", "ma": "massachusetts"}[state]) and a["zip"].strip() == zipc:
                return True
    return False


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
    found = {name: [c for c in cards if c.name == name] for name in PEOPLE}
    extra["found"] = {k: [(c.title, c.org, c.phones, c.emails, c.addresses) for c in v] for k, v in found.items()}
    one = {name: v[0] for name, v in found.items() if len(v) == 1}
    m["names_titles"] = int(
        len(one) == 3 and all(n(c.title) == PEOPLE[k]["title"] and n(c.org) == PEOPLE[k]["org"] for k, c in one.items())
    )
    m["emails"] = int(len(one) == 3 and all(PEOPLE[k]["email"] in c.email_keys for k, c in one.items()))
    m["phones_labeled"] = int(
        len(one) == 3
        and all(
            phone_ok(c, PEOPLE[k]["work"], WORK) and phone_ok(c, PEOPLE[k]["mobile"], MOBILE)
            for k, c in one.items()
        )
    )
    m["addresses"] = int(
        len(one) == 3 and all(addr_ok(one[k], PEOPLE[k]["addr"]) for k in PEOPLE if PEOPLE[k]["addr"])
    )
    dana = [c for c in cards if c.name == "Dana Whitfield"]
    kwame = [c for c in cards if c.name == "Kwame Mensah"]
    m["others_untouched"] = int(
        len(dana) == 1 and "dana@harboranalytics.example" in dana[0].email_keys
        and len(kwame) == 1 and "6175550123" in kwame[0].phone_keys
        and len([c for c in cards if c.name == "Jordan Lee"]) == 1
    )
    events_log = M.input_events(root / "input.log")
    m["ui_path"] = int(bool({e.get("kind") for e in events_log} & {"click", "drag"}))
    extra["input_events"] = len(events_log)
    cu.emit(m, METRICS, **extra)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
