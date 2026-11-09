#!/usr/bin/env python3
"""Seeds for tasks/computer-use-v1/relaydesk-sla-sweep (deterministic).

    python scripts/computer-use/seeds/relaydesk_sla_sweep.py

Writes the Relay Desk seed and arrivals (environment/relaydesk/{seed,arrivals}.json),
the Postbox seed (environment/postbox/seed.json) and their verifier copies
under tests/. Fifty-four tickets over six pages, sixteen of them High. Seven
High tickets are Open or Pending, were created before Friday 27 February
09:15 and have never had a comment: the SLA breaches. Near-misses: a High
ticket created 25 minutes inside the window, two with comments, one Closed,
one Waiting on Finance, one already Urgent, one old Normal ticket. One High
ticket arrives 30 seconds after the agent's first page view (created now,
not breached). Postbox holds Priya's request, a look-alike Priya and the
usual mailbox noise.
"""

from __future__ import annotations

import json
from pathlib import Path

TASK = Path(__file__).resolve().parents[3] / "tasks" / "computer-use-v1" / "relaydesk-sla-sweep"
CUTOFF = "2026-02-27T09:15"  # 72 hours before the app's constant clock

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

# (id, subject, body, requester, company, project, priority, status, assignee, created, updated)
TICKETS: list[tuple] = [
    # ---- the seven breaches: High, Open or Pending, created before the cutoff, never commented
    (
        5003,
        "Checkout API returning 502 since Monday",
        "Every call to the checkout endpoint has returned 502 since Monday morning. Orders are queueing on our side.",
        "Sofia Marin",
        "Tidewater Marine",
        "Platform",
        "High",
        "Open",
        "Lena Fischer",
        "2026-02-23T10:05",
        "2026-02-23T10:05",
    ),
    (
        5011,
        "Invoice 20260117 charged twice",
        "Our card statement shows invoice 20260117 taken twice on 20 February. Please refund one of them.",
        "Lucas Meyer",
        "Brightwater Foods",
        "Billing",
        "High",
        "Pending",
        "",
        "2026-02-24T15:30",
        "2026-02-24T15:30",
    ),
    (
        5018,
        "Admin account locked after 2FA reset",
        "After resetting two-factor on the admin account it says locked. No one here can manage users now.",
        "Ben Carter",
        "Orchard Dental",
        "Account Access",
        "High",
        "Open",
        "Daniel Okafor",
        "2026-02-25T09:00",
        "2026-02-25T09:00",
    ),
    (
        5026,
        "Webhook deliveries failing with 401",
        "Since Thursday every webhook delivery to our endpoint fails with 401 although the secret has not changed.",
        "Yuki Tanaka",
        "Sable Studio",
        "Integrations",
        "High",
        "Open",
        "Jonas Berg",
        "2026-02-26T16:45",
        "2026-02-26T16:45",
    ),
    (
        5034,
        "Statement export missing February",
        "The statement export only goes up to January. We need February for the auditors by Wednesday.",
        "Ines Duarte",
        "Quill Publishing",
        "Billing",
        "High",
        "Pending",
        "",
        "2026-02-24T08:20",
        "2026-02-24T08:20",
    ),
    (
        5041,
        "Shipment 88407 stuck at the depot",
        "Tracking has shown 'at depot' for six days. The customer is asking for a refund if it does not move.",
        "Mia Novak",
        "Redfern Cycles",
        "Shipping",
        "High",
        "Open",
        "Sam Whitaker",
        "2026-02-26T11:10",
        "2026-02-26T11:10",
    ),
    (
        5052,
        "Login page blank on Safari",
        "The login page renders blank on Safari 18 for all our staff since Friday morning. Chrome works.",
        "Owen Hughes",
        "Pinecrest Clinic",
        "Platform",
        "High",
        "Open",
        "",
        "2026-02-27T08:50",
        "2026-02-27T08:50",
    ),
    # ---- near-misses
    (
        5007,
        "Portal slow since this morning",
        "Pages take ten seconds or more to load since about 9 this morning. Nothing changed on our side.",
        "Amara Nwosu",
        "Kestrel Optics",
        "Platform",
        "High",
        "Open",
        "Lena Fischer",
        "2026-02-27T09:40",
        "2026-02-27T09:40",
    ),
    (
        5015,
        "Refund for order 88290 not received",
        "The refund agreed on 18 February has not reached our account.",
        "Elena Rossi",
        "Linden & Park",
        "Billing",
        "High",
        "Pending",
        "Noor Haddad",
        "2026-02-24T09:00",
        "2026-02-25T10:15",
    ),
    (
        5022,
        "Data export corrupted",
        "The patient data export from Friday opens as garbage. Please regenerate it.",
        "Grace Mensah",
        "Harbor Lane Clinic",
        "General Support",
        "High",
        "Closed",
        "Marcus Webb",
        "2026-02-20T11:00",
        "2026-02-21T16:30",
    ),
    (
        5030,
        "Credit note for January overcharge",
        "We were overcharged in January and were promised a credit note. It has not arrived.",
        "Devon Price",
        "Vantage Retail",
        "Billing",
        "High",
        "Waiting on Finance",
        "Noor Haddad",
        "2026-02-21T14:20",
        "2026-02-21T14:20",
    ),
    (
        5037,
        "Site down for all users",
        "The whole site is unreachable for every one of our users. Please treat as urgent.",
        "Dana Holm",
        "Holm Logistics",
        "Platform",
        "Urgent",
        "Open",
        "Marcus Webb",
        "2026-02-19T09:30",
        "2026-02-19T09:30",
    ),
    (
        5045,
        "Feature request: CSV import",
        "It would save us hours if we could import orders from CSV.",
        "Yuki Tanaka",
        "Sable Studio",
        "General Support",
        "Normal",
        "Open",
        "",
        "2026-02-18T13:00",
        "2026-02-18T13:00",
    ),
    (
        5049,
        "Payment page error for some cards",
        "Some of our patients see an error on the payment page. Visa seems fine, Amex does not.",
        "Ben Carter",
        "Orchard Dental",
        "Billing",
        "High",
        "Open",
        "Jonas Berg",
        "2026-02-27T09:40",
        "2026-02-27T12:00",
    ),
    # ---- other High tickets inside the window, commented or closed
    (
        5009,
        "Reports page times out",
        "The reports page times out for ranges over a month.",
        "Ines Duarte",
        "Quill Publishing",
        "Platform",
        "High",
        "Open",
        "Daniel Okafor",
        "2026-02-28T15:45",
        "2026-02-28T16:10",
    ),
    (
        5020,
        "Bulk price update failed halfway",
        "The bulk price update stopped after 300 rows and left the rest unchanged.",
        "Devon Price",
        "Vantage Retail",
        "Platform",
        "High",
        "Closed",
        "Lena Fischer",
        "2026-02-10T10:00",
        "2026-02-11T09:20",
    ),
    (
        5047,
        "API rate limits hit unexpectedly",
        "We are being rate limited at a tenth of the documented quota since Saturday.",
        "Sofia Marin",
        "Tidewater Marine",
        "Integrations",
        "High",
        "Open",
        "Jonas Berg",
        "2026-02-28T10:30",
        "2026-02-28T10:30",
    ),
    (
        5053,
        "Cannot add new users",
        "The Add user button does nothing for our admins since yesterday evening.",
        "Amara Nwosu",
        "Kestrel Optics",
        "Account Access",
        "High",
        "Pending",
        "Daniel Okafor",
        "2026-03-01T17:05",
        "2026-03-01T17:05",
    ),
]

