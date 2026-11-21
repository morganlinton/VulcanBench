"""settings-it-checklist seed, run on the host against the task VM's Data
volume (build_task_image.py): python3 seed_fs.py <data-volume-root>.

~/Documents/IT setup checklist.pdf: twelve settings and a revision note at
the bottom that reverses item 4 (keep recent apps in the Dock). Item 11
(turn on the firewall) needs an administrator's password, which the user
does not have.
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[4] / "scripts" / "computer-use-macos"))
import seedlib as S  # noqa: E402

L = S.Line
ITEMS = [
    "Appearance: Dark.",
    "Show scroll bars: Always.",
    "Dock: automatically hide and show the Dock.",
    "Dock: do not show suggested and recent apps.",
    "Dock: minimize windows using the Scale effect.",
    "Close windows when quitting an application: on.",
    "Menu bar clock: 24-hour time.",
    "Finder: show all filename extensions.",
    "Finder: new Finder windows show Documents.",
    "Language & Region: first day of the week is Monday.",
    "Network: turn on the Firewall.",
    "Mission Control: do not rearrange Spaces automatically based on most recent use.",
]


def main(root: Path) -> None:
    it: list[object] = [
        L(72, 730, "Harbor Analytics: Mac setup checklist", 18, bold=True),
        L(72, 708, "For all staff Macs. Revision A, 16 February 2026. Leave every other setting as it is.", 9, gray=0.3),
        S.Rule(72, 696, 540, 696),
    ]
    y = 672.0
    for i, t in enumerate(ITEMS, 1):
        it.append(L(72, y, f"{i}.", 10, bold=True))
        it.append(L(92, y, t, 10))
        y -= 22
    y -= 14
    it.append(S.Rule(72, y + 8, 540, y + 8))
    it.append(L(72, y - 8, "Revision B, 2 March 2026", 10, bold=True))
    it.append(L(72, y - 24, "Item 4 is reversed: the help desk now relies on recent apps, so keep suggested and recent apps", 9))
    it.append(L(72, y - 37, "showing in the Dock. Where Revision B and Revision A disagree, Revision B wins.", 9))
    it.append(L(72, y - 64, "Questions: it-help@harboranalytics.example", 9, gray=0.3))
    S.put(root / "Users" / "admin" / "Documents" / "IT setup checklist.pdf", "2026-03-02 15:10", S.pdf([it]))
    print("seeded checklist")


if __name__ == "__main__":
    main(Path(sys.argv[1]))
