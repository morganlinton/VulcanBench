#!/usr/bin/env python3
"""Seed for tasks/computer-use-v1/ledger-reconcile (deterministic).

    python scripts/computer-use/seeds/ledger_reconcile.py

Writes the Relay Desk seed (environment/relaydesk/seed.json), the ledger
answer key and workbook (environment/ledger/ledger.json, Q1-ledger.ods), an
empty discrepancies.txt, and the verifier copies under tests/. Forty Billing
tickets, one per invoice INV-2026-0101 to 0141 except 0131; a ledger of
forty-one rows sorted by posting date. Four disagreements: one amount with
two digits swapped, one invoice only in the ledger, one only in the tickets,
one entered twice. Everything else agrees, including two adjacent invoices
for the same customer whose amounts are digit-swaps of each other and a
customer spelt differently in the two places.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from ods import write_ods

TASK = Path(__file__).resolve().parents[3] / "tasks" / "computer-use-v1" / "ledger-reconcile"

PROJECTS = ["General Support", "Billing", "Shipping", "Account Access", "Integrations"]
USERS = [
    "Priya Raman",
    "Daniel Okafor",
    "Lena Fischer",
    "Sam Whitaker",
    "Noor Haddad",
    "Jonas Berg",
]
CUSTOMERS = [
    ("Kestrel Optics", "Amara Nwosu"),
    ("Brightwater Foods", "Lucas Meyer"),
    ("Tidewater Marine", "Sofia Marin"),
    ("Orchard Dental", "Ben Carter"),
    ("Quill Publishing", "Ines Duarte"),
    ("Sable Studio", "Yuki Tanaka"),
    ("Pinecrest Clinic", "Owen Hughes"),
    ("Redfern Cycles", "Mia Novak"),
    ("Linden & Park", "Elena Rossi"),
    ("Holm Logistics", "Dana Holm"),
    ("Harbor Lane Clinic", "Grace Mensah"),
    ("Vantage Retail", "Devon Price"),
]
MONTH_DAYS = {1: 31, 2: 28, 3: 31}

AMOUNT_MISMATCH = "INV-2026-0117"  # ledger 1,480.00, ticket 1,840.00
LEDGER_ONLY = "INV-2026-0131"  # no ticket
TICKET_ONLY = "INV-2026-0109"  # no ledger row
DUPLICATED = "INV-2026-0122"  # two ledger rows
DISCREPANCIES = {
    AMOUNT_MISMATCH: {"kind": "amount", "ledger": 1480.0, "ticket": 1840.0},
    LEDGER_ONLY: {"kind": "ledger-only"},
    TICKET_ONLY: {"kind": "ticket-only"},
    DUPLICATED: {"kind": "duplicate"},
}


def amount_for(n: int) -> float:
    base = [
        975.20,
        1840.00,
        2312.50,
        640.00,
        3105.75,
        1260.40,
        4480.00,
        2150.00,
        2510.00,
        880.90,
        1995.00,
        3320.25,
    ]
    return base[(n * 7) % len(base)] + ((n * 13) % 5) * 100.0


def invoices() -> list[dict]:
    out = []
    for n in range(101, 142):
        inv = f"INV-2026-{n:04d}"
        cust, requester = CUSTOMERS[(n * 5) % len(CUSTOMERS)]
        if inv in ("INV-2026-0115", "INV-2026-0116"):
            cust, requester = CUSTOMERS[0]  # adjacent, same customer
        amount = {"INV-2026-0115": 2150.0, "INV-2026-0116": 2510.0, AMOUNT_MISMATCH: 1840.0}.get(
            inv, amount_for(n)
        )
        day_index = (n - 101) * 2 + 3
        month = 1 if day_index <= 31 else (2 if day_index <= 59 else 3)
        day = day_index if month == 1 else (day_index - 31 if month == 2 else day_index - 59)
        day = min(day, MONTH_DAYS[month])
        out.append(
            {
                "invoice": inv,
                "customer": cust,
                "requester": requester,
                "amount": amount,
                "date": f"2026-{month:02d}-{day:02d}",
            }
        )
    return out


def build() -> tuple[dict, dict]:
    inv = invoices()
    by_id = {i["invoice"]: i for i in inv}
    # tickets: every invoice except the ledger-only one
    tickets = []
    for k, i in enumerate(inv):
        if i["invoice"] == LEDGER_ONLY:
            continue
        cust_label = "Linden and Park" if i["invoice"] == "INV-2026-0127" else i["customer"]
        amt = f"{i['amount']:,.2f}"
        tid = 6001 + k
        month_name = {1: "January", 2: "February", 3: "March"}[int(i["date"][5:7])]
        day = int(i["date"][8:10])
        tickets.append(
            {
                "id": tid,
                "subject": f"Invoice {i['invoice']}: {cust_label}, {amt}",
                "body": f"Invoice {i['invoice']} issued to {cust_label} on {day} {month_name} 2026 for {amt}. Terms 30 days.",
                "requester": i["requester"],
                "company": i["customer"],
                "project": "Billing",
                "priority": "Normal",
                "status": "Closed" if k % 3 else "Open",
                "assignee": USERS[k % len(USERS)],
                "created_at": f"{i['date']}T{9 + k % 7:02d}:{(k * 11) % 60:02d}",
                "updated_at": f"{i['date']}T{10 + k % 7:02d}:{(k * 17) % 60:02d}",
            }
        )
    assert len(tickets) == 40, len(tickets)
    others = [
        (
            "Password reset loop",
            "The reset email arrives but the link says it has expired.",
            "Account Access",
            "Kestrel Optics",
            "Amara Nwosu",
        ),
        (
            "Shipment tracking number not updating",
            "Order 88213 shows 'label created' for a week.",
            "Shipping",
            "Redfern Cycles",
            "Mia Novak",
        ),
        (
            "API key rotation",
            "We need to rotate our integration key before the audit.",
            "Integrations",
            "Tidewater Marine",
            "Sofia Marin",
        ),
        (
            "Report export cuts off at 500 rows",
            "The monthly report export stops at 500 rows.",
            "General Support",
            "Quill Publishing",
            "Ines Duarte",
        ),
        (
            "Add a second admin user",
            "Please add our office manager as an admin.",
            "Account Access",
            "Orchard Dental",
            "Ben Carter",
        ),
        (
            "Webhook retries",
            "Our endpoint was down for an hour; were events retried?",
            "Integrations",
            "Sable Studio",
            "Yuki Tanaka",
        ),
        (
            "Two-factor codes delayed",
            "SMS codes arrive after they expire.",
            "Account Access",
            "Pinecrest Clinic",
            "Owen Hughes",
        ),
        (
            "Dark mode request",
            "Any plans for a dark theme?",
            "General Support",
            "Brightwater Foods",
            "Lucas Meyer",
        ),
    ]
    for k, (subject, body, project, company, requester) in enumerate(others):
        tickets.append(
            {
                "id": 6101 + k,
                "subject": subject,
                "body": body,
                "requester": requester,
                "company": company,
                "project": project,
                "priority": ["Normal", "Low", "High"][k % 3],
                "status": ["Open", "Pending"][k % 2],
                "assignee": USERS[(k * 5) % len(USERS)],
                "created_at": f"2026-02-{4 + k:02d}T{9 + k:02d}:15",
                "updated_at": f"2026-02-{6 + k:02d}T{10 + k:02d}:40",
            }
        )
    seed = {
        "projects": PROJECTS,
        "users": USERS,
        "tickets": sorted(tickets, key=lambda t: t["id"]),
        "comments": [],
    }

    # ledger rows: every invoice except the ticket-only one, the mismatch amount,
    # and a second posting of the duplicated invoice, sorted by posting date
    rows = []
    for i in inv:
        if i["invoice"] == TICKET_ONLY:
            continue
        amount = (
            DISCREPANCIES[AMOUNT_MISMATCH]["ledger"]
            if i["invoice"] == AMOUNT_MISMATCH
            else i["amount"]
        )
        rows.append(
            {
                "invoice": i["invoice"],
                "customer": i["customer"],
                "posted": i["date"],
                "amount": amount,
            }
        )
    dup = by_id[DUPLICATED]
    rows.append(
        {
            "invoice": dup["invoice"],
            "customer": dup["customer"],
            "posted": "2026-02-20",
            "amount": dup["amount"],
        }
    )
    rows.sort(key=lambda r: (r["posted"], r["invoice"]))
    assert len(rows) == 41, len(rows)
    ledger = {
        "file": "Q1-ledger.ods",
        "sheet": "Q1 ledger",
        "headers": ["Invoice", "Customer", "Posted", "Amount"],
        "rows": rows,
        "report": "discrepancies.txt",
        "discrepancies": DISCREPANCIES,
        "agreeing_near_misses": {
            "INV-2026-0115/0116": "same customer, amounts 2,150.00 and 2,510.00, both consistent",
            "INV-2026-0127": "ticket says 'Linden and Park', ledger 'Linden & Park', same amount",
        },
    }
    return seed, ledger


def main() -> None:
    seed, ledger = build()
    seed_text = json.dumps(seed, indent=1, ensure_ascii=False) + "\n"
    ledger_text = json.dumps(ledger, indent=1, ensure_ascii=False) + "\n"
    for rel, text in (
        ("environment/relaydesk/seed.json", seed_text),
        ("tests/seed-relaydesk.json", seed_text),
        ("environment/ledger/ledger.json", ledger_text),
        ("tests/seed-ledger.json", ledger_text),
        ("environment/ledger/discrepancies.txt", ""),
    ):
        out = TASK / rel
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(text)
        print(f"wrote {out.relative_to(TASK.parent)}")
    sheet = {
        "name": ledger["sheet"],
        "widths": ["co2", "co3", "co2", "co2"],
        "rows": [ledger["headers"]]
        + [[r["invoice"], r["customer"], r["posted"], r["amount"]] for r in ledger["rows"]],
    }
    ods = write_ods(TASK / "environment" / "ledger" / ledger["file"], [sheet])
    print(f"wrote {ods.relative_to(TASK.parent)}; discrepancies {sorted(DISCREPANCIES)}")


if __name__ == "__main__":
    main()
