#!/usr/bin/env python3
"""Seed for tasks/computer-use-v1/relaydesk-northwind-escalation (deterministic).

    python scripts/computer-use/seeds/relaydesk_northwind_escalation.py

Writes environment/relaydesk/seed.json and tests/seed-relaydesk.json for the
task. Sixty tickets, five projects, six users. Twelve tickets mention
"Northwind" so a search spans two pages; five of them are the targets (open,
Northwind Traders, about the invoice export bug). The others are the
distractors described in the task's DESIGN.md.
"""

from __future__ import annotations

import json
from pathlib import Path

TASK = (
    Path(__file__).resolve().parents[3]
    / "tasks"
    / "computer-use-v1"
    / "relaydesk-northwind-escalation"
)

PROJECTS = ["General Support", "Billing", "Shipping", "Account Access", "Integrations"]
USERS = [
    "Priya Raman",
    "Daniel Okafor",
    "Lena Fischer",
    "Sam Whitaker",
    "Noor Haddad",
    "Jonas Berg",
]
NWT = "Northwind Traders"

# (id, subject, body, requester, company, project, priority, status, assignee, created, updated)
TICKETS: list[tuple] = [
    # ---- targets: open Northwind Traders tickets about the invoice export bug
    (
        2007,
        "Invoice export to CSV drops the VAT column",
        "Since the February update the invoice export leaves the VAT column empty for every line. We use the export for our monthly VAT return.",
        "Hanna Lindqvist",
        NWT,
        "General Support",
        "Normal",
        "Open",
        "Daniel Okafor",
        "2026-02-11T09:40",
        "2026-02-27T14:05",
    ),
    (
        2013,
        "Export of invoices times out for January",
        "Running the invoice export for January 2026 spins for a minute and then shows a timeout page. December exports fine.",
        "Tomas Reyes",
        NWT,
        "Integrations",
        "Low",
        "Open",
        "",
        "2026-02-13T11:20",
        "2026-02-20T10:15",
    ),
    (
        2021,
        "Invoice export missing credit notes",
        "The invoice export only includes positive invoices. Credit notes issued in the same period are missing, so the totals do not reconcile.",
        "Hanna Lindqvist",
        NWT,
        "General Support",
        "Normal",
        "Open",
        "Lena Fischer",
        "2026-02-17T15:02",
        "2026-03-01T08:50",
    ),
    (
        2034,
        "Cannot export invoices as PDF bundle",
        "Choosing PDF bundle in the invoice export dialog downloads a zero-byte file. CSV has the VAT problem reported separately, so right now no export works for us.",
        "Marcus Oyelaran",
        NWT,
        "Billing",
        "Normal",
        "Open",
        "Sam Whitaker",
        "2026-02-20T13:30",
        "2026-02-24T16:40",
    ),
    (
        2048,
        "Exported invoice totals do not match the statement",
        "The grand total in the invoice export is 1,840.00 lower than the account statement for the same range. Looks like the export skips invoices paid by credit.",
        "Tomas Reyes",
        NWT,
        "General Support",
        "High",
        "Open",
        "Jonas Berg",
        "2026-02-25T10:10",
        "2026-02-28T17:25",
    ),
    # ---- Northwind Traders distractors
    (
        2010,
        "Invoice export fails with error 500",
        "Clicking Export on the invoices page returns a server error. Started yesterday afternoon.",
        "Hanna Lindqvist",
        NWT,
        "Billing",
        "High",
        "Closed",
        "Priya Raman",
        "2026-01-21T09:05",
        "2026-01-29T12:00",
    ),
    (
        2016,
        "Shipping label export prints blank pages",
        "The label export for last week's orders produces blank pages on our Zebra printer. Single labels print fine.",
        "Ines Carvalho",
        NWT,
        "Shipping",
        "Normal",
        "Open",
        "Noor Haddad",
        "2026-02-14T08:30",
        "2026-02-26T09:10",
    ),
    (
        2025,
        "Change the billing address on our invoices",
        "We moved offices on 1 February. Invoices still show the old address; please update it to 14 Harbour Row, Leith.",
        "Marcus Oyelaran",
        NWT,
        "Billing",
        "Normal",
        "Open",
        "Daniel Okafor",
        "2026-02-18T10:45",
        "2026-02-23T11:30",
    ),
    (
        2030,
        "Two-factor codes arrive late",
        "SMS codes for sign-in take five to ten minutes to arrive, by which time they have expired.",
        "Ines Carvalho",
        NWT,
        "Account Access",
        "Normal",
        "Open",
        "Noor Haddad",
        "2026-02-19T14:20",
        "2026-02-19T14:20",
    ),
    (
        2042,
        "Wrong currency on March invoices",
        "The first three invoices in March are in USD although our account is set to EUR.",
        "Hanna Lindqvist",
        NWT,
        "Billing",
        "Normal",
        "Pending",
        "Daniel Okafor",
        "2026-03-02T08:15",
        "2026-03-02T08:15",
    ),
    (
        2055,
        "Webhook retries stop after two attempts",
        "Our order webhook endpoint was down for twenty minutes and the retries stopped after the second attempt instead of backing off.",
        "Tomas Reyes",
        NWT,
        "Integrations",
        "Low",
        "Open",
        "",
        "2026-02-27T16:00",
        "2026-02-27T16:00",
    ),
    # ---- Northwind Health (a different customer)
    (
        2037,
        "Invoice export shows the wrong company name",
        "Our invoice export header says Northwind Health Ltd instead of Northwind Health Group. The invoices themselves are right.",
        "Ruth Adeyemi",
        "Northwind Health",
        "General Support",
        "Normal",
        "Open",
        "Lena Fischer",
        "2026-02-21T09:00",
        "2026-02-25T13:45",
    ),
]

