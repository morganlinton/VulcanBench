"""finder-receipts-rename seed, run on the host against the task VM's Data
volume (build_task_image.py): python3 seed_fs.py <data-volume-root>.

Ten scanned receipts in ~/Documents/Receipts named the way a scanner names
them, an expense policy that is not a receipt, and a Personal folder holding
one receipt already renamed the way the owner wants. Seven were paid with
the business Amex ending 1009, three with the personal Visa ending 4417.
Dates are printed in each merchant's own format; the London hotel folio is
day first (Arrival 02/03/2026, Departure 04/03/2026, two nights).

`documents()` returns {relative path: bytes}; tests/check.py pins their
SHA-256 (tests/test_computer_use_macos.py rebuilds them to check the pins).
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[4] / "scripts" / "computer-use-macos"))
import seedlib as S  # noqa: E402

L = S.Line
AMEX = "AMEX ************1009"
VISA = "VISA ************4417"


def receipt(head: list[str], body: list[tuple[str, str]], foot: list[str], width: float = 288) -> bytes:
    """A narrow till receipt: centered header, item lines with right-aligned
    amounts, footer lines."""
    items: list[object] = []
    y = 760.0
    for i, h in enumerate(head):
        size = 15 if i == 0 else 9
        items.append(L(20 + max(0, (width - 40 - len(h) * size * 0.5) / 2), y, h, size, bold=i == 0))
        y -= size + 6
    y -= 8
    items.append(S.Rule(20, y + 6, width - 20, y + 6))
    for left, right in body:
        items.append(L(24, y - 8, left, 9))
        if right:
            items.append(L(width - 24 - len(right) * 5.2, y - 8, right, 9))
        y -= 14
    y -= 6
    items.append(S.Rule(20, y + 6, width - 20, y + 6))
    for f in foot:
        items.append(L(24, y - 8, f, 9))
        y -= 14
    return S.pdf([items], size=(width, 792))


def letter(lines: list[str]) -> bytes:
    return S.pdf([S.text_page(lines, size=10)])


def documents() -> dict[str, bytes]:
    d: dict[str, bytes] = {}
    d["Receipts/Scan.pdf"] = receipt(
        ["Office Depot", "Store #2214", "540 Atlantic Ave, Boston MA 02210", "(617) 555-0108"],
        [("03/02/2026  14:37   REG 3  TRN 0771", ""), ("TONER CARTRIDGE HP 67XL", "42.99"),
         ("COPY PAPER 8.5X11 5RM", "38.49"), ("SUBTOTAL", "81.48"), ("TAX 6.25%", "5.09"),
         ("TOTAL", "86.57")],
        [f"{AMEX}   86.57", "APPROVED  AUTH 004417", "Thank you for shopping Office Depot"],
    )
    d["Receipts/Scan 1.pdf"] = receipt(
        ["Whole Foods Market", "15 Westland Ave, Boston MA 02115"],
        [("2026-02-27 18:05", ""), ("ORGANIC BANANAS", "1.99"), ("SOURDOUGH LOAF", "6.49"),
         ("GREEK YOGURT 32OZ", "5.99"), ("SALMON FILLET", "14.21"), ("TOTAL", "28.68")],
        [f"{VISA}   28.68", "Prime member savings 2.40"],
    )
    d["Receipts/Scan 2.pdf"] = receipt(
        ["Delta Air Lines", "Electronic Ticket Receipt"],
        [("Date of issue: 18FEB26", ""), ("Ticket 006 2391847705", ""),
         ("BOS-LHR  DL 158  02MAR26  Seat 23C", ""), ("LHR-BOS  DL 159  05MAR26  Seat 24C", ""),
         ("Fare", "USD 612.00"), ("Taxes, fees, charges", "USD 186.40"),
         ("Total", "USD 798.40")],
        [f"Form of payment: {AMEX}", "Passenger: LEE/JORDAN"],
    )
    d["Receipts/Scan 3.pdf"] = receipt(
        ["Hilton Garden Inn", "London Bridge", "40 Borough High St, London SE1", "VAT No. GB 123 4567 89"],
        [("Guest: Mr Jordan Lee   Room 512", ""), ("Arrival 02/03/2026   Departure 04/03/2026", ""),
         ("Nights: 2", ""), ("02/03 Room charge", "GBP 189.00"), ("03/03 Room charge", "GBP 189.00"),
         ("03/03 Restaurant", "GBP 41.50"), ("Total incl. VAT", "GBP 419.50"),
         ("Paid 04/03/2026", "GBP 419.50")],
        [f"Settlement: {AMEX}", "Folio 88213  Thank you for staying with us"],
    )
    d["Receipts/Scan 4.pdf"] = receipt(
        ["Uber", "Trip receipt"],
        [("Mar 5, 2026 7:42 PM", ""), ("Logan Airport to Back Bay", ""), ("Trip fare", "$31.20"),
         ("Booking fee", "$3.15"), ("Tip", "$6.00"), ("Total", "$40.35")],
        [f"Charged to {AMEX}"],
    )
    d["Receipts/Scan 5.pdf"] = receipt(
        ["Shell", "1250 Commonwealth Ave, Allston MA"],
        [("02/21/26  09:12  PUMP 04", ""), ("UNLEADED 11.204 GAL @ 3.459", "38.75"),
         ("TOTAL", "38.75")],
        [f"{AMEX}", "AUTH 190233"],
    )
    d["Receipts/Scan 6.pdf"] = receipt(
        ["CVS Pharmacy", "Store 1023", "587 Boylston St, Boston MA"],
        [("03/06/2026 17:44", ""), ("ALLERGY RELIEF 30CT", "18.99"), ("TOOTHPASTE 2PK", "7.49"),
         ("TOTAL", "26.48")],
        [f"{VISA}   26.48", "ExtraCare card **4471"],
    )
    d["Receipts/Scan 7.pdf"] = receipt(
        ["Apple Store", "Boylston Street", "815 Boylston St, Boston MA 02116"],
        [("February 24, 2026  3:18 PM", ""), ("USB-C Digital AV Adapter", "69.00"),
         ("Magic Mouse", "79.00"), ("Subtotal", "148.00"), ("Tax", "9.25"),
         ("Total", "157.25")],
        [f"{AMEX}   157.25", "Order W1938472110"],
    )
    d["Receipts/Scan 8.pdf"] = receipt(
        ["Blue Bottle Coffee", "Seaport", "1 Seaport Blvd, Boston MA"],
        [("Tue 03/03/2026 8:12 AM", ""), ("Latte (2)", "12.50"), ("Croissant (2)", "9.00"),
         ("Total", "21.50")],
        [f"{AMEX}   21.50", "Meeting with client: R. Alvarez"],
    )
    d["Receipts/Scan 9.pdf"] = receipt(
        ["Staples", "Store 0417", "25 Court St, Boston MA"],
        [("Mar 6 2026  11:02 AM", ""), ("BACKPACK, KIDS", "34.99"), ("CRAYONS 64CT", "6.49"),
         ("GLUE STICKS 6PK", "4.29"), ("TOTAL", "45.77")],
        [f"{VISA}   45.77", "Staples Rewards #0417 2291"],
    )
    d["Receipts/Expense policy.pdf"] = letter(
        ["# Expense policy (2026)", "", "Submit receipts within 30 days of payment.",
         "Name each file with the payment date and the merchant.",
         "Personal purchases are not reimbursed; keep them separate.",
         "Business card: Amex ending 1009."]
    )
    d["Receipts/Personal/2026-01-30 Corner Bakery.pdf"] = receipt(
        ["Corner Bakery", "Newbury St, Boston MA"],
        [("01/30/2026 07:55", ""), ("Muffin", "3.75"), ("Coffee", "2.95"), ("TOTAL", "6.70")],
        [f"{VISA}   6.70"],
    )
    return d


STAMPS = {
    "Receipts/Scan.pdf": "2026-03-07 10:14",
    "Receipts/Scan 1.pdf": "2026-03-07 10:15",
    "Receipts/Scan 2.pdf": "2026-03-07 10:15",
    "Receipts/Scan 3.pdf": "2026-03-07 10:16",
    "Receipts/Scan 4.pdf": "2026-03-07 10:16",
    "Receipts/Scan 5.pdf": "2026-03-07 10:17",
    "Receipts/Scan 6.pdf": "2026-03-07 10:17",
    "Receipts/Scan 7.pdf": "2026-03-07 10:18",
    "Receipts/Scan 8.pdf": "2026-03-07 10:18",
    "Receipts/Scan 9.pdf": "2026-03-07 10:19",
    "Receipts/Expense policy.pdf": "2026-01-05 09:00",
    "Receipts/Personal/2026-01-30 Corner Bakery.pdf": "2026-02-01 18:20",
}


def main(root: Path) -> None:
    docs = root / "Users" / "admin" / "Documents"
    for rel, data in documents().items():
        S.put(docs / rel, STAMPS[rel], data)
    S.stamp_dir(docs / "Receipts" / "Personal", "2026-02-01 18:20")
    S.stamp_dir(docs / "Receipts", "2026-03-07 10:19")
    for p in sorted((docs / "Receipts").rglob("*")):
        print(p.relative_to(docs), p.stat().st_size, "uid", p.stat().st_uid)


if __name__ == "__main__":
    main(Path(sys.argv[1]))
