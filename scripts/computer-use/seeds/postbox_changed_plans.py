#!/usr/bin/env python3
"""Seed for tasks/computer-use-v1/postbox-changed-plans (deterministic).

    python scripts/computer-use/seeds/postbox_changed_plans.py

Writes environment/postbox/seed.json, environment/postbox/arrivals.json and
their copies under tests/. Twenty-eight messages. Dana Holm (Holm Logistics)
is rescheduling the Leith depot delivery; her latest seeded mail proposes
Thursday 12 March 08:00 to 10:00. Two seconds after the agent clicks Reply
on that conversation a new mail from her arrives replacing the slot with
Wednesday 11 March 13:00 to 15:00. An unrelated newsletter arrives ten
seconds after the agent's first page view, so "new mail" is not by itself
the signal. A look-alike sender (Dana Holmberg, Holmberg Freight) asks a
similar question.
"""

from __future__ import annotations

import json
from pathlib import Path

TASK = Path(__file__).resolve().parents[3] / "tasks" / "computer-use-v1" / "postbox-changed-plans"
ME = "you@relaydesk.example"
DANA = ("Dana Holm", "dana@holmlogistics.example")
THREAD = 4200
OLD_SLOT = "Thursday 12 March between 08:00 and 10:00"
NEW_SLOT = "Wednesday 11 March between 13:00 and 15:00"


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
    # ---- the Holm Logistics thread
    msg(
        4201,
        THREAD,
        "Inbox",
        DANA,
        "Leith depot delivery: new slot needed",
        "Hi Alex,\n\nThe depot can no longer take the delivery on Tuesday 3 March; their yard is closed for "
        "resurfacing that week. Can we move it? I will come back to you with the options they give me.\n\n"
        "Best,\nDana Holm\nHolm Logistics",
        "2026-02-24T11:05",
    ),
    msg(
        4202,
        THREAD,
        "Sent",
        ("Alex Morgan", ME),
        "Re: Leith depot delivery: new slot needed",
        "No problem, Dana. Send me the options and I will confirm one.\n\nAlex",
        "2026-02-24T14:30",
    ),
    msg(
        4203,
        THREAD,
        "Inbox",
        DANA,
        "Re: Leith depot delivery: new slot needed",
        f"Hi Alex,\n\nThe depot has offered {OLD_SLOT}, same driver as before. Please reply to confirm so I "
        "can book it with them today.\n\nThanks,\nDana Holm\nHolm Logistics",
        "2026-03-02T08:10",
        unread=1,
    ),
    # ---- look-alike sender
    msg(
        4210,
        4210,
        "Inbox",
        ("Dana Holmberg", "dana@holmbergfreight.example"),
        "Delivery slot next week?",
        "Hi,\n\nCan you do Wednesday 11 March, 09:00 to 11:00, for the pallet collection?\n\nDana Holmberg\n"
        "Holmberg Freight",
        "2026-03-01T17:45",
        unread=1,
    ),
    # ---- the rest of the mailbox
    msg(
        4211,
        4211,
        "Inbox",
        ("Priya Raman", "priya@relaydesk.example"),
        "Northwind escalation: status?",
        "Did the Northwind tickets get moved yesterday? Finance is asking.",
        "2026-03-02T08:35",
        unread=1,
    ),
    msg(
        4212,
        4212,
        "Inbox",
        ("Jonas Berg", "jonas@relaydesk.example"),
        "Re: statement export",
        "Customer sent the statement, comparing now.",
        "2026-03-01T16:10",
    ),
    msg(
        4213,
        4213,
        "Inbox",
        ("Brightwater Foods", "orders@brightwaterfoods.example"),
        "Updated price list",
        "Please find the March price list attached.",
        "2026-03-01T09:00",
        attachment="brightwater-prices-march.pdf",
    ),
    msg(
        4214,
        4214,
        "Inbox",
        ("Sam Whitaker", "sam@relaydesk.example"),
        "Lunch Thursday?",
        "The usual place at 12:30?",
        "2026-02-27T15:40",
    ),
    msg(
        4215,
        4215,
        "Inbox",
        ("Pinecrest Clinic", "noreply@pinecrestclinic.example"),
        "Appointment reminder",
        "Your appointment is on 9 March at 09:30.",
        "2026-02-27T08:00",
    ),
    msg(
        4216,
        4216,
        "Inbox",
        ("Hanna Lindqvist", "hanna@northwindtraders.example"),
        "Invoice query",
        "The February invoice shows two seats but we have one.",
        "2026-02-26T13:15",
    ),
    msg(
        4217,
        4217,
        "Inbox",
        ("Sable Studio", "hello@sablestudio.example"),
        "Logo files",
        "Final logo files attached as agreed.",
        "2026-02-25T17:05",
        attachment="relaydesk-logo-final.zip",
    ),
    msg(
        4218,
        4218,
        "Inbox",
        ("Omar Haddad", "omar@tidewatermarine.example"),
        "Re: integration kickoff",
        "Tuesday works for us.",
        "2026-02-25T10:20",
    ),
    msg(
        4219,
        4219,
        "Inbox",
        ("Tomas Reyes", "tomas@northwindtraders.example"),
        "Re: invoice export timeout",
        "Still happening this morning.",
        "2026-02-24T09:05",
    ),
    msg(
        4220,
        4220,
        "Inbox",
        ("Lena Fischer", "lena@relaydesk.example"),
        "Fwd: PDF export bug",
        "Forwarding the Northwind thread for context.",
        "2026-02-23T14:50",
    ),
    msg(
        4221,
        4221,
        "Inbox",
        ("Fathom Analytics", "newsletter@fathomanalytics.example"),
        "Fathom weekly: March preview",
        "This week in Fathom Analytics.",
        "2026-03-02T07:00",
        newsletter=1,
    ),
    msg(
        4222,
        4222,
        "Newsletters",
        ("Vantage Retail", "deals@vantageretail.example"),
        "Weekend offers",
        "Offers for the weekend.",
        "2026-02-28T18:00",
        newsletter=1,
    ),
    msg(
        4223,
        4223,
        "Newsletters",
        ("Fathom Analytics", "newsletter@fathomanalytics.example"),
        "Fathom weekly: February recap",
        "February in numbers.",
        "2026-02-23T07:00",
        newsletter=1,
    ),
    msg(
        4224,
        4224,
        "Newsletters",
        ("Quill Publishing", "news@quillpublishing.example"),
        "New titles this month",
        "Our March list.",
        "2026-03-01T06:30",
        newsletter=1,
    ),
    msg(
        4225,
        4225,
        "Archive",
        ("Amara Nwosu", "amara@kestreloptics.example"),
        "Thanks for the quick fix",
        "All sorted now, thank you.",
        "2026-01-30T16:20",
    ),
    msg(
        4226,
        4226,
        "Archive",
        ("Holm Logistics Accounts", "accounts@holmlogistics.example"),
        "Statement January",
        "January statement attached.",
        "2026-02-03T10:00",
        attachment="holm-statement-jan.pdf",
    ),
    msg(
        4227,
        4227,
        "Archive",
        DANA,
        "Leith depot: access instructions",
        "Access instructions for the Leith depot attached. Gate B, ask for the yard supervisor.",
        "2026-02-10T09:40",
        attachment="leith-depot-access.pdf",
    ),
    msg(
        4228,
        4228,
        "Trash",
        ("Vantage Retail", "deals@vantageretail.example"),
        "Flash sale ends tonight",
        "Last chance.",
        "2026-02-15T18:00",
        newsletter=1,
    ),
    msg(
        4229,
        4229,
        "Trash",
        ("Pinecrest Clinic", "noreply@pinecrestclinic.example"),
        "Survey: how did we do?",
        "Two minutes of your time.",
        "2026-02-10T12:00",
    ),
    msg(
        4230,
        4230,
        "Sent",
        ("Alex Morgan", ME),
        "Re: Pallet quote",
        "Looking into the quote now.",
        "2026-01-22T15:00",
    ),
    msg(
        4231,
        4231,
        "Sent",
        ("Alex Morgan", ME),
        "Re: Lunch Thursday?",
        "12:30 works.",
        "2026-02-27T16:20",
    ),
    msg(
        4232,
        4232,
        "Inbox",
        ("Redfern Cycles", "accounts@redferncycles.example"),
        "Invoice 20260203 paid",
        "Payment received, thank you.",
        "2026-02-20T11:35",
    ),
    msg(
        4233,
        4233,
        "Inbox",
        ("Noor Haddad", "noor@relaydesk.example"),
        "Billing address change: Harbor Lane",
        "Waiting on their headed-paper confirmation.",
        "2026-02-25T09:35",
    ),
    msg(
        4234,
        4234,
        "Inbox",
        ("Linden & Park", "studio@lindenpark.example"),
        "Receipt for February",
        "Receipt attached.",
        "2026-02-19T12:15",
        attachment="Receipt_Linden & Park_2026-02-19.pdf",
    ),
]

