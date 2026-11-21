"""invoices-to-reminders gold: Terminal asks Reminders (AppleScript) for a
Bills list and one all-day reminder per unpaid invoice, with the amount in
the notes. The gold knows the invoices; an agent has to read them.

Cheat variants: VB_EU=us reads 16.03.2026 month first (an impossible month,
so it uses the invoice date instead: 2 March); VB_PAID=1 also adds the paid
Northwind invoice; VB_NET=from-today counts Net 15 from today (24 March).
"""

import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import goldlib as G  # noqa: E402

bright = (2026, 3, 24) if os.environ.get("VB_NET") == "from-today" else (2026, 3, 14)
lumiere = (2026, 3, 2) if os.environ.get("VB_EU") == "us" else (2026, 3, 16)
BILLS = [
    ("Pay Brightwave Printing", "USD 1,240.00 (INV-2026-0412)", bright),
    ("Pay Cobalt Cloud Hosting", "USD 389.50 (CC-88120)", (2026, 3, 31)),
    ("Pay Atelier Lumiere", "EUR 2,450.00 (F-2026-118)", lumiere),
    ("Pay Greenleaf Landscaping", "USD 860.00 (2026-031)", (2026, 3, 22)),
]
if os.environ.get("VB_PAID") == "1":
    BILLS.append(("Pay Northwind Office Supply", "USD 498.00", (2026, 3, 6)))

G.start()
G.open_terminal()
G.authorize("Reminders", data_prompt=True)
G.run("osascript -e 'tell application \"Reminders\" to make new list with properties {name:\"Bills\"}'", wait=4)
for title, notes, (y, mo, d) in BILLS:
    when = G.as_date(y, mo, d, 9, 0)
    G.run(
        f"osascript -e 'tell application \"Reminders\" to make new reminder at end of list \"Bills\" "
        f"with properties {{name:\"{title}\", body:\"{notes}\", allday due date:{when}}}'",
        wait=2.5,
    )
G.run("osascript -e 'tell application \"Reminders\" to get name of reminders of list \"Bills\"'", wait=3)
G.shot("final")
print("gold done")