OTHER_COMPANIES = [
    ("Fathom Analytics", "Chloe Bernard"),
    ("Holm Logistics", "Dana Holm"),
    ("Brightwater Foods", "Yusuf Demir"),
    ("Kestrel Optics", "Amara Nwosu"),
    ("Linden & Park", "Olivia Park"),
    ("Sable Studio", "Mateo Ruiz"),
    ("Orchard Dental", "Grace Mbeki"),
    ("Tidewater Marine", "Sean Gallagher"),
    ("Quill Publishing", "Nadia Petrova"),
    ("Redfern Cycles", "Leo Hartmann"),
    ("Pinecrest Clinic", "Aisha Rahman"),
    ("Vantage Retail", "Ben Okoro"),
]

OTHER_SUBJECTS = [
    (
        "Password reset email never arrives",
        "I have requested a reset three times and nothing shows up, not even in spam.",
        "Account Access",
    ),
    (
        "Order stuck in Processing for four days",
        "Order B-11203 has been in Processing since Monday with no tracking number.",
        "Shipping",
    ),
    (
        "Charged twice for the same subscription",
        "My card statement shows two charges on the same day for the March plan.",
        "Billing",
    ),
    (
        "API key stopped working after rotation",
        "After rotating the key in Settings every request returns 401.",
        "Integrations",
    ),
    (
        "Cannot add a second user to the account",
        "The Invite button on the Users page does nothing when clicked.",
        "Account Access",
    ),
    (
        "Tracking page shows the wrong carrier",
        "The tracking link opens DHL but the parcel went with UPS.",
        "Shipping",
    ),
    (
        "Discount code not applied at checkout",
        "SPRING10 is accepted but the total does not change.",
        "Billing",
    ),
    (
        "Zapier integration drops attachments",
        "Files attached to orders are missing when the Zap fires.",
        "Integrations",
    ),
    (
        "Dashboard numbers do not add up",
        "The weekly revenue tile shows a figure that is higher than the sum of the daily tiles.",
        "General Support",
    ),
    (
        "Receipt PDF has the wrong date format",
        "Receipts show the date as 02/03/2026 which our accountant reads as 3 February.",
        "Billing",
    ),
    (
        "Return label link expired",
        "The return label emailed to our customer expired before they could print it.",
        "Shipping",
    ),
    (
        "Need to change the account owner",
        "Our previous admin has left; please move ownership to me.",
        "Account Access",
    ),
    (
        "Export of customer list is truncated",
        "The customer export stops at 5,000 rows but we have 7,200 customers.",
        "General Support",
    ),
    (
        "Session logs out every ten minutes",
        "Since last week I am signed out roughly every ten minutes while working.",
        "Account Access",
    ),
    (
        "Pallet shipment quoted at parcel rate",
        "The quote for 6 pallets came back at a parcel price and failed at booking.",
        "Shipping",
    ),
    (
        "Refund not received after 14 days",
        "The refund for invoice INV-20419 was confirmed on 10 February but has not arrived.",
        "Billing",
    ),
    (
        "Slack notifications arrive twice",
        "Every order notification posts twice to our channel.",
        "Integrations",
    ),
    (
        "Invoice shows a line item we did not order",
        "Invoice INV-20733 lists 2 units of the Pro add-on, which we never added.",
        "Billing",
    ),
    (
        "Search ignores part numbers",
        "Searching for KX-440 returns nothing although the part exists.",
        "General Support",
    ),
    (
        "Monthly report email is blank",
        "The report email arrives with a subject line but no body.",
        "General Support",
    ),
    (
        "Address validation rejects valid postcode",
        "The checkout refuses EH6 6JJ, which is our real postcode.",
        "Shipping",
    ),
    (
        "Upgrade to annual plan not reflected",
        "I upgraded on 1 March but the plan page still says Monthly.",
        "Billing",
    ),
    (
        "Webhook signature mismatch",
        "Our endpoint rejects every webhook because the signature header does not verify.",
        "Integrations",
    ),
    (
        "Need an invoice for last year's payments",
        "Please send a consolidated invoice for all 2025 payments for our audit.",
        "Billing",
    ),
    (
        "Cannot download the data export",
        "The link on the Export page opens a blank tab.",
        "General Support",
    ),
    (
        "Delivery left with neighbour without consent",
        "The courier left the parcel with a neighbour although we opted out.",
        "Shipping",
    ),
    (
        "Login works on desktop but not on the phone",
        "The mobile sign-in page loops back to itself after entering the code.",
        "Account Access",
    ),
    (
        "Sales tax applied to an exempt order",
        "Order B-11490 should be tax exempt but tax was added.",
        "Billing",
    ),
    (
        "Inventory sync stops at midnight",
        "The nightly inventory sync from our ERP stops partway every night.",
        "Integrations",
    ),
    (
        "Statement export has duplicate rows",
        "The account statement export lists some payments twice.",
        "General Support",
    ),
    (
        "Custom field missing from the order form",
        "The PO number field we added is not shown on new orders.",
        "General Support",
    ),
    (
        "Shipment split without notice",
        "Order B-11388 arrived in two boxes on different days without any email.",
        "Shipping",
    ),
    (
        "Cannot remove an old payment card",
        "The expired card cannot be deleted from Billing settings.",
        "Billing",
    ),
    (
        "OAuth consent screen shows the wrong app name",
        "Our integration shows up as Test App on the consent screen.",
        "Integrations",
    ),
    (
        "Priority support line not answering",
        "We are on the Plus plan but the priority number goes to voicemail.",
        "General Support",
    ),
    (
        "Invoice emails go to the wrong contact",
        "Invoices are sent to our former finance contact despite the change.",
        "Billing",
    ),
    (
        "Picking list prints in the wrong order",
        "The picking list sorts by product name instead of bin location.",
        "Shipping",
    ),
    (
        "Two accounts with the same email",
        "I seem to have two accounts and can only log in to the older one.",
        "Account Access",
    ),
    (
        "Webhook payload missing customer notes",
        "The order webhook no longer includes the notes field.",
        "Integrations",
    ),
    (
        "Late fee applied in error",
        "A late fee was added although the invoice was paid on the due date.",
        "Billing",
    ),
    (
        "Export to QuickBooks fails on refunds",
        "The QuickBooks export errors out whenever the period includes a refund.",
        "Integrations",
    ),
    (
        "Order confirmation email in the wrong language",
        "Our customers in Lyon receive confirmations in German.",
        "General Support",
    ),
    (
        "Returns portal shows another customer's order",
        "Entering our order number shows a different customer's details.",
        "Shipping",
    ),
    (
        "Need VAT number on invoices",
        "Our VAT number is missing from all invoices this year.",
        "Billing",
    ),
    (
        "Pagination skips a page in the orders list",
        "Page 3 of the orders list repeats page 2.",
        "General Support",
    ),
    (
        "Can't change the time zone",
        "The time zone dropdown in Settings is empty.",
        "Account Access",
    ),
    (
        "SFTP delivery of reports stopped",
        "The nightly SFTP push of our reports has not run since 20 February.",
        "Integrations",
    ),
    (
        "Shipping cost shows zero for international",
        "International orders show 0.00 shipping at checkout.",
        "Shipping",
    ),
]

