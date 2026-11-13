"""preview-redline seed, run on the host against the task VM's Data volume
(build_task_image.py): python3 seed_fs.py <data-volume-root>.

~/Documents/Contracts holds "Northwind MSA - draft.pdf" (four pages; page 3
is stored with /Rotate 180, so it shows upside down) and "Legal comments.txt".
Line positions (PDF points from the bottom left of the unrotated page) are
listed in LAYOUT for the checker:
  page 2: clause 3.1 (Net 30) at y 640; the bank account line at y 610
  page 3: clause 5.2 (liability cap) at y 610 and 595
  page 4: clause 7.1 (termination for convenience) at y 700 and 685; the
          routing and account number lines at y 520 and 505
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[4] / "scripts" / "computer-use-macos"))
import seedlib as S  # noqa: E402

L = S.Line
STAMP = "2026-03-06 14:20"
ACCOUNT = "4417 2290 1185 03"
ROUTING = "021000089"
ACCOUNT2 = "7731 0045 9921"


def page(lines: list[tuple[float, str, bool]], head: str = "") -> list[object]:
    it: list[object] = []
    if head:
        it.append(L(72, 740, head, 16, bold=True))
    for y, t, b in lines:
        it.append(L(72, y, t, 11 if b else 10, bold=b))
    it.append(L(300, 40, "Northwind MSA  |  draft 3  |  confidential", 8, gray=0.4))
    return it


PAGES = [
    page([
        (712, "Master Services Agreement", True),
        (694, "between Harbor Analytics, Inc. (\"Harbor\") and Northwind Traders LLC (\"Northwind\")", False),
        (660, "1. Definitions", True),
        (642, "1.1 \"Services\" means the analytics services described in Exhibit A.", False),
        (627, "1.2 \"Deliverables\" means the reports and dashboards Harbor provides under this Agreement.", False),
        (600, "2. Services", True),
        (582, "2.1 Harbor will perform the Services with reasonable skill and care.", False),
        (567, "2.2 Northwind will give Harbor timely access to the data the Services need.", False),
    ], head="Northwind MSA"),
    page([
        (660, "3. Fees and payment", True),
        (640, "3.1 Northwind will pay each invoice within thirty (30) days of the invoice date.", False),
        (625, "3.2 Payments go by bank transfer to Harbor's operating account:", False),
        (610, f"     Bank account {ACCOUNT}, First Harbor Bank, Boston.", False),
        (590, "3.3 Late payments carry interest at 1% per month.", False),
        (560, "4. Term", True),
        (542, "4.1 This Agreement starts on the Effective Date and runs for two (2) years.", False),
        (527, "4.2 It renews for one-year terms unless either party gives 60 days' notice.", False),
    ]),
    page([
        (660, "5. Liability", True),
        (640, "5.1 Neither party is liable for indirect or consequential losses.", False),
        (610, "5.2 Each party's total liability under this Agreement is capped at the fees", False),
        (595, "     paid in the twelve (12) months before the claim.", False),
        (560, "6. Confidentiality", True),
        (542, "6.1 Each party keeps the other's confidential information secret for five years.", False),
    ]),
    page([
        (720, "7. Termination", True),
        (700, "7.1 Either party may terminate this Agreement for convenience on ninety (90)", False),
        (685, "     days' written notice.", False),
        (665, "7.2 Either party may terminate at once for a material breach not cured in 30 days.", False),
        (620, "8. Signatures and payment details", True),
        (600, "Signed for Harbor Analytics, Inc.: ____________________   Date: ________", False),
        (580, "Signed for Northwind Traders LLC: ____________________   Date: ________", False),
        (540, "Remittance details for Northwind's accounts payable team:", False),
        (520, f"     Routing number {ROUTING}", False),
        (505, f"     Account number {ACCOUNT2}", False),
    ]),
]
COMMENTS = """Legal comments on the Northwind MSA (from Dana)

Before this goes to Northwind:
1. Highlight clause 5.2 (the liability cap) and clause 7.1 (termination for convenience) so they see what changed.
2. Put a note on clause 3.1 that says: Net 45, not Net 30.
3. Redact our bank account number, routing number and account number. Real redaction, not black boxes: they must not be able to copy them out.
4. Page 3 got scanned upside down, fix it.
5. Save the result as "Northwind MSA - for Northwind.pdf" in the same folder. Keep the draft as it is.
"""


def main(root: Path) -> None:
    d = root / "Users" / "admin" / "Documents" / "Contracts"
    S.put(d / "Northwind MSA - draft.pdf", STAMP, S.pdf(PAGES, title="Northwind MSA draft 3", rotate={2: 180}))
    S.put(d / "Legal comments.txt", STAMP, COMMENTS)
    S.stamp_dir(d, STAMP)
    print("seeded contract and comments")


if __name__ == "__main__":
    main(Path(sys.argv[1]))
