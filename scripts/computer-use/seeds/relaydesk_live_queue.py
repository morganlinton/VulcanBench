#!/usr/bin/env python3
"""Seed for tasks/computer-use-v1/relaydesk-live-queue (deterministic).

    python scripts/computer-use/seeds/relaydesk_live_queue.py

Writes environment/relaydesk/seed.json, environment/relaydesk/arrivals.json
and their copies under tests/. Forty-eight tickets, six projects, seven
users. Harbor Lane Clinic has four open tickets about the patient portal
outage (the targets), three open tickets about other things, three closed
tickets (one of them the outage, already handled), and two look-alike
customers have portal outage tickets of their own. Two tickets arrive while
the task runs: one more outage ticket from the clinic (a target) and an
account request from the clinic (not a target).
"""

from __future__ import annotations

import json
from pathlib import Path

TASK = Path(__file__).resolve().parents[3] / "tasks" / "computer-use-v1" / "relaydesk-live-queue"

PROJECTS = ["General Support", "Billing", "Shipping", "Account Access", "Integrations", "Platform"]
USERS = [
    "Priya Raman",
    "Daniel Okafor",
    "Lena Fischer",
    "Sam Whitaker",
    "Noor Haddad",
    "Jonas Berg",
    "Marcus Webb",
]
HLC = "Harbor Lane Clinic"
HLD = "Harbor Lane Dental"
HVP = "Harborview Pharmacy"

# (id, subject, body, requester, company, project, priority, status, assignee, created, updated)
TICKETS: list[tuple] = [
    # ---- targets: open Harbor Lane Clinic tickets about the patient portal outage
    (
        3004,
        "Patient portal has been down since Friday",
        "None of our staff can open the patient portal since Friday afternoon. The page times out. We rely on it for the morning appointment list.",
        "Grace Mensah",
        HLC,
        "General Support",
        "Normal",
        "Open",
        "Lena Fischer",
        "2026-02-27T16:40",
        "2026-02-28T09:10",
    ),
    (
        3011,
        "Still cannot reach the portal from the clinic",
        "Following up on the outage: the portal is still unreachable from every machine at the clinic this morning. Our IT contractor says the problem is on your side.",
        "Tom Achebe",
        HLC,
        "Account Access",
        "Low",
        "Open",
        "",
        "2026-03-01T08:55",
        "2026-03-01T08:55",
    ),
    (
        3019,
        "Portal outage: appointment reminders not going out",
        "Because the portal is down, the automatic appointment reminders for Monday did not go out. Patients are calling to ask. Please treat as part of the outage.",
        "Grace Mensah",
        HLC,
        "Integrations",
        "Normal",
        "Open",
        "Daniel Okafor",
        "2026-02-28T11:20",
        "2026-03-01T15:05",
    ),
    (
        3027,
        "Outage update needed for our front desk",
        "Can someone send our front desk an update on the portal outage and an estimated fix time? They are turning patients away from the kiosk.",
        "Tom Achebe",
        HLC,
        "General Support",
        "High",
        "Open",
        "Marcus Webb",
        "2026-03-01T17:30",
        "2026-03-02T08:05",
    ),
    # ---- Harbor Lane Clinic, closed: the outage ticket already handled, and two old ones
    (
        3008,
        "Portal down for everyone at the clinic",
        "The patient portal is down for every user here. Please advise.",
        "Grace Mensah",
        HLC,
        "Platform",
        "High",
        "Closed",
        "Marcus Webb",
        "2026-02-27T14:05",
        "2026-02-27T15:50",
    ),
    (
        3002,
        "Welcome call notes",
        "Notes from the onboarding call: two sites, kiosk at the front desk, reminders by SMS.",
        "Grace Mensah",
        HLC,
        "General Support",
        "Low",
        "Closed",
        "Priya Raman",
        "2026-01-06T10:00",
        "2026-01-06T10:40",
    ),
    (
        3045,
        "Training session request",
        "Could we book a refresher training session for the reception team in March?",
        "Tom Achebe",
        HLC,
        "General Support",
        "Low",
        "Closed",
        "Sam Whitaker",
        "2026-02-10T09:15",
        "2026-02-12T14:00",
    ),
    # ---- Harbor Lane Clinic, open, not about the outage
    (
        3015,
        "Update billing address for the clinic",
        "Our invoices should go to the new practice address from April. I will send the details on headed paper.",
        "Tom Achebe",
        HLC,
        "Billing",
        "Normal",
        "Open",
        "Noor Haddad",
        "2026-02-24T13:10",
        "2026-02-25T09:30",
    ),
    (
        3033,
        "Invoice for February missing",
        "We have not received the February invoice yet; January's arrived on the 3rd. Could you resend it?",
        "Grace Mensah",
        HLC,
        "Billing",
        "Normal",
        "Open",
        "",
        "2026-03-02T08:40",
        "2026-03-02T08:40",
    ),
    (
        3038,
        "Export of patient list to CSV",
        "Is there a way to export the patient list with phone numbers to a spreadsheet? Our old system could.",
        "Grace Mensah",
        HLC,
        "Integrations",
        "Low",
        "Pending",
        "Jonas Berg",
        "2026-02-18T15:25",
        "2026-02-20T10:10",
    ),
    # ---- look-alike customers with portal outage tickets
    (
        3022,
        "Portal outage affecting our practice",
        "Our patient portal has been unavailable since Friday evening. Is this the same incident other practices are reporting?",
        "Farah Qureshi",
        HLD,
        "General Support",
        "Normal",
        "Open",
        "Lena Fischer",
        "2026-02-28T10:05",
        "2026-02-28T12:45",
    ),
    (
        3040,
        "Portal outage: prescription pickups delayed",
        "With the portal down we cannot see pickup requests. Please add us to the incident updates.",
        "Devon Price",
        HVP,
        "General Support",
        "High",
        "Pending",
        "Marcus Webb",
        "2026-02-28T13:30",
        "2026-03-01T11:20",
    ),
]

