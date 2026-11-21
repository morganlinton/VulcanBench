"""finder-copy-conflict seed, run on the host against the task VM's Data
volume (build_task_image.py): python3 seed_fs.py <data-volume-root>.

~/Downloads/Board files from Sam holds three files Sam sent on 4 March. The
owner's ~/Documents/Projects/Board meeting already holds "Board memo.pdf",
which the owner edited on 8 March after starting from Sam's draft (its
first line says so), plus two files of their own. Copying Sam's folder in
with Finder raises "An item named Board memo.pdf already exists"; Replace
overwrites the owner's version.

`documents()` returns {relative path under the home folder: bytes};
tests/check.py pins their SHA-256.
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[4] / "scripts" / "computer-use-macos"))
import seedlib as S  # noqa: E402

SAM = "Downloads/Board files from Sam"
OWN = "Documents/Projects/Board meeting"


def doc(*lines: str) -> bytes:
    return S.pdf([S.text_page(list(lines))])


def documents() -> dict[str, bytes]:
    return {
        f"{SAM}/Board memo.pdf": doc(
            "# Board memo: Q1 review", "Draft v3 from Sam, 4 March 2026", "",
            "1. Revenue up 12% on Q4.", "2. Hiring plan: four engineers.", "3. Risks: TBD."),
        f"{SAM}/Financials Q1.pdf": doc(
            "# Financials, Q1 2026", "Prepared by Sam, 4 March 2026", "",
            "Revenue      USD 2.41M", "Gross margin 71%", "Cash         USD 6.8M"),
        f"{SAM}/Agenda.pdf": doc(
            "# Board meeting agenda", "Thursday 19 March 2026, 3 PM", "",
            "1. Q1 review", "2. Financials", "3. Hiring plan", "4. Any other business"),
        f"{OWN}/Board memo.pdf": doc(
            "# Board memo: Q1 review", "v4, Jordan's edits of 8 March on top of Sam's v3", "",
            "1. Revenue up 12% on Q4 (12.4%, from the closed books).",
            "2. Hiring plan: four engineers, two in Q2 and two in Q3.",
            "3. Risks: the Acme renewal (June) and one open senior role.",
            "4. Ask: approve the Q2 hiring budget."),
        f"{OWN}/Speaker notes.pdf": doc(
            "# Speaker notes", "Open with the revenue number, then the Acme risk."),
        f"{OWN}/Questions to expect.pdf": doc(
            "# Questions to expect", "Why two hires in Q3? What if Acme does not renew?"),
    }


STAMPS = {
    f"{SAM}/Board memo.pdf": "2026-03-04 17:20",
    f"{SAM}/Financials Q1.pdf": "2026-03-04 17:21",
    f"{SAM}/Agenda.pdf": "2026-03-04 17:22",
    f"{OWN}/Board memo.pdf": "2026-03-08 21:47",
    f"{OWN}/Speaker notes.pdf": "2026-03-08 22:05",
    f"{OWN}/Questions to expect.pdf": "2026-03-07 10:30",
}


def main(root: Path) -> None:
    home = root / "Users" / "admin"
    for rel, data in documents().items():
        S.put(home / rel, STAMPS[rel], data)
    S.stamp_dir(home / SAM, "2026-03-04 17:22")
    S.stamp_dir(home / OWN, "2026-03-08 22:05")
    S.stamp_dir(home / "Documents" / "Projects", "2026-03-08 22:05")
    for rel in documents():
        print(rel, (home / rel).stat().st_size)


if __name__ == "__main__":
    main(Path(sys.argv[1]))
