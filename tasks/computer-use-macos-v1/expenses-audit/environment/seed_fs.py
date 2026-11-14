"""expenses-audit seed, run on the host against the task VM's Data volume
(build_task_image.py): python3 seed_fs.py <data-volume-root>.

~/Documents/Expenses holds "February expense report.rtf" (TextEdit rich
text, one line per expense: date, tab, description, tab, amount, then a
Total line), "Expense policy.pdf", and Receipts/ with 41 receipt PDFs:
36 business receipts (two of them mixed, four in euros), four personal ones
and a second scan of the Bistro Nine receipt.

The report as seeded (36 expense lines):
  - is missing five business receipts (O'Hare Coffee, Pressed Juice NYC,
    Kinko Print Hub, the 23 Feb Lisbon taxi, Paper Trail toner),
  - has six mistyped amounts (Union Square Inn 291.60 for 219.60, DevSummit
    290.00 for 299.00, Yellow Line 28.70 for 27.80, FastLane 17 Feb 46.10 for
    41.60, Skyline Air flight 624.00 for 642.00, Noodle House 112.04 for 112.40),
  - claims the two mixed receipts in full (Harbor Hotel 451.79 with minibar and
    a movie; Lakeside Grill 174.45 with wine),
  - claims three euro receipts at their euro face value,
  - claims the Bloom & Stem client gift at 95.00 (the policy caps gifts at 75),
  - lists the StreamBox subscription (personal),
  - lists Bistro Nine twice,
  - has a City Cab taxi line with no receipt,
  - and its Total is the sum of what it lists.
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[4] / "scripts" / "computer-use-macos"))
import seedlib as S  # noqa: E402

L = S.Line
RATE = 1.0874  # EUR to USD on the card statement

# (date, vendor, description on the report, receipt items, file, kind)
# items: [(text, amount)], in USD unless kind is "eur"; kind: "biz", "mixed",
# "eur" or "personal".
RECEIPTS = [
    ("2026-02-02", "Logan Taxi Co.", "Taxi, home to Logan airport", [("Fare and tip", 46.20)], "2026-02-02 Logan Taxi.pdf", "biz"),
    ("2026-02-02", "Skyline Air", "Checked bag fee, Boston to Chicago", [("First checked bag", 35.00)], "2026-02-02 Skyline Air bag.pdf", "biz"),
    ("2026-02-02", "O'Hare Coffee", "Coffee, O'Hare airport", [("Latte, large", 6.40)], "2026-02-02 OHare Coffee.pdf", "biz"),
    ("2026-02-03", "Lakeside Grill", "Client dinner, Northwind", [("Dinner for four", 108.45), ("Bottle of wine", 46.00), ("Tip", 20.00)], "2026-02-03 Lakeside Grill.pdf", "mixed"),
    ("2026-02-03", "Metro Cab", "Taxi to Northwind office", [("Fare", 22.10)], "2026-02-03 Metro Cab.pdf", "biz"),
    ("2026-02-04", "Copy Corner", "Printing, pitch decks", [("Color printing, 150 pages", 18.75)], "2026-02-04 Copy Corner.pdf", "biz"),
    ("2026-02-04", "Deep Dish Co.", "Lunch, Chicago trip", [("Lunch special, Chicago Loop", 17.85)], "2026-02-04 Deep Dish.pdf", "biz"),
    ("2026-02-05", "Harbor Hotel Chicago", "Hotel, 2 nights, Chicago", [("Room, 2 nights", 412.80), ("Minibar", 24.00), ("In-room movie", 14.99)], "2026-02-05 Harbor Hotel.pdf", "mixed"),
    ("2026-02-05", "Skyline Air", "Flight change fee", [("Change fee", 75.00)], "2026-02-05 Skyline Air change.pdf", "biz"),
    ("2026-02-06", "Paper Trail Supplies", "Office supplies", [("Notebooks and pens", 54.99)], "2026-02-06 Paper Trail.pdf", "biz"),
    ("2026-02-08", "StreamBox", "StreamBox subscription", [("Premium plan, monthly", 15.99)], "2026-02-08 StreamBox.pdf", "personal"),
    ("2026-02-09", "Garage 7", "Parking, client visit", [("Parking, 4 hours", 31.00)], "2026-02-09 Garage 7.pdf", "biz"),
    ("2026-02-10", "Bistro Nine", "Lunch with candidate", [("Lunch for two", 64.30)], "2026-02-10 Bistro Nine.pdf", "biz"),
    ("2026-02-10", "Cloudnote", "Note-taking software, annual", [("Cloudnote Pro, 1 year", 59.99)], "2026-02-10 Cloudnote.pdf", "biz"),
    ("2026-02-11", "DevSummit", "Conference registration", [("DevSummit 2026 pass", 299.00)], "2026-02-11 DevSummit.pdf", "biz"),
    ("2026-02-12", "Northeast Rail", "Train, Boston to New York", [("Ticket, one way", 89.00)], "2026-02-12 Northeast Rail.pdf", "biz"),
    ("2026-02-12", "Union Square Inn", "Hotel, 1 night, New York", [("Room, 1 night", 219.60)], "2026-02-12 Union Square Inn.pdf", "biz"),
    ("2026-02-13", "Daybreak Diner", "Team breakfast, New York office", [("Breakfast for five", 87.25)], "2026-02-13 Daybreak Diner.pdf", "biz"),
    ("2026-02-13", "Yellow Line Taxi", "Taxi to the New York office", [("Fare", 27.80)], "2026-02-13 Yellow Line.pdf", "biz"),
    ("2026-02-13", "Pressed Juice NYC", "Juice, New York trip", [("Green juice", 8.50)], "2026-02-13 Pressed Juice.pdf", "biz"),
    ("2026-02-14", "Northeast Rail", "Train, New York to Boston", [("Ticket, one way", 89.00)], "2026-02-14 Northeast Rail.pdf", "biz"),
    ("2026-02-14", "Rosewood Restaurant", "Dinner", [("Valentine's set menu for two, Boston", 186.50)], "2026-02-14 Rosewood.pdf", "personal"),
    ("2026-02-15", "Peak Fitness", "Membership, March", [("Monthly membership", 49.00)], "2026-02-15 Peak Fitness.pdf", "personal"),
    ("2026-02-16", "Diagram Pro", "Diagram software, monthly", [("Diagram Pro, 1 month", 12.00)], "2026-02-16 Diagram Pro.pdf", "biz"),
    ("2026-02-17", "FastLane Couriers", "Courier, contracts to Cobalt", [("Same-day courier", 41.60)], "2026-02-17 FastLane.pdf", "biz"),
    ("2026-02-17", "Garage 7", "Parking, client visit", [("Parking, 4 hours", 31.00)], "2026-02-17 Garage 7.pdf", "biz"),
    ("2026-02-18", "Kinko Print Hub", "Printing, workshop handouts", [("Black and white, 400 pages", 36.40)], "2026-02-18 Kinko Print Hub.pdf", "biz"),
    ("2026-02-18", "Bloom & Stem", "Client gift, flowers for Cobalt launch", [("Arrangement, delivered", 95.00)], "2026-02-18 Bloom and Stem.pdf", "biz"),
    ("2026-02-19", "Skyline Air", "Flight, Boston to Lisbon", [("Economy, round trip", 642.00)], "2026-02-19 Skyline Air flight.pdf", "biz"),
    ("2026-02-20", "Taxis Lisboa", "Taxi, Lisbon airport to hotel", [("Fare", 28.50)], "2026-02-20 Taxis Lisboa.pdf", "eur"),
    ("2026-02-20", "Casa Alfama", "Hotel, 3 nights, Lisbon", [("Room, 3 nights", 396.00)], "2026-02-20 Casa Alfama.pdf", "eur"),
    ("2026-02-21", "Tasca do Bairro", "Client dinner, Lusitania Bank", [("Dinner for three", 142.00)], "2026-02-21 Tasca do Bairro.pdf", "eur"),
    ("2026-02-22", "Sweet Layers Bakery", "Birthday cake", [("Birthday cake, \"Happy 8th, Maya\"", 42.00)], "2026-02-22 Sweet Layers.pdf", "personal"),
    ("2026-02-23", "Taxis Lisboa", "Taxi, hotel to Lisbon airport", [("Fare", 27.00)], "2026-02-23 Taxis Lisboa.pdf", "eur"),
    ("2026-02-23", "Skyline Air", "Checked bag fee, Lisbon to Boston", [("First checked bag", 35.00)], "2026-02-23 Skyline Air bag.pdf", "biz"),
    ("2026-02-24", "Logan Taxi Co.", "Taxi, Logan airport to home", [("Fare and tip", 48.60)], "2026-02-24 Logan Taxi.pdf", "biz"),
    ("2026-02-25", "Noodle House", "Team lunch", [("Lunch for six", 112.40)], "2026-02-25 Noodle House.pdf", "biz"),
    ("2026-02-26", "Paper Trail Supplies", "Printer toner", [("Toner cartridge", 89.99)], "2026-02-26 Paper Trail.pdf", "biz"),
    ("2026-02-26", "FastLane Couriers", "Courier, signed NDA to Brightwave", [("Standard courier", 23.80)], "2026-02-26 FastLane.pdf", "biz"),
    ("2026-02-27", "Garage 7", "Parking, client visit", [("Parking, 4 hours", 31.00)], "2026-02-27 Garage 7.pdf", "biz"),
]  # fmt: skip
DUPLICATE_SCAN = "Scan 2026-02-10.pdf"  # the Bistro Nine receipt again
MISSING = {"2026-02-02 OHare Coffee.pdf", "2026-02-13 Pressed Juice.pdf", "2026-02-18 Kinko Print Hub.pdf",
           "2026-02-23 Taxis Lisboa.pdf", "2026-02-26 Paper Trail.pdf"}  # fmt: skip
WRONG = {"2026-02-12 Union Square Inn.pdf": 291.60, "2026-02-11 DevSummit.pdf": 290.00, "2026-02-13 Yellow Line.pdf": 28.70,
         "2026-02-17 FastLane.pdf": 46.10, "2026-02-19 Skyline Air flight.pdf": 624.00, "2026-02-25 Noodle House.pdf": 112.04}  # fmt: skip
LISTED_PERSONAL = {"2026-02-08 StreamBox.pdf"}
UNBACKED = ("2026-02-13", "Taxi, airport (City Cab)", 38.00)
MONTHS = ("Jan", "Feb", "Mar")
HEAD = (
    "{\\rtf1\\ansi\\ansicpg1252\\cocoartf2907\n"
    "\\cocoatextscaling0\\cocoaplatform0{\\fonttbl\\f0\\fswiss\\fcharset0 Helvetica;}\n"
    "{\\colortbl;\\red255\\green255\\blue255;}\n"
    "{\\*\\expandedcolortbl;;}\n"
    "\\paperw11900\\paperh16840\\margl1440\\margr1440\\vieww11520\\viewh8400\\viewkind0\n"
    "\\pard\\tx1440\\tx5760\\pardirnatural\\partightenfactor0\n"
    "\n\\f0\\fs24 \\cf0 "
)


def day(d: str) -> str:
    y, m, dd = d.split("-")
    return f"{int(dd):02d} {MONTHS[int(m) - 1]} {y}"


def money(x: float) -> str:
    return f"{x:,.2f}"


def face(items: list[tuple[str, float]]) -> float:
    return round(sum(a for _, a in items), 2)


def report() -> bytes:
    rows: list[tuple[str, str, float]] = []
    for d, vendor, desc, items, fname, kind in RECEIPTS:
        if fname in MISSING or (kind == "personal" and fname not in LISTED_PERSONAL):
            continue
        rows.append((d, f"{desc} ({vendor})", WRONG.get(fname, face(items))))
        if vendor == "Bistro Nine":
            rows.append((d, f"{desc} ({vendor})", face(items)))
        if vendor == "Daybreak Diner":
            rows.append(UNBACKED)
    lines = ["February 2026 expenses, Jordan Lee", "", "Date\\tab Description\\tab Amount (USD)"]
    lines += [f"{day(d)}\\tab {desc}\\tab {money(a)}" for d, desc, a in rows]
    total = round(sum(a for _, _, a in rows), 2)
    lines += ["", f"Total\\tab \\tab {money(total)}"]
    return (HEAD + "\\\n".join(lines) + "}").encode("latin-1")


def receipt(vendor: str, d: str, items: list[tuple[str, float]], kind: str) -> bytes:
    cur = "EUR" if kind == "eur" else "USD"
    it: list[object] = [L(72, 730, vendor, 18, bold=True), L(72, 708, f"Receipt  |  {day(d)}", 10, gray=0.3)]
    y = 670.0
    for desc, a in items:
        it.append(L(72, y, desc, 10))
        it.append(L(430, y, f"{cur} {money(a)}", 10))
        y -= 15
    total = face(items)
    it.append(S.Rule(72, y + 4, 540, y + 4))
    it.append(L(330, y - 14, "Total paid", 11, bold=True))
    it.append(L(430, y - 14, f"{cur} {money(total)}", 11, bold=True))
    card = "Paid by Visa ending 4417."
    if kind == "eur":
        usd = round(total * RATE + 1e-9, 2)
        card = f"Paid by Visa ending 4417. Card charged USD {money(usd)} (EUR {money(total)} at {RATE})."
    it.append(L(72, y - 44, card, 9, gray=0.3))
    it.append(L(72, y - 60, "Thank you!", 9, gray=0.3))
    return S.pdf([it])


POLICY = [
    "Expense policy (summary)",
    "",
    "Reimbursable: travel (fares, fees, taxis, parking), lodging on business trips,",
    "meals while travelling on business, meals with clients, candidates or the team",
    "(tips included), conference fees, office supplies and printing, business",
    "software, couriers, and client gifts up to USD 75 each.",
    "",
    "Not reimbursable: alcohol, hotel minibar and in-room entertainment, meals and",
    "entertainment at home, gym or fitness memberships, personal subscriptions, and",
    "anything for family or friends.",
    "",
    "How to claim:",
    "- If a receipt mixes the two, claim only the reimbursable items.",
    "- For a purchase in another currency, claim the US dollars charged to the card.",
    "- A client gift over USD 75 is claimed at 75.00.",
    "- Every line needs a receipt. List each expense once, in date order.",
]


def main(root: Path) -> None:
    exp = root / "Users" / "admin" / "Documents" / "Expenses"
    for d, vendor, _, items, fname, kind in RECEIPTS:
        body = receipt(vendor, d, items, kind)
        S.put(exp / "Receipts" / fname, f"{d} 19:00", body)
        if vendor == "Bistro Nine":
            S.put(exp / "Receipts" / DUPLICATE_SCAN, "2026-02-11 08:40", body)
    S.put(exp / "Expense policy.pdf", "2026-01-05 10:00", S.pdf([[L(72, 730 - 16 * i, t, 12 if i == 0 else 10, bold=i == 0) for i, t in enumerate(POLICY) if t]]))
    S.put(exp / "February expense report.rtf", "2026-02-26 17:30", report())
    S.stamp_dir(exp / "Receipts", "2026-02-26 17:00")
    S.stamp_dir(exp, "2026-02-26 17:30")
    print(len(list((exp / "Receipts").iterdir())), "receipts")


if __name__ == "__main__":
    main(Path(sys.argv[1]))