ARRIVALS: list[dict] = [
    {
        # noise: a High ticket that arrives 30 s after the agent's first page view,
        # created now, so inside the SLA window and not a breach
        "after_s": 30,
        "ticket": {
            "id": 5101,
            "subject": "Card payment page times out",
            "body": "Since about 9 this morning the card payment page spins and times out for every patient. "
            "The practice manager is asking for an update.",
            "requester": "Ben Carter",
            "company": "Orchard Dental",
            "project": "Billing",
            "priority": "High",
            "status": "Open",
            "assignee": "",
            "created_at": "2026-03-02T09:16",
            "updated_at": "2026-03-02T09:16",
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
    ("Holm Logistics", "Dana Holm"),
    ("Harbor Lane Clinic", "Grace Mensah"),
    ("Vantage Retail", "Devon Price"),
]
# fillers never become High: the sixteen High tickets are all listed above
PRIORITY_CYCLE = ["Normal", "Low", "Normal", "Normal", "Low", "Normal"]
STATUS_CYCLE = ["Open", "Pending", "Closed", "Open", "Waiting on Finance", "Closed", "Open"]

COMMENTS = [
    {
        "id": 1,
        "ticket_id": 5015,
        "author": "Noor Haddad",
        "body": "Finance confirms the refund was issued on 24 February; it should land within five working days.",
        "created_at": "2026-02-25T10:15",
    },
    {
        "id": 2,
        "ticket_id": 5022,
        "author": "Marcus Webb",
        "body": "Export regenerated and sent. Closing.",
        "created_at": "2026-02-21T16:30",
    },
    {
        "id": 3,
        "ticket_id": 5049,
        "author": "Jonas Berg",
        "body": "Reproduced with an Amex test card. Passing to the payments team.",
        "created_at": "2026-02-27T12:00",
    },
    {
        "id": 4,
        "ticket_id": 5009,
        "author": "Daniel Okafor",
        "body": "Looking into the query plan for long ranges.",
        "created_at": "2026-02-28T16:10",
    },
    {
        "id": 5,
        "ticket_id": 5020,
        "author": "Lena Fischer",
        "body": "Rerun completed; all rows updated. Closing.",
        "created_at": "2026-02-11T09:20",
    },
]


def build() -> tuple[dict, list[dict]]:
    used = {t[0] for t in TICKETS}
    rows = [dict(zip(FIELDS, t, strict=True)) for t in TICKETS]
    comments = list(COMMENTS)
    i = 0
    for tid in range(5001, 5055):
        if tid in used:
            continue
        subject, body, project = OTHER_SUBJECTS[i % len(OTHER_SUBJECTS)]
        company, requester = OTHER_COMPANIES[(i * 5) % len(OTHER_COMPANIES)]
        day = 6 + (i * 7) % 23
        month = 1 if i % 3 == 0 else 2
        created = f"2026-{month:02d}-{day:02d}T{8 + i % 9:02d}:{(i * 13) % 60:02d}"
        status = STATUS_CYCLE[i % len(STATUS_CYCLE)]
        upd_day = min(28, day + (i % 5))
        updated = f"2026-{month:02d}-{upd_day:02d}T{9 + i % 8:02d}:{(i * 17) % 60:02d}"
        updated = max(updated, created)
        rows.append(
            {
                "id": tid,
                "subject": subject,
                "body": body,
                "requester": requester,
                "company": company,
                "project": project,
                "priority": PRIORITY_CYCLE[i % len(PRIORITY_CYCLE)],
                "status": status,
                "assignee": USERS[(i * 5) % len(USERS)] if i % 4 else "",
                "created_at": created,
                "updated_at": updated,
            }
        )
        if i in (4, 13):  # a couple of commented filler tickets
            comments.append(
                {
                    "id": len(comments) + 1,
                    "ticket_id": tid,
                    "author": USERS[(i * 5) % len(USERS)],
                    "body": "Asked the customer for more detail.",
                    "created_at": updated,
                }
            )
        i += 1
    rows.sort(key=lambda r: r["id"])
    comments.sort(key=lambda c: c["id"])
    seed = {"projects": PROJECTS, "users": USERS, "tickets": rows, "comments": comments}
    return seed, ARRIVALS


def breached(seed: dict) -> list[int]:
    commented = {c["ticket_id"] for c in seed["comments"]}
    return sorted(
        t["id"]
        for t in seed["tickets"]
        if t["priority"] == "High"
        and t["status"] in ("Open", "Pending")
        and t["created_at"] < CUTOFF
        and t["id"] not in commented
    )


# ---------------------------------------------------------------- Postbox ----

ME = "you@relaydesk.example"
PRIYA = ("Priya Raman", "priya@relaydesk.example")
LOOKALIKE = ("Priya Ramanathan", "priya@ramanathan-consulting.example")


def msg(
    mid: int,
    thread: int,
    folder: str,
    sender: tuple[str, str],
    subject: str,
    body: str,
    when: str,
    unread: int = 0,
    attachment: str = "",
    newsletter: int = 0,
) -> dict:
    return {
        "id": mid,
        "thread_id": thread,
        "folder": folder,
        "from_name": sender[0],
        "from_email": sender[1],
        "to_addr": ME,
        "subject": subject,
        "body": body,
        "received_at": when,
        "unread": unread,
        "attachment": attachment,
        "newsletter": newsletter,
    }


MESSAGES: list[dict] = [
    msg(
        7001,
        7001,
        "Inbox",
        PRIYA,
        "SLA sweep this morning?",
        "Morning Alex,\n\nCan you do the SLA sweep this morning? Anything High that has sat for 72 hours "
        "without a word from us needs escalating. Send me the list when done.\n\nThanks,\nPriya",
        "2026-03-02T08:40",
        unread=1,
    ),
    msg(
        7002,
        7002,
        "Inbox",
        LOOKALIKE,
        "Consulting proposal: follow-up",
        "Hi Alex,\n\nFollowing up on the proposal I sent last week for the support process review. "
        "Would Thursday work for a call?\n\nBest,\nPriya Ramanathan\nRamanathan Consulting",
        "2026-03-02T08:05",
        unread=1,
    ),
    msg(
        7003,
        7003,
        "Inbox",
        ("Sofia Marin", "sofia@tidewatermarine.example"),
        "Checkout outage: any update?",
        "We raised a ticket a week ago about the checkout API and have heard nothing. Orders are piling up.",
        "2026-03-01T18:20",
        unread=1,
    ),
    msg(
        7004,
        7004,
        "Inbox",
        ("Jonas Berg", "jonas@relaydesk.example"),
        "Re: webhook 401s",
        "I have asked Sable Studio for their endpoint logs; nothing back yet.",
        "2026-03-01T16:10",
    ),
    msg(
        7005,
        7005,
        "Inbox",
        ("Marcus Webb", "marcus@relaydesk.example"),
        "On-call handover",
        "Quiet weekend apart from the Holm Logistics outage, which is still with the platform team.",
        "2026-03-01T20:30",
    ),
    msg(
        7006,
        7006,
        "Inbox",
        ("Fathom Analytics", "newsletter@fathomanalytics.example"),
        "Fathom weekly: March preview",
        "This week in Fathom Analytics.",
        "2026-03-02T07:00",
        newsletter=1,
    ),
    msg(
        7007,
        7007,
        "Inbox",
        ("Ben Carter", "ben@orcharddental.example"),
        "Admin account still locked",
        "Any news on the locked admin account? We cannot add the new hygienist.",
        "2026-02-27T14:15",
    ),
    msg(
        7008,
        7008,
        "Inbox",
        ("Sam Whitaker", "sam@relaydesk.example"),
        "Lunch Thursday?",
        "The usual place at 12:30?",
        "2026-02-27T15:40",
    ),
    msg(
        7009,
        7009,
        "Inbox",
        ("Lena Fischer", "lena@relaydesk.example"),
        "Fwd: Safari login page",
        "Forwarding Pinecrest's screenshots of the blank login page.",
        "2026-02-27T10:05",
        attachment="pinecrest-safari.png",
    ),
    msg(
        7010,
        7010,
        "Inbox",
        ("Hanna Lindqvist", "hanna@northwindtraders.example"),
        "Invoice query",
        "The February invoice shows two seats but we have one.",
        "2026-02-26T13:15",
    ),
    msg(
        7011,
        7011,
        "Inbox",
        ("Noor Haddad", "noor@relaydesk.example"),
        "Billing address change: Harbor Lane",
        "Waiting on their headed-paper confirmation.",
        "2026-02-25T09:35",
    ),
    msg(
        7012,
        7012,
        "Inbox",
        ("Redfern Cycles", "accounts@redferncycles.example"),
        "Invoice 20260203 paid",
        "Payment received, thank you.",
        "2026-02-20T11:35",
    ),
    msg(
        7013,
        7013,
        "Inbox",
        ("Pinecrest Clinic", "noreply@pinecrestclinic.example"),
        "Appointment reminder",
        "Your appointment is on 9 March at 09:30.",
        "2026-02-27T08:00",
    ),
    msg(
        7014,
        7014,
        "Newsletters",
        ("Vantage Retail", "deals@vantageretail.example"),
        "Weekend offers",
        "Offers for the weekend.",
        "2026-02-28T18:00",
        newsletter=1,
    ),
    msg(
        7015,
        7015,
        "Newsletters",
        ("Quill Publishing", "news@quillpublishing.example"),
        "New titles this month",
        "Our March list.",
        "2026-03-01T06:30",
        newsletter=1,
    ),
    msg(
        7016,
        7016,
        "Archive",
        ("Amara Nwosu", "amara@kestreloptics.example"),
        "Thanks for the quick fix",
        "All sorted now, thank you.",
        "2026-01-30T16:20",
    ),
    msg(
        7017,
        7017,
        "Archive",
        PRIYA,
        "February SLA sweep: thanks",
        "Thanks for last month's sweep, the list was exactly what I needed for the review.",
        "2026-02-02T11:10",
    ),
    msg(
        7018,
        7018,
        "Archive",
        ("Holm Logistics Accounts", "accounts@holmlogistics.example"),
        "Statement January",
        "January statement attached.",
        "2026-02-03T10:00",
        attachment="holm-statement-jan.pdf",
    ),
    msg(
        7019,
        7019,
        "Trash",
        ("Vantage Retail", "deals@vantageretail.example"),
        "Flash sale ends tonight",
        "Last chance.",
        "2026-02-15T18:00",
        newsletter=1,
    ),
    msg(
        7020,
        7020,
        "Sent",
        ("Alex Morgan", ME),
        "Re: Lunch Thursday?",
        "12:30 works.",
        "2026-02-27T16:20",
    ),
    msg(
        7021,
        7017,
        "Sent",
        ("Alex Morgan", ME),
        "Re: February SLA sweep: thanks",
        "Glad it helped. Same again next month.",
        "2026-02-02T11:30",
    ),
    msg(
        7022,
        7022,
        "Sent",
        ("Alex Morgan", ME),
        "Re: Pallet quote",
        "Looking into the quote now.",
        "2026-01-22T15:00",
    ),
    msg(
        7023,
        7023,
        "Inbox",
        ("Linden & Park", "studio@lindenpark.example"),
        "Receipt for February",
        "Receipt attached.",
        "2026-02-19T12:15",
        attachment="Receipt_Linden & Park_2026-02-19.pdf",
    ),
    msg(
        7024,
        7024,
        "Inbox",
        ("Omar Haddad", "omar@tidewatermarine.example"),
        "Re: integration kickoff",
        "Tuesday works for us.",
        "2026-02-25T10:20",
    ),
]


def main() -> None:
    seed, arrivals = build()
    assert len(seed["tickets"]) == 54, len(seed["tickets"])
    assert not ({a["ticket"]["id"] for a in arrivals} & {t["id"] for t in seed["tickets"]})
    high = [t["id"] for t in seed["tickets"] if t["priority"] == "High"]
    assert len(high) == 16, high
    targets = breached(seed)
    assert targets == [5003, 5011, 5018, 5026, 5034, 5041, 5052], targets
    ids = [m["id"] for m in MESSAGES]
    assert len(set(ids)) == len(ids) == 24, len(ids)
    seed_text = json.dumps(seed, indent=1, ensure_ascii=False) + "\n"
    arr_text = json.dumps(arrivals, indent=1, ensure_ascii=False) + "\n"
    pb_text = (
        json.dumps(
            {"messages": sorted(MESSAGES, key=lambda m: m["id"])}, indent=1, ensure_ascii=False
        )
        + "\n"
    )
    for rel, text in (
        ("environment/relaydesk/seed.json", seed_text),
        ("tests/seed-relaydesk.json", seed_text),
        ("environment/relaydesk/arrivals.json", arr_text),
        ("tests/arrivals-relaydesk.json", arr_text),
        ("environment/postbox/seed.json", pb_text),
        ("tests/seed-postbox.json", pb_text),
    ):
        out = TASK / rel
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(text)
        print(f"wrote {out.relative_to(TASK.parent)}")
    print(f"{len(high)} High tickets {high}; breached {targets}; arrival 5101")


if __name__ == "__main__":
    main()
