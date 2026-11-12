"""invoices-to-reminders seed, run on the host against the task VM's Data
volume (build_task_image.py): python3 seed_fs.py <data-volume-root>.

Six PDFs in ~/Downloads/Invoices (today is Monday 9 March 2026):
  Brightwave Printing   invoice 27 Feb, Net 15            -> due 14 Mar, USD 1,240.00
  Cobalt Cloud Hosting  due date 31 March                 -> due 31 Mar, USD 389.50
  Atelier Lumiere       due date 16.03.2026 (day first)   -> due 16 Mar, EUR 2.450,00
  Greenleaf Landscaping invoice 20 Feb, due in 30 days    -> due 22 Mar, USD 860.00
  Northwind Office      due 6 Mar, stamped PAID           -> skip
  Brightwave credit note CN-0099, USD -120.00             -> skip (not a bill)
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[4] / "scripts" / "computer-use-macos"))
import seedlib as S  # noqa: E402

L = S.Line


def invoice(vendor: str, addr: str, head: list[str], items: list[tuple[str, str]], total: str,
            foot: list[str], stamp: str | None = None) -> bytes:
    it: list[object] = [L(72, 730, vendor, 20, bold=True), L(72, 710, addr, 9, gray=0.3)]
    y = 670.0
    for h in head:
        it.append(L(72, y, h, 10))
        y -= 15
    y -= 10
    it.append(S.Rule(72, y + 4, 540, y + 4))
    it.append(L(72, y - 12, "Description", 10, bold=True))
    it.append(L(450, y - 12, "Amount", 10, bold=True))
    y -= 30
    for d, a in items:
        it.append(L(72, y, d, 10))
        it.append(L(450, y, a, 10))
        y -= 15
    it.append(S.Rule(72, y + 4, 540, y + 4))
    it.append(L(330, y - 14, "Total due", 11, bold=True))
    it.append(L(450, y - 14, total, 11, bold=True))
    y -= 50
    for f in foot:
        it.append(L(72, y, f, 9, gray=0.25))
        y -= 13
    if stamp:
        it.append(S.Stamp(300, 520, stamp))
    return S.pdf([it])


def documents() -> dict[str, bytes]:
    d = "Downloads/Invoices/"
    return {
        d + "INV-2026-0412 Brightwave Printing.pdf": invoice(
            "Brightwave Printing", "18 Dry Dock Ave, Boston MA 02210",
            ["Invoice INV-2026-0412", "Invoice date: February 27, 2026", "Terms: Net 15"],
            [("Trade show banners (3)", "USD 960.00"), ("Brochures, 500", "USD 280.00")], "USD 1,240.00",
            ["Please pay within 15 days of the invoice date.", "Bank transfer or check payable to Brightwave Printing."]),
        d + "Cobalt Cloud - March 2026.pdf": invoice(
            "Cobalt Cloud Hosting", "500 Brannan St, San Francisco CA 94107",
            ["Invoice CC-88120", "Invoice date: March 1, 2026", "Due date: March 31, 2026"],
            [("Managed hosting, March", "USD 349.50"), ("Backups, March", "USD 40.00")], "USD 389.50",
            ["Card on file will not be charged; pay by transfer."]),
        d + "Atelier Lumiere F-2026-118.pdf": invoice(
            "Atelier Lumiere", "12 rue des Arts, 69002 Lyon, France",
            ["Invoice F-2026-118", "Invoice date: 02.03.2026", "Due date: 16.03.2026"],
            [("Product photography, 1 day", "EUR 1.800,00"), ("Retouching, 26 images", "EUR 650,00")], "EUR 2.450,00",
            ["VAT reverse charge.", "IBAN FR76 3000 6000 0112 3456 7890 189"]),
        d + "Greenleaf Landscaping 2026-031.pdf": invoice(
            "Greenleaf Landscaping", "71 Elm St, Newton MA 02458",
            ["Invoice 2026-031", "Invoice date: February 20, 2026"],
            [("Winter cleanup", "USD 610.00"), ("Mulch delivery", "USD 250.00")], "USD 860.00",
            ["Payment is due 30 days from the invoice date."]),
        d + "Northwind Office Supply 7781.pdf": invoice(
            "Northwind Office Supply", "9 Wharf St, Boston MA 02110",
            ["Invoice 7781", "Invoice date: February 20, 2026", "Due date: March 6, 2026"],
            [("Desk chairs (2)", "USD 498.00")], "USD 498.00",
            ["Thank you for your business."], stamp="PAID  MAR 4"),
        d + "Brightwave credit note CN-0099.pdf": invoice(
            "Brightwave Printing", "18 Dry Dock Ave, Boston MA 02210",
            ["CREDIT NOTE CN-0099", "Date: March 3, 2026", "Against invoice INV-2026-0388"],
            [("Misprinted flyers, refund", "USD -120.00")], "USD -120.00",
            ["This credit will be applied to your next invoice. No payment is due."]),
    }


STAMP = "2026-03-06 18:30"


def main(root: Path) -> None:
    home = root / "Users" / "admin"
    for rel, data in documents().items():
        S.put(home / rel, STAMP, data)
    S.stamp_dir(home / "Downloads" / "Invoices", STAMP)
    for rel in documents():
        print(rel, (home / rel).stat().st_size)


if __name__ == "__main__":
    main(Path(sys.argv[1]))
