#!/usr/bin/env python3
"""contacts-duplicate-merge checker: the Contacts store copied from the
stopped VM.

    check.py /var/lib/vbmac

A person is identified by their phone numbers (last ten digits) and email
addresses. For each person, the cards holding any of their identifiers must
be exactly one card, holding all of them and nobody else's, under one of the
person's names; for the duplicated people, their notes must survive on it.

Sub-metrics (reward is 1 only when all are 1):
  artifact_present   the Contacts store was collected
  exact_name_merged  Maria Gonzalez and Tom Becker (two cards each, same
                     name) are one card each with every detail
  variant_merged     Robert/Bob Alvarez (shared email) and Jennifer/Jen
                     Okafor (shared mobile) are one card each with every detail
  notes_kept         Tom's and Bob's notes are on their merged cards
  distinct_kept      both Chris Parks, Sam Patel and Samantha Patel are still
                     separate cards with only their own details
  others_untouched   the ten other people and the owner's card are unchanged
  ui_path            the MCP server logged pointer input
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, "/opt/vbdesk")
import cu_check as cu  # noqa: E402
import mac_state as M  # noqa: E402

METRICS = (
    "artifact_present",
    "exact_name_merged",
    "variant_merged",
    "notes_kept",
    "distinct_kept",
    "others_untouched",
    "ui_path",
)


def ident(phones: tuple[str, ...] = (), emails: tuple[str, ...] = ()) -> frozenset[str]:
    return frozenset({M.phone_key(p) for p in phones} | {e.lower() for e in emails})


# name(s) allowed on the surviving card, identifiers, words its note must keep
MERGED = {
    "maria": (
        {"Maria Gonzalez"},
        ident(("3125550147", "3125550190"), ("maria.gonzalez@example.com", "mgonzalez@fieldstone.example")),
        (),
    ),
    "tom": ({"Tom Becker"}, ident(("6465550112",), ("tom.becker@example.net",)), ("kayak",)),
    "alvarez": (
        {"Robert Alvarez", "Bob Alvarez"},
        ident(("5035550175", "5035550133"), ("bob@alvarezplumbing.example",)),
        ("water heater",),
    ),
    "okafor": (
        {"Jennifer Okafor", "Jen Okafor"},
        ident(("4045550128",), ("jokafor@brightline.example", "jen.okafor@example.org")),
        (),
    ),
}
DISTINCT = {
    "chris_nw": ({"Chris Park"}, ident(("4155550134",), ("chris@northwind.example",))),
    "chris_hd": ({"Chris Park"}, ident(("6175550199",), ("office@harbordental.example",))),
    "sam": ({"Sam Patel"}, ident(("2075550110",), ("sam@patelcycles.example",))),
    "samantha": ({"Samantha Patel"}, ident(("6175550186",), ("samantha.patel@example.com",))),
}
OTHERS = {
    "Elena Novak": ident(("2125550161",), ("elena.novak@example.com",)),
    "Kwame Mensah": ident(("6175550123",), ("kwame@mensahco.example",)),
    "Lucy Zhang": ident(("8575550109",)),
    "Omar Haddad": ident(("6175550172",), ("omar.haddad@example.net",)),
    "Grace Liu": ident((), ("grace@liudesign.example",)),
    "Henrik Larsen": ident(("4520550144",)),
    "Priya Shah": ident(("7815550155",), ("priya.shah@example.org",)),
    "Diego Torres": ident(("6175550141",)),
    "Nina Petrov": ident((), ("nina.petrov@example.com",)),
    "Alex Morgan": ident(("3395550117",)),
}


def ids(c: M.Contact) -> frozenset[str]:
    return frozenset(c.phone_keys | c.email_keys)


def one_card(cards: list[M.Contact], names: set[str], want: frozenset[str]) -> M.Contact | None:
    """The single card for a person, or None if there is not exactly one
    card touching their identifiers that holds them all and nothing else."""
    touching = [c for c in cards if ids(c) & want]
    if len(touching) != 1:
        return None
    c = touching[0]
    if ids(c) != want or c.name not in names:
        return None
    return c


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
    found = {k: one_card(cards, names, want) for k, (names, want, _) in MERGED.items()}
    extra["merged_ok"] = sorted(k for k, c in found.items() if c is not None)
    m["exact_name_merged"] = int(found["maria"] is not None and found["tom"] is not None)
    m["variant_merged"] = int(found["alvarez"] is not None and found["okafor"] is not None)
    m["notes_kept"] = int(
        all(
            found[k] is not None and all(w in found[k].note.lower() for w in words)  # type: ignore[union-attr]
            for k, (_, _, words) in MERGED.items()
            if words
        )
    )
    distinct = {k: one_card(cards, names, want) for k, (names, want) in DISTINCT.items()}
    extra["distinct_ok"] = sorted(k for k, c in distinct.items() if c is not None)
    m["distinct_kept"] = int(all(c is not None for c in distinct.values()))
    others_ok = [name for name, want in OTHERS.items() if one_card(cards, {name}, want) is not None]
    owner = [c for c in cards if c.name == "Jordan Lee"]
    m["others_untouched"] = int(len(others_ok) == len(OTHERS) and len(owner) == 1)
    events_log = M.input_events(root / "input.log")
    m["ui_path"] = int(bool({e.get("kind") for e in events_log} & {"click", "drag"}))
    extra["input_events"] = len(events_log)
    cu.emit(m, METRICS, **extra)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
