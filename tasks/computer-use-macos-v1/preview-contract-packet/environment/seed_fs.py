"""preview-contract-packet seed, run on the host against the task VM's Data
volume (build_task_image.py): python3 seed_fs.py <data-volume-root>.

Three PDFs in ~/Downloads:
- "Acme MSA - draft 3.pdf", seven pages: 1 cover, 2 sections 1 to 3, 3 a
  blank scanned page, 4 sections 4 to 6, 5 a duplicate of page 4, 6 sections
  7 to 9, 7 the unsigned signature page.
- "Acme MSA - Exhibit A.pdf", two pages of pricing.
- "Acme MSA - signature page (signed).pdf", one page, signed.
Plus unrelated files that must not change. The expected packet is draft
pages 1, 2, 4, 6, Exhibit A pages 1 and 2, then the signed page.

`documents()` returns {relative path under the home folder: bytes}.
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[4] / "scripts" / "computer-use-macos"))
import seedlib as S  # noqa: E402

L = S.Line
DRAFT_FOOT = "Acme Corp / Harbor Analytics LLC  -  Master Services Agreement  -  Draft 3"


def page(lines: list[str], foot: str | None = None) -> list[object]:
    items = S.text_page(lines, size=11, top=720)
    if foot:
        items.append(L(72, 40, foot, 8, gray=0.35))
    return items


def draft() -> bytes:
    p1 = page(
        ["# MASTER SERVICES AGREEMENT", "", "between", "", "Acme Corp (\"Customer\")", "and",
         "Harbor Analytics LLC (\"Provider\")", "", "Effective date: March 1, 2026", "",
         "Draft 3, for signature"],
        DRAFT_FOOT + "  -  page 1 of 7",
    )
    p2 = page(
        ["# 1. Services", "Provider will deliver the analytics services described in Exhibit A.",
         "", "# 2. Personnel", "Each party names one coordinator for the services.", "",
         "# 3. Acceptance", "Customer reviews each deliverable within ten business days."],
        DRAFT_FOOT + "  -  page 2 of 7",
    )
    p3: list[object] = [L(560, 30, ".", 6, gray=0.85)]  # a blank scanned page: one speck
    p4 = page(
        ["# 4. Fees", "Customer pays the fees in Exhibit A within thirty days of invoice.", "",
         "# 5. Confidentiality", "Each party protects the other's confidential information.", "",
         "# 6. Data", "Provider processes Customer data only to perform the services."],
        DRAFT_FOOT + "  -  page 4 of 7",
    )
    p5 = page(  # the same page again, as a scanner feeds a page twice
        ["# 4. Fees", "Customer pays the fees in Exhibit A within thirty days of invoice.", "",
         "# 5. Confidentiality", "Each party protects the other's confidential information.", "",
         "# 6. Data", "Provider processes Customer data only to perform the services."],
        DRAFT_FOOT + "  -  page 4 of 7",
    )
    p6 = page(
        ["# 7. Term", "This agreement runs for twelve months from the effective date.", "",
         "# 8. Termination", "Either party may end it on sixty days' written notice.", "",
         "# 9. Governing law", "The laws of the Commonwealth of Massachusetts apply."],
        DRAFT_FOOT + "  -  page 6 of 7",
    )
    p7 = page(
        ["# Signatures", "", "Acme Corp", "Signature: ______________________", "Name:",
         "Date:", "", "Harbor Analytics LLC", "Signature: ______________________",
         "Name:", "Date:"],
        DRAFT_FOOT + "  -  page 7 of 7",
    )
    return S.pdf([p1, p2, p3, p4, p5, p6, p7], title="Acme MSA - draft 3")


def exhibit() -> bytes:
    foot = "Exhibit A to the Master Services Agreement  -  page {} of 2"
    e1 = page(
        ["# EXHIBIT A: SERVICES AND FEES", "", "A.1 Monthly analytics reporting      USD 4,500 per month",
         "A.2 Quarterly business review        USD 2,000 per quarter",
         "A.3 Dashboard maintenance            USD 1,200 per month"],
        foot.format(1),
    )
    e2 = page(
        ["# EXHIBIT A (continued)", "", "A.4 Ad hoc analysis                  USD 180 per hour",
         "A.5 Fees are invoiced monthly in arrears.", "A.6 Rates are fixed for the initial term."],
        foot.format(2),
    )
    return S.pdf([e1, e2], title="Acme MSA - Exhibit A")


def signed() -> bytes:
    sig = page(
        ["# Signatures", "", "Acme Corp", "Signature: /s/ Dana Whitfield", "Name: Dana Whitfield, COO",
         "Date: March 6, 2026", "", "Harbor Analytics LLC", "Signature: /s/ Jordan Lee",
         "Name: Jordan Lee, Managing Partner", "Date: March 6, 2026"],
        "Master Services Agreement  -  executed signature page",
    )
    return S.pdf([sig], title="Acme MSA - signature page (signed)")


def documents() -> dict[str, bytes]:
    return {
        "Downloads/Acme MSA - draft 3.pdf": draft(),
        "Downloads/Acme MSA - Exhibit A.pdf": exhibit(),
        "Downloads/Acme MSA - signature page (signed).pdf": signed(),
        "Downloads/Harbor NDA - template.pdf": S.pdf(
            [page(["# MUTUAL NONDISCLOSURE AGREEMENT", "", "Template, not for signature."])]
        ),
        "Documents/Contracts/Northwind MSA - executed.pdf": S.pdf(
            [page(["# MASTER SERVICES AGREEMENT", "", "Northwind Labs and Harbor Analytics LLC",
                   "", "Executed January 12, 2026"])]
        ),
    }


STAMPS = {
    "Downloads/Acme MSA - draft 3.pdf": "2026-03-04 16:02",
    "Downloads/Acme MSA - Exhibit A.pdf": "2026-03-04 16:03",
    "Downloads/Acme MSA - signature page (signed).pdf": "2026-03-06 11:41",
    "Downloads/Harbor NDA - template.pdf": "2026-02-10 09:30",
    "Documents/Contracts/Northwind MSA - executed.pdf": "2026-01-12 15:00",
}


def main(root: Path) -> None:
    home = root / "Users" / "admin"
    for rel, data in documents().items():
        S.put(home / rel, STAMPS[rel], data)
    S.stamp_dir(home / "Documents" / "Contracts", "2026-01-12 15:00")
    for rel in documents():
        p = home / rel
        print(rel, p.stat().st_size, "uid", p.stat().st_uid)


if __name__ == "__main__":
    main(Path(sys.argv[1]))
