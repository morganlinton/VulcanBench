"""expenses-reconcile gold: Terminal writes the reconciled report as text
(sent as base64, so tabs and apostrophes survive typing), converts it to rich
text with textutil over the original, and files the two personal receipts.
The gold knows the receipts; an agent has to read them and the policy.

Cheat variants: VB_NOFIX=1 leaves the Lakeside Grill amount wrong;
VB_KEEP_PERSONAL=1 keeps the Rosewood dinner; VB_APPEND=1 adds the three
missing receipts at the end; VB_TOTAL=old keeps the old total; VB_NOMOVE=1
leaves the personal receipts in Receipts.
"""

import base64
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import goldlib as G  # noqa: E402

ROWS = [  # (date, description, amount), the business receipts in date order
    ("02 Feb 2026", "Taxi, home to Logan airport (Logan Taxi Co.)", 46.20),
    ("02 Feb 2026", "Checked bag fee, Boston to Chicago (Skyline Air)", 35.00),
    ("03 Feb 2026", "Hotel, 2 nights, Chicago (Harbor Hotel Chicago)", 412.80),
    ("03 Feb 2026", "Client dinner, Northwind (Lakeside Grill)", 128.45),
    ("04 Feb 2026", "Taxi to Northwind office (Metro Cab)", 22.10),
    ("04 Feb 2026", "Printing, pitch decks (Copy Corner)", 18.75),
    ("05 Feb 2026", "Flight change fee (Skyline Air)", 75.00),
    ("05 Feb 2026", "Coffee, O'Hare airport (O'Hare Coffee)", 6.40),
    ("09 Feb 2026", "Office supplies (Paper Trail Supplies)", 54.99),
    ("10 Feb 2026", "Parking, client visit (Garage 7)", 31.00),
    ("11 Feb 2026", "Lunch with candidate (Bistro Nine)", 64.30),
    ("12 Feb 2026", "Conference registration (DevSummit)", 299.00),
    ("16 Feb 2026", "Train, Boston to New York (Northeast Rail)", 89.00),
    ("16 Feb 2026", "Hotel, 1 night, New York (Union Square Inn)", 219.60),
    ("17 Feb 2026", "Team breakfast, New York office (Daybreak Diner)", 87.25),
    ("17 Feb 2026", "Taxi to the New York office (Yellow Line Taxi)", 27.80),
    ("18 Feb 2026", "Train, New York to Boston (Northeast Rail)", 89.00),
    ("19 Feb 2026", "Diagram software, monthly (Diagram Pro)", 12.00),
    ("20 Feb 2026", "Courier, contracts to Cobalt (FastLane Couriers)", 41.60),
    ("23 Feb 2026", "Client gift, flowers for Cobalt launch (Bloom & Stem)", 58.00),
    ("24 Feb 2026", "Parking, client visit (Garage 7)", 31.00),
    ("25 Feb 2026", "Team lunch (Noodle House)", 112.40),
]
rows = list(ROWS)
if os.environ.get("VB_NOFIX") == "1":
    rows[3] = (rows[3][0], rows[3][1], 182.45)
if os.environ.get("VB_KEEP_PERSONAL") == "1":
    rows.insert(12, ("14 Feb 2026", "Dinner (Rosewood Restaurant)", 186.50))
if os.environ.get("VB_APPEND") == "1":
    late = [r for r in rows if "O'Hare Coffee" in r[1] or "Daybreak" in r[1] or "Bloom" in r[1]]
    rows = [r for r in rows if r not in late] + late
total = 2054.63 if os.environ.get("VB_TOTAL") == "old" else round(sum(a for _, _, a in rows), 2)
text = "February 2026 expenses, Jordan Lee\n\nDate\tDescription\tAmount (USD)\n"
text += "".join(f"{d}\t{desc}\t{a:,.2f}\n" for d, desc, a in rows)
text += f"\nTotal\t\t{total:,.2f}\n"
b64 = base64.b64encode(text.encode()).decode()
EXP = "~/Documents/Expenses"

G.start()
G.open_terminal()
G.run(f"cd {EXP} && ls", wait=3)
G.allow_folder("documents-prompt")
G.run(f"echo {b64} | base64 -D > /tmp/report.txt && wc -l /tmp/report.txt", wait=4)
G.run('textutil -convert rtf -font Helvetica -fontsize 12 -output "February expense report.rtf" /tmp/report.txt && echo converted', wait=4)
if os.environ.get("VB_NOMOVE") != "1":
    G.run('mkdir -p Personal && mv "Receipts/2026-02-14 Rosewood.pdf" "Receipts/2026-02-21 Peak Fitness.pdf" Personal/ && ls Personal', wait=3)
G.shot("final")
print("gold done")
