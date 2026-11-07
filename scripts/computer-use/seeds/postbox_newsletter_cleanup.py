#!/usr/bin/env python3
"""Seed for tasks/computer-use-v1/postbox-newsletter-cleanup (deterministic).

    python scripts/computer-use/seeds/postbox_newsletter_cleanup.py

Writes environment/postbox/seed.json and tests/seed-postbox.json. Forty-one
messages. Fourteen are from Fathom Analytics: nine newsletters received
before February (the targets, spread over Inbox and the Newsletters folder,
which runs to two pages), four newsletters from February and March (keep),
and one invoice email with an attachment (keep). Three more newsletters come
from the look-alike sender Fathom Insights (keep).
"""

from __future__ import annotations

import json
from pathlib import Path

TASK = (
    Path(__file__).resolve().parents[3] / "tasks" / "computer-use-v1" / "postbox-newsletter-cleanup"
)
ME = "you@relaydesk.example"
FATHOM = ("Fathom Analytics", "news@fathomanalytics.example")
FATHOM_BILLING = ("Fathom Analytics", "billing@fathomanalytics.example")
INSIGHTS = ("Fathom Insights", "digest@fathom-insights.example")

TARGET_IDS = (3002, 3005, 3009, 3012, 3016, 3019, 3023, 3026, 3030)