ARRIVALS: list[dict] = [
    {
        # unrelated: arrives 10 s after the agent's first look at anything
        "after_s": 10,
        "message": msg(
            4290,
            4290,
            "Inbox",
            ("Vantage Retail", "deals@vantageretail.example"),
            "Spring sale starts today",
            "Spring sale: up to 40% off this week only.",
            "2026-03-02T09:16",
            unread=1,
            newsletter=1,
        ),
    },
    {
        # the change of plans: 2 s after the agent clicks Reply on the conversation
        "after_s": 2,
        "trigger": {"event": "compose.open", "thread_id": THREAD},
        "message": msg(
            4204,
            THREAD,
            "Inbox",
            DANA,
            "Re: Leith depot delivery: new slot needed",
            "Alex, sorry, scrap Thursday: the depot has double-booked the morning. They can do "
            f"{NEW_SLOT} instead, same driver. Please confirm that one and I will book it.\n\n"
            "Dana",
            "2026-03-02T09:21",
            unread=1,
        ),
    },
]


def main() -> None:
    ids = [m["id"] for m in MESSAGES]
    assert len(set(ids)) == len(ids) == 28, len(ids)
    assert not ({a["message"]["id"] for a in ARRIVALS} & set(ids))
    seed_text = (
        json.dumps(
            {"messages": sorted(MESSAGES, key=lambda m: m["id"])}, indent=1, ensure_ascii=False
        )
        + "\n"
    )
    arr_text = json.dumps(ARRIVALS, indent=1, ensure_ascii=False) + "\n"
    for rel, text in (
        ("environment/postbox/seed.json", seed_text),
        ("tests/seed-postbox.json", seed_text),
        ("environment/postbox/arrivals.json", arr_text),
        ("tests/arrivals-postbox.json", arr_text),
    ):
        out = TASK / rel
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(text)
        print(f"wrote {out.relative_to(TASK.parent)}")


if __name__ == "__main__":
    main()