ARRIVALS: list[dict] = [
    {
        "after_s": 60,
        "ticket": {
            "id": 3101,
            "subject": "Portal still unreachable this morning, any news?",
            "body": "It is Monday 09:20 and the patient portal is still unreachable from the clinic. The waiting room is full. Please let us know where things stand with the outage.",
            "requester": "Grace Mensah",
            "company": HLC,
            "project": "General Support",
            "priority": "Normal",
            "status": "Open",
            "assignee": "",
            "created_at": "2026-03-02T09:20",
            "updated_at": "2026-03-02T09:20",
        },
    },
    {
        "after_s": 150,
        "ticket": {
            "id": 3102,
            "subject": "New user account for Dr Mensah",
            "body": "Please create a user account for Dr Adaeze Mensah, who joins the clinic on 9 March. Standard clinician permissions.",
            "requester": "Tom Achebe",
            "company": HLC,
            "project": "Account Access",
            "priority": "Normal",
            "status": "Open",
            "assignee": "",
            "created_at": "2026-03-02T09:22",
            "updated_at": "2026-03-02T09:22",
        },
    },
]

OTHER_SUBJECTS = [
    (
        "Password reset loop",
        "The reset email arrives but the link says it has expired.",
        "Account Access",
    ),
    ("Duplicate invoice #4471", "We were invoiced twice for January.", "Billing"),
    (
        "Shipment tracking number not updating",
        "Order 88213 shows 'label created' for a week.",
        "Shipping",
    ),
    ("API key rotation", "We need to rotate our integration key before the audit.", "Integrations"),
    (
        "Report export cuts off at 500 rows",
        "The monthly report export stops at 500 rows.",
        "General Support",
    ),
    ("Add a second admin user", "Please add our office manager as an admin.", "Account Access"),
    ("Refund not received", "The refund agreed on 12 February has not arrived.", "Billing"),
    ("Wrong items in order 88302", "Received the wrong SKU in the last shipment.", "Shipping"),
    ("Webhook retries", "Our endpoint was down for an hour; were events retried?", "Integrations"),
    ("Dark mode request", "Any plans for a dark theme?", "General Support"),
    ("Two-factor codes delayed", "SMS codes arrive after they expire.", "Account Access"),
    ("VAT number missing on invoices", "Our VAT number is not printed on invoices.", "Billing"),
]
OTHER_COMPANIES = [
    ("Kestrel Optics", "Amara Nwosu"),
    ("Brightwater Foods", "Lucas Meyer"),
    ("Tidewater Marine", "Sofia Marin"),
    ("Orchard Dental", "Ben Carter"),
    ("Quill Publishing", "Ines Duarte"),
    ("Sable Studio", "Yuki Tanaka"),
    ("Pinecrest Clinic", "Owen Hughes"),
    ("Redfern Cycles", "Mia Novak"),
    ("Linden & Park", "Elena Rossi"),
]
PRIORITY_CYCLE = ["Normal", "Low", "High", "Normal", "Urgent"]
STATUS_CYCLE = ["Open", "Pending", "Open", "Closed", "Open", "Waiting on Finance"]
FIELDS = (
    "id",
    "subject",
    "body",
    "requester",
    "company",
    "project",
    "priority",
    "status",
    "assignee",
    "created_at",
    "updated_at",
)