def nl(
    mid: int,
    thread: int,
    folder: str,
    sender: tuple[str, str],
    subject: str,
    body: str,
    when: str,
    unread: int = 0,
    attachment: str = "",
    newsletter: int = 1,
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


def fathom(mid: int, folder: str, title: str, when: str, unread: int = 0) -> dict:
    body = (
        f"{title}\n\nThis week in Fathom Analytics: tips on reading your referrer report, a new "
        "events dashboard, and a reminder that annual plans renew in March.\n\n"
        "Read the full issue on our site."
    )
    return nl(mid, mid, folder, FATHOM, title, body, when, unread)


MESSAGES: list[dict] = [
    # ---- Fathom Analytics newsletters before February: targets
    fathom(3002, "Inbox", "Fathom weekly: referrers you did not know you had", "2026-01-28T07:30"),
    fathom(3005, "Newsletters", "Fathom weekly: the events dashboard is live", "2026-01-21T07:30"),
    fathom(3009, "Newsletters", "Fathom weekly: year in review", "2026-01-14T07:30"),
    fathom(3012, "Inbox", "Fathom weekly: January roadmap", "2026-01-07T07:30", unread=1),
    fathom(3016, "Newsletters", "Fathom weekly: holiday traffic patterns", "2025-12-31T07:30"),
    fathom(3019, "Newsletters", "Fathom weekly: what changed in December", "2025-12-17T07:30"),
    fathom(
        3023, "Newsletters", "Fathom weekly: cookie banners and bounce rate", "2025-12-03T07:30"
    ),
    fathom(3026, "Inbox", "Fathom weekly: Black Friday numbers", "2025-11-26T07:30"),
    fathom(3030, "Newsletters", "Fathom weekly: UTM hygiene", "2025-11-12T07:30"),
    # ---- Fathom Analytics newsletters from February on: keep
    fathom(3001, "Inbox", "Fathom weekly: March preview", "2026-03-01T07:30", unread=1),
    fathom(3003, "Newsletters", "Fathom weekly: campaign tracking refresh", "2026-02-25T07:30"),
    fathom(3006, "Newsletters", "Fathom weekly: sharing dashboards", "2026-02-11T07:30"),
    fathom(3008, "Newsletters", "Fathom weekly: the February release", "2026-02-04T07:30"),
    # ---- Fathom Analytics invoice, January: keep
    nl(
        3014,
        3014,
        "Inbox",
        FATHOM_BILLING,
        "Your Fathom Analytics invoice for January",
        "Hi,\n\nYour invoice INV-7731 for the Business plan (January 2026) is attached. The amount of 140.00 USD "
        "was charged to the card ending 4471.\n\nThanks,\nFathom Analytics billing",
        "2026-01-09T10:05",
        attachment="fathom-INV-7731.pdf",
        newsletter=0,
    ),
    # ---- Fathom Insights, a different sender: keep
    nl(
        3010,
        3010,
        "Newsletters",
        INSIGHTS,
        "Fathom Insights digest: January",
        "Analytics stories from around the web.\n\nThis month: cohort retention done right.",
        "2026-01-13T08:00",
    ),
    nl(
        3021,
        3021,
        "Newsletters",
        INSIGHTS,
        "Fathom Insights digest: December",
        "Analytics stories from around the web.\n\nThis month: attribution without cookies.",
        "2025-12-10T08:00",
    ),
    nl(
        3031,
        3031,
        "Newsletters",
        INSIGHTS,
        "Fathom Insights digest: November",
        "Analytics stories from around the web.\n\nThis month: dashboards nobody reads.",
        "2025-11-11T08:00",
    ),
    # ---- other newsletters in the Newsletters folder: keep
    nl(
        3004,
        3004,
        "Newsletters",
        ("Linden & Park", "studio@lindenpark.example"),
        "New arrivals for spring",
        "Our spring collection is here.",
        "2026-02-20T09:00",
    ),
    nl(
        3015,
        3015,
        "Newsletters",
        ("Quill Publishing", "letters@quillpublishing.example"),
        "Quill monthly: five books for January",
        "Our editors' picks.",
        "2026-01-08T09:00",
    ),
    nl(
        3027,
        3027,
        "Newsletters",
        ("Redfern Cycles", "news@redferncycles.example"),
        "Winter service offer",
        "Book a service before 15 December and save 20 percent.",
        "2025-11-24T09:00",
    ),
    # ---- regular mail
    nl(
        3000,
        3000,
        "Inbox",
        ("Priya Raman", "priya@relaydesk.example"),
        "Northwind escalation: status?",
        "Hi,\n\nDid the Northwind tickets get moved yesterday? Finance is asking.\n\nPriya",
        "2026-03-02T08:40",
        unread=1,
        newsletter=0,
    ),
    nl(
        3007,
        3007,
        "Inbox",
        ("Dana Holm", "dana@holmlogistics.example"),
        "Delivery window for 12 March",
        "Hello,\n\nCan we confirm Thursday 12 March between 08:00 and 10:00 for the Leith depot delivery?\n\nDana",
        "2026-02-27T15:10",
        unread=1,
        newsletter=0,
    ),
    nl(
        3011,
        3011,
        "Inbox",
        ("Jonas Berg", "jonas@relaydesk.example"),
        "Re: statement export",
        "Customer sent the statement, comparing now.",
        "2026-02-28T17:30",
        newsletter=0,
    ),
    nl(
        3013,
        3013,
        "Inbox",
        ("Chloe Bernard", "chloe@fathomanalytics.example"),
        "Re: API question",
        "Hi,\n\nYes, the events endpoint accepts batches of up to 500. Let me know if you hit limits.\n\nChloe (Fathom Analytics support)",
        "2026-02-18T11:20",
        newsletter=0,
    ),
    nl(
        3017,
        3017,
        "Inbox",
        ("Noor Haddad", "noor@relaydesk.example"),
        "Lunch Thursday?",
        "The usual place at 12:30?",
        "2026-02-26T10:00",
        newsletter=0,
    ),
    nl(
        3018,
        3018,
        "Inbox",
        ("Yusuf Demir", "yusuf@brightwaterfoods.example"),
        "Updated price list",
        "Please find the March price list attached.",
        "2026-02-24T14:45",
        attachment="brightwater-prices-march.pdf",
        newsletter=0,
    ),
    nl(
        3020,
        3020,
        "Inbox",
        ("Sam Whitaker", "sam@relaydesk.example"),
        "Fwd: PDF export bug",
        "Forwarding the Northwind thread for context.",
        "2026-02-20T13:35",
        newsletter=0,
    ),
    nl(
        3022,
        3022,
        "Inbox",
        ("Grace Mbeki", "grace@orcharddental.example"),
        "Appointment reminder",
        "Your appointment is on 9 March at 09:30.",
        "2026-02-23T09:00",
        newsletter=0,
    ),
    nl(
        3024,
        3024,
        "Archive",
        ("Lena Fischer", "lena@relaydesk.example"),
        "Credit notes in the export",
        "Logged as #2021.",
        "2026-02-17T15:10",
        newsletter=0,
    ),
    nl(
        3025,
        3025,
        "Archive",
        ("Daniel Okafor", "daniel@relaydesk.example"),
        "Address change form",
        "Sent the template to Northwind.",
        "2026-02-23T11:35",
        newsletter=0,
    ),
    nl(
        3028,
        3028,
        "Archive",
        ("Amara Nwosu", "amara@kestreloptics.example"),
        "Thanks for the quick fix",
        "All sorted now, thank you.",
        "2026-01-30T16:20",
        newsletter=0,
    ),
    nl(
        3029,
        3029,
        "Archive",
        ("Fathom Analytics", "news@fathomanalytics.example"),
        "Fathom weekly: October recap",
        "Already filed last year.",
        "2025-10-29T07:30",
    ),
    nl(
        3032,
        3032,
        "Trash",
        ("Vantage Retail", "deals@vantageretail.example"),
        "Flash sale ends tonight",
        "Last chance.",
        "2026-02-15T18:00",
    ),
    nl(
        3033,
        3033,
        "Trash",
        ("Pinecrest Clinic", "noreply@pinecrestclinic.example"),
        "Survey: how did we do?",
        "Two minutes of your time.",
        "2026-02-10T12:00",
        newsletter=0,
    ),
    nl(
        3034,
        3034,
        "Inbox",
        ("Olivia Park", "olivia@lindenpark.example"),
        "Invoice query",
        "Hi, the February invoice shows two seats but we have one.",
        "2026-02-19T10:10",
        newsletter=0,
    ),
    nl(
        3035,
        3035,
        "Inbox",
        ("Mateo Ruiz", "mateo@sablestudio.example"),
        "Logo files",
        "Final logo files attached as agreed.",
        "2026-02-12T16:00",
        attachment="sable-logo-final.zip",
        newsletter=0,
    ),
    nl(
        3036,
        3036,
        "Inbox",
        ("Ben Okoro", "ben@vantageretail.example"),
        "Re: integration kickoff",
        "Tuesday works for us.",
        "2026-02-06T09:45",
        newsletter=0,
    ),
    nl(
        3037,
        3037,
        "Inbox",
        ("Tomas Reyes", "tomas@northwindtraders.example"),
        "Re: invoice export timeout",
        "Still happening this morning.",
        "2026-02-21T08:15",
        unread=1,
        newsletter=0,
    ),
    nl(
        3038,
        3038,
        "Inbox",
        ("Aisha Rahman", "aisha@pinecrestclinic.example"),
        "Parking at the clinic",
        "Use the rear entrance during the works.",
        "2026-01-27T11:00",
        newsletter=0,
    ),
    nl(
        3039,
        3039,
        "Inbox",
        ("Sean Gallagher", "sean@tidewatermarine.example"),
        "Pallet quote",
        "The quote came back at a parcel rate, can you check?",
        "2026-01-22T13:30",
        newsletter=0,
    ),
    nl(
        3040,
        3040,
        "Sent",
        ("Alex Morgan", ME),
        "Re: Delivery window for 12 March",
        "Checking with the depot, back to you Monday.",
        "2026-02-27T16:40",
        newsletter=0,
    ),
]


def main() -> None:
    assert len({m["id"] for m in MESSAGES}) == len(MESSAGES) == 41, len(MESSAGES)
    targets = [m for m in MESSAGES if m["id"] in TARGET_IDS]
    assert all(
        m["from_email"] == FATHOM[1] and m["received_at"] < "2026-02-01" and m["newsletter"]
        for m in targets
    )
    assert len(targets) == 9
    seed = {"messages": sorted(MESSAGES, key=lambda m: m["id"])}
    text = json.dumps(seed, indent=1, ensure_ascii=False) + "\n"
    for rel in ("environment/postbox/seed.json", "tests/seed-postbox.json"):
        out = TASK / rel
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(text)
        print(f"wrote {out.relative_to(TASK.parent)}")
    folders = {}
    for m in MESSAGES:
        folders[m["folder"]] = folders.get(m["folder"], 0) + 1
    print("per folder:", folders)


if __name__ == "__main__":
    main()
