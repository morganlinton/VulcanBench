"""finder-smart-folder seed, run on the host against the task VM's Data
volume (build_task_image.py): python3 seed_fs.py <data-volume-root>.

A Documents folder with client subfolders holding invoices (PDF), contracts
and an invoice template that is not a PDF, plus an invoice in Downloads
that is outside the folder the smart folder must search. What the smart
folder shows is not graded (the guest's Spotlight index is not reliable);
the files make the search meaningful to an agent checking its work.
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[4] / "scripts" / "computer-use-macos"))
import seedlib as S  # noqa: E402


def doc(title: str, *lines: str) -> bytes:
    return S.pdf([S.text_page([f"# {title}", "", *lines])])


FILES = {
    "Documents/Clients/Acme/Invoice 2026-014 Acme.pdf": ("2026-02-02 10:00", doc("Invoice 2026-014", "Acme Corp", "USD 4,500.00")),
    "Documents/Clients/Acme/Invoice 2026-021 Acme.pdf": ("2026-03-02 10:00", doc("Invoice 2026-021", "Acme Corp", "USD 6,700.00")),
    "Documents/Clients/Acme/Acme MSA - executed.pdf": ("2026-03-06 12:00", doc("Master Services Agreement", "Acme Corp")),
    "Documents/Clients/Northwind/northwind-invoice-feb.pdf": ("2026-02-27 16:30", doc("Invoice NW-0207", "Northwind Labs", "USD 3,150.00")),
    "Documents/Clients/Northwind/Statement of work.pdf": ("2026-01-12 15:00", doc("Statement of Work", "Northwind Labs")),
    "Documents/Admin/Invoice template.docx": ("2025-12-01 09:00", b"PK\x03\x04 not a real document\n"),
    "Documents/Admin/Expense policy.pdf": ("2026-01-05 09:00", doc("Expense policy")),
    "Documents/Personal/Tax 2025 summary.pdf": ("2026-02-20 10:15", doc("Tax summary 2025")),
    "Downloads/Invoice 88213 Hilton.pdf": ("2026-03-05 08:10", doc("Folio 88213", "Hilton Garden Inn")),
}


def main(root: Path) -> None:
    home = root / "Users" / "admin"
    for rel, (stamp, data) in FILES.items():
        S.put(home / rel, stamp, data)
    for d in ("Documents/Clients/Acme", "Documents/Clients/Northwind", "Documents/Clients", "Documents/Admin", "Documents/Personal"):
        S.stamp_dir(home / d, "2026-03-06 12:00")
    for rel in FILES:
        print(rel, (home / rel).stat().st_size)


if __name__ == "__main__":
    main(Path(sys.argv[1]))