PRIORITY_CYCLE = ["Normal", "Low", "Normal", "High", "Normal", "Urgent", "Low", "Normal"]
STATUS_CYCLE = [
    "Open",
    "Open",
    "Pending",
    "Open",
    "Closed",
    "Open",
    "Open",
    "Pending",
    "Open",
    "Closed",
]


def build() -> dict:
    used = {t[0] for t in TICKETS}
    rows = [
        dict(
            zip(
                (
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
                ),
                t,
                strict=True,
            )
        )
        for t in TICKETS
    ]
    i = 0
    for tid in range(2001, 2061):
        if tid in used:
            continue
        subject, body, project = OTHER_SUBJECTS[i % len(OTHER_SUBJECTS)]
        company, requester = OTHER_COMPANIES[i % len(OTHER_COMPANIES)]
        day = 5 + (i * 7) % 24
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
            "ticket_id": 2010,
            "author": "Priya Raman",
            "body": "Fixed in the 29 January deploy; closing.",
            "created_at": "2026-01-29T12:00",
        },
        {
            "id": 2,
            "ticket_id": 2007,
            "author": "Daniel Okafor",
            "body": "Reproduced with the February build. Passing to engineering.",
            "created_at": "2026-02-12T10:20",
        },
        {
            "id": 3,
            "ticket_id": 2025,
            "author": "Daniel Okafor",
            "body": "Address change needs a signed form; sent the template.",
            "created_at": "2026-02-23T11:30",
        },
        {
            "id": 4,
            "ticket_id": 2048,
            "author": "Jonas Berg",
            "body": "Asked the customer for the statement export to compare.",
            "created_at": "2026-02-28T17:25",
        },
    ]
    return {"projects": PROJECTS, "users": USERS, "tickets": rows, "comments": comments}


def main() -> None:
    seed = build()
    assert len(seed["tickets"]) == 60
    text = json.dumps(seed, indent=1, ensure_ascii=False) + "\n"
    for rel in ("environment/relaydesk/seed.json", "tests/seed-relaydesk.json"):
        out = TASK / rel
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(text)
        print(f"wrote {out.relative_to(TASK.parent)}")
    nw = [t for t in seed["tickets"] if "Northwind" in t["company"]]
    print(f"{len(nw)} Northwind tickets; targets 2007 2013 2021 2034 2048")


if __name__ == "__main__":
    main()