def build() -> tuple[dict, list[dict]]:
    used = {t[0] for t in TICKETS}
    rows = [dict(zip(FIELDS, t, strict=True)) for t in TICKETS]
    i = 0
    for tid in range(3001, 3049):
        if tid in used:
            continue
        subject, body, project = OTHER_SUBJECTS[i % len(OTHER_SUBJECTS)]
        company, requester = OTHER_COMPANIES[i % len(OTHER_COMPANIES)]
        day = 4 + (i * 7) % 24
        month = 1 if i % 3 == 0 else 2
        created = f"2026-{month:02d}-{day:02d}T{8 + i % 9:02d}:{(i * 13) % 60:02d}"
        upd_day = min(28, day + (i % 5))
        updated = f"2026-{month:02d}-{upd_day:02d}T{9 + i % 8:02d}:{(i * 17) % 60:02d}"
        rows.append(
            {
                "id": tid,
                "subject": subject,
                "body": body,
                "requester": requester,
                "company": company,
                "project": project,
                "priority": PRIORITY_CYCLE[i % len(PRIORITY_CYCLE)],
                "status": STATUS_CYCLE[i % len(STATUS_CYCLE)],
                "assignee": USERS[(i * 5) % len(USERS)] if i % 4 else "",
                "created_at": created,
                "updated_at": updated,
            }
        )
        i += 1
    rows.sort(key=lambda r: r["id"])
    comments = [
        {
            "id": 1,
            "ticket_id": 3008,
            "author": "Marcus Webb",
            "body": "Portal restored for Harbor Lane at 15:40 after the load balancer swap. Closing; the Friday recurrence is tracked on the new tickets.",
            "created_at": "2026-02-27T15:50",
        },
        {
            "id": 2,
            "ticket_id": 3004,
            "author": "Lena Fischer",
            "body": "Confirmed from our side: the portal cluster is degraded again. Escalating.",
            "created_at": "2026-02-28T09:10",
        },
        {
            "id": 3,
            "ticket_id": 3015,
            "author": "Noor Haddad",
            "body": "Waiting for the headed-paper confirmation before changing the billing address.",
            "created_at": "2026-02-25T09:30",
        },
        {
            "id": 4,
            "ticket_id": 3040,
            "author": "Marcus Webb",
            "body": "Added Harborview to the incident distribution list.",
            "created_at": "2026-03-01T11:20",
        },
    ]
    seed = {"projects": PROJECTS, "users": USERS, "tickets": rows, "comments": comments}
    return seed, ARRIVALS


def main() -> None:
    seed, arrivals = build()
    assert len(seed["tickets"]) == 48, len(seed["tickets"])
    assert not ({a["ticket"]["id"] for a in arrivals} & {t["id"] for t in seed["tickets"]})
    seed_text = json.dumps(seed, indent=1, ensure_ascii=False) + "\n"
    arr_text = json.dumps(arrivals, indent=1, ensure_ascii=False) + "\n"
    for rel, text in (
        ("environment/relaydesk/seed.json", seed_text),
        ("tests/seed-relaydesk.json", seed_text),
        ("environment/relaydesk/arrivals.json", arr_text),
        ("tests/arrivals-relaydesk.json", arr_text),
    ):
        out = TASK / rel
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(text)
        print(f"wrote {out.relative_to(TASK.parent)}")
    hl = [t["id"] for t in seed["tickets"] if "Harbor Lane" in t["company"]]
    print(f"{len(hl)} Harbor Lane tickets seeded {hl}; targets 3004 3011 3019 3027 + arrival 3101")


if __name__ == "__main__":
    main()
