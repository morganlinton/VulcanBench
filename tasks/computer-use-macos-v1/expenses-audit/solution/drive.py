"""expenses-audit gold: Terminal writes the reconciled report as text (sent
as base64, so tabs and apostrophes survive typing), converts it to rich text
with textutil over the original, and files the four personal receipts. The
gold knows the receipts; an agent has to read all 41 and the policy.

Cheat variants: VB_SPLIT=full claims the Harbor Hotel bill in full;
VB_FX=face claims the euro receipts at face value; VB_GIFT=full claims the
gift at 95.00; VB_DUP=1 lists Bistro Nine twice; VB_UNBACKED=1 keeps the
City Cab line; VB_NOMOVE=1 leaves the personal receipts in Receipts.
"""

import base64
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import goldlib as G  # noqa: E402

ROWS = [  # (date, description, amount): the 36 claims in date order
    ("02 Feb 2026", "Taxi, home to Logan airport (Logan Taxi Co.)", 46.20),
    ("02 Feb 2026", "Checked bag fee, Boston to Chicago (Skyline Air)", 35.00),
    ("02 Feb 2026", "Coffee, O'Hare airport (O'Hare Coffee)", 6.40),
    ("03 Feb 2026", "Client dinner, Northwind, wine excluded (Lakeside Grill)", 128.45),
    ("03 Feb 2026", "Taxi to Northwind office (Metro Cab)", 22.10),
    ("04 Feb 2026", "Printing, pitch decks (Copy Corner)", 18.75),
    ("04 Feb 2026", "Lunch, Chicago trip (Deep Dish Co.)", 17.85),
    ("05 Feb 2026", "Hotel, 2 nights, Chicago, room only (Harbor Hotel Chicago)", 412.80),
    ("05 Feb 2026", "Flight change fee (Skyline Air)", 75.00),
    ("06 Feb 2026", "Office supplies (Paper Trail Supplies)", 54.99),
    ("09 Feb 2026", "Parking, client visit (Garage 7)", 31.00),
    ("10 Feb 2026", "Lunch with candidate (Bistro Nine)", 64.30),
    ("10 Feb 2026", "Note-taking software, annual (Cloudnote)", 59.99),
    ("11 Feb 2026", "Conference registration (DevSummit)", 299.00),
    ("12 Feb 2026", "Train, Boston to New York (Northeast Rail)", 89.00),
    ("12 Feb 2026", "Hotel, 1 night, New York (Union Square Inn)", 219.60),
    ("13 Feb 2026", "Team breakfast, New York office (Daybreak Diner)", 87.25),
    ("13 Feb 2026", "Taxi to the New York office (Yellow Line Taxi)", 27.80),
    ("13 Feb 2026", "Juice, New York trip (Pressed Juice NYC)", 8.50),
    ("14 Feb 2026", "Train, New York to Boston (Northeast Rail)", 89.00),
    ("16 Feb 2026", "Diagram software, monthly (Diagram Pro)", 12.00),
    ("17 Feb 2026", "Courier, contracts to Cobalt (FastLane Couriers)", 41.60),
    ("17 Feb 2026", "Parking, client visit (Garage 7)", 31.00),
    ("18 Feb 2026", "Printing, workshop handouts (Kinko Print Hub)", 36.40),
    ("18 Feb 2026", "Client gift, flowers for Cobalt launch, capped (Bloom & Stem)", 75.00),
    ("19 Feb 2026", "Flight, Boston to Lisbon (Skyline Air)", 642.00),
    ("20 Feb 2026", "Taxi, Lisbon airport to hotel (Taxis Lisboa)", 30.99),
    ("20 Feb 2026", "Hotel, 3 nights, Lisbon (Casa Alfama)", 430.61),
    ("21 Feb 2026", "Client dinner, Lusitania Bank (Tasca do Bairro)", 154.41),
    ("23 Feb 2026", "Taxi, hotel to Lisbon airport (Taxis Lisboa)", 29.36),
    ("23 Feb 2026", "Checked bag fee, Lisbon to Boston (Skyline Air)", 35.00),
    ("24 Feb 2026", "Taxi, Logan airport to home (Logan Taxi Co.)", 48.60),
    ("25 Feb 2026", "Team lunch (Noodle House)", 112.40),
    ("26 Feb 2026", "Printer toner (Paper Trail Supplies)", 89.99),
    ("26 Feb 2026", "Courier, signed NDA to Brightwave (FastLane Couriers)", 23.80),
    ("27 Feb 2026", "Parking, client visit (Garage 7)", 31.00),
]
FACE = {30.99: 28.50, 430.61: 396.00, 154.41: 142.00, 29.36: 27.00}
rows = list(ROWS)
if os.environ.get("VB_SPLIT") == "full":
    rows = [(d, t, 451.79 if a == 412.80 else a) for d, t, a in rows]
if os.environ.get("VB_FX") == "face":
    rows = [(d, t, FACE.get(a, a)) for d, t, a in rows]
if os.environ.get("VB_GIFT") == "full":
    rows = [(d, t, 95.00 if "Bloom" in t else a) for d, t, a in rows]
if os.environ.get("VB_DUP") == "1":
    i = next(i for i, r in enumerate(rows) if "Bistro Nine" in r[1])
    rows.insert(i + 1, rows[i])
if os.environ.get("VB_UNBACKED") == "1":
    i = next(i for i, r in enumerate(rows) if "Daybreak" in r[1])
    rows.insert(i + 1, ("13 Feb 2026", "Taxi, airport (City Cab)", 38.00))
total = round(sum(a for _, _, a in rows), 2)
text = "February 2026 expenses, Jordan Lee\n\nDate\tDescription\tAmount (USD)\n"
text += "".join(f"{d}\t{desc}\t{a:,.2f}\n" for d, desc, a in rows)
text += f"\nTotal\t\t{total:,.2f}\n"
b64 = base64.b64encode(text.encode()).decode()
EXP = "~/Documents/Expenses"
PERSONAL = ["2026-02-08 StreamBox.pdf", "2026-02-14 Rosewood.pdf", "2026-02-15 Peak Fitness.pdf", "2026-02-22 Sweet Layers.pdf"]

G.start()
G.open_terminal()
G.run(f"cd {EXP} && ls", wait=3)
G.allow_folder("documents-prompt")
# Long base64 lines are typed in pieces: one very long type_text has dropped
# keys before (a doubled digit in calendar-workshop-update).
G.run(": > /tmp/r.b64", wait=1)
for k in range(0, len(b64), 400):
    G.run(f"printf %s {b64[k:k + 400]} >> /tmp/r.b64", wait=1)
G.run("base64 -D -i /tmp/r.b64 -o /tmp/report.txt && shasum /tmp/report.txt && wc -l /tmp/report.txt", wait=3)
G.run('textutil -convert rtf -font Helvetica -fontsize 12 -output "February expense report.rtf" /tmp/report.txt && echo converted', wait=4)
if os.environ.get("VB_NOMOVE") != "1":
    names = " ".join(f'"Receipts/{p}"' for p in PERSONAL)
    G.run(f"mkdir -p Personal && mv {names} Personal/ && ls Personal", wait=3)
G.shot("final")
print("gold done")
