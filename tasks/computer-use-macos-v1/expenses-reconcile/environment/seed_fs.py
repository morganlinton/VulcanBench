"""expenses-reconcile seed, run on the host against the task VM's Data
volume (build_task_image.py): python3 seed_fs.py <data-volume-root>.

~/Documents/Expenses holds "February expense report.rtf" (TextEdit rich
text, one line per expense: date, tab, description, tab, amount, then a
Total line), "Expense policy.pdf", and Receipts/ with 24 receipt PDFs: 22
business receipts and two personal ones (a Valentine's dinner and a gym
membership, not reimbursable under the policy). The report as seeded:
  - lists 19 of the 22 business receipts (O'Hare Coffee, Daybreak Diner and
    Bloom & Stem are missing),
  - has five wrong amounts (hotel 421.80 for 412.80, Lakeside Grill 182.45
    for 128.45, DevSummit 290.00 for 299.00, FastLane 46.10 for 41.60,
    Noodle House 112.04 for 112.40),
  - lists the personal Rosewood dinner,
  - and its Total is the sum of what it lists.
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[4] / "scripts" / "computer-use-macos"))
import seedlib as S  # noqa: E402

L = S.Line
# (date, vendor, description on the report, amount, receipt file, personal)
RECEIPTS = [
    ("2026-02-02", "Logan Taxi Co.", "Taxi, home to Logan airport", 46.20, "2026-02-02 Logan Taxi.pdf", False),
    ("2026-02-02", "Skyline Air", "Checked bag fee, Boston to Chicago", 35.00, "2026-02-02 Skyline Air bag.pdf", False),
    ("2026-02-03", "Harbor Hotel Chicago", "Hotel, 2 nights, Chicago", 412.80, "2026-02-03 Harbor Hotel.pdf", False),
    ("2026-02-03", "Lakeside Grill", "Client dinner, Northwind", 128.45, "2026-02-03 Lakeside Grill.pdf", False),
    ("2026-02-04", "Metro Cab", "Taxi to Northwind office", 22.10, "2026-02-04 Metro Cab.pdf", False),
    ("2026-02-04", "Copy Corner", "Printing, pitch decks", 18.75, "2026-02-04 Copy Corner.pdf", False),
    ("2026-02-05", "Skyline Air", "Flight change fee", 75.00, "2026-02-05 Skyline Air change.pdf", False),
    ("2026-02-05", "O'Hare Coffee", "Coffee, O'Hare airport", 6.40, "2026-02-05 OHare Coffee.pdf", False),
    ("2026-02-09", "Paper Trail Supplies", "Office supplies", 54.99, "2026-02-09 Paper Trail.pdf", False),
    ("2026-02-10", "Garage 7", "Parking, client visit", 31.00, "2026-02-10 Garage 7.pdf", False),
    ("2026-02-11", "Bistro Nine", "Lunch with candidate", 64.30, "2026-02-11 Bistro Nine.pdf", False),
    ("2026-02-12", "DevSummit", "Conference registration", 299.00, "2026-02-12 DevSummit.pdf", False),
    ("2026-02-14", "Rosewood Restaurant", "Dinner", 186.50, "2026-02-14 Rosewood.pdf", True),
    ("2026-02-16", "Northeast Rail", "Train, Boston to New York", 89.00, "2026-02-16 Northeast Rail.pdf", False),
    ("2026-02-16", "Union Square Inn", "Hotel, 1 night, New York", 219.60, "2026-02-16 Union Square Inn.pdf", False),
    ("2026-02-17", "Daybreak Diner", "Team breakfast, New York office", 87.25, "2026-02-17 Daybreak Diner.pdf", False),
    ("2026-02-17", "Yellow Line Taxi", "Taxi to the New York office", 27.80, "2026-02-17 Yellow Line.pdf", False),
    ("2026-02-18", "Northeast Rail", "Train, New York to Boston", 89.00, "2026-02-18 Northeast Rail.pdf", False),
    ("2026-02-19", "Diagram Pro", "Diagram software, monthly", 12.00, "2026-02-19 Diagram Pro.pdf", False),
    ("2026-02-20", "FastLane Couriers", "Courier, contracts to Cobalt", 41.60, "2026-02-20 FastLane.pdf", False),
    ("2026-02-21", "Peak Fitness", "Membership, March", 49.00, "2026-02-21 Peak Fitness.pdf", True),
    ("2026-02-23", "Bloom & Stem", "Client gift, flowers for Cobalt launch", 58.00, "2026-02-23 Bloom and Stem.pdf", False),
    ("2026-02-24", "Garage 7", "Parking, client visit", 31.00, "2026-02-24 Garage 7.pdf", False),
    ("2026-02-25", "Noodle House", "Team lunch", 112.40, "2026-02-25 Noodle House.pdf", False),
]
MISSING = {"O'Hare Coffee", "Daybreak Diner", "Bloom & Stem"}
WRONG = {"Harbor Hotel Chicago": 421.80, "Lakeside Grill": 182.45, "DevSummit": 290.00, "FastLane Couriers": 46.10, "Noodle House": 112.04}
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


def report() -> bytes:
    lines = ["February 2026 expenses, Jordan Lee", "", "Date\\tab Description\\tab Amount (USD)"]
    total = 0.0
    for d, vendor, desc, amt, _, personal in RECEIPTS:
        if vendor in MISSING or (personal and vendor != "Rosewood Restaurant"):
            continue
        a = WRONG.get(vendor, amt)
        total += a
        lines.append(f"{day(d)}\\tab {desc} ({vendor})\\tab {money(a)}")
    lines += ["", f"Total\\tab \\tab {money(round(total, 2))}"]
    body = "\\\n".join(lines)
    return (HEAD + body + "}").encode("latin-1")


def receipt(vendor: str, d: str, items: list[tuple[str, float]], total: float, extra: str = "") -> bytes:
    it: list[object] = [L(72, 730, vendor, 18, bold=True), L(72, 708, f"Receipt  |  {day(d)}", 10, gray=0.3)]
    y = 670.0
    for desc, a in items:
        it.append(L(72, y, desc, 10))
        it.append(L(430, y, f"USD {money(a)}", 10))
        y -= 15
    it.append(S.Rule(72, y + 4, 540, y + 4))
    it.append(L(330, y - 14, "Total paid", 11, bold=True))
    it.append(L(430, y - 14, f"USD {money(total)}", 11, bold=True))
    if extra:
        it.append(L(72, y - 44, extra, 9, gray=0.3))
    it.append(L(72, y - 60, "Paid by Visa ending 4417. Thank you!", 9, gray=0.3))
    return S.pdf([it])


POLICY = [
    "Expense policy (summary)",
    "",
    "Reimbursable: travel (fares, fees, taxis, parking), lodging on business trips,",
    "meals with clients, candidates or the team, conference fees, office supplies,",
    "business software, couriers, and client gifts up to USD 75.",
    "",
    "Not reimbursable: personal meals and entertainment, gym or fitness memberships,",
    "personal subscriptions, and anything for family or friends.",
    "",
    "List each expense once, in date order, with the amount on its receipt.",
]


def main(root: Path) -> None:
    exp = root / "Users" / "admin" / "Documents" / "Expenses"
    for d, vendor, desc, amt, fname, _ in RECEIPTS:
        S.put(exp / "Receipts" / fname, f"{d} 19:00", receipt(vendor, d, [(desc, amt)], amt))
    S.put(exp / "Expense policy.pdf", "2026-01-05 10:00", S.pdf([[L(72, 730 - 16 * i, t, 12 if i == 0 else 10, bold=i == 0) for i, t in enumerate(POLICY) if t]]))
    S.put(exp / "February expense report.rtf", "2026-02-26 17:30", report())
    S.stamp_dir(exp / "Receipts", "2026-02-26 17:00")
    S.stamp_dir(exp, "2026-02-26 17:30")
    print(len(list((exp / "Receipts").iterdir())), "receipts")


if __name__ == "__main__":
    main(Path(sys.argv[1]))
