"""settings-it-audit seed, run on the host against the task VM's Data volume
(build_task_image.py): python3 seed_fs.py <data-volume-root>.

~/Documents/IT setup checklist.pdf: 23 settings for all staff, a Sales
section and a Finance section that disagree about the bottom-right hot
corner, and two revisions at the bottom: B reverses item 5 (keep recent apps
in the Dock) and C changes item 19's key repeat rate to one step below the
fastest. The user is on the Sales team (instruction.md).
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
    "Dock: minimize windows using the Scale effect.",
    "Dock: do not show suggested and recent apps.",
    "Prefer tabs when opening documents: Always.",
    "Ask to keep changes when closing documents: on.",
    "Close windows when quitting an application: on.",
    "Double-click a window's title bar to: Minimize.",
    "Menu bar clock: use a 24-hour clock.",
    "Menu bar clock: display the time with seconds.",
    "Finder: show all filename extensions.",
    "Finder: do not show a warning before changing an extension.",
    "Finder: new Finder windows show Documents.",
    "Finder: show the path bar.",
    "Finder: show the status bar.",
    "Finder: keep folders on top in windows when sorting by name.",
    "Finder: when performing a search, search the current folder.",
    "Keyboard: key repeat rate at the fastest setting.",
    "Keyboard text input: turn off \"Correct spelling automatically\".",
    "Language & Region: first day of the week is Monday.",
    "Mission Control: do not rearrange Spaces automatically based on most recent use.",
    "Desktop & Dock: click wallpaper to reveal desktop: only in Stage Manager.",
]
SALES = ["S1. Hot corner, bottom right: Desktop.", "S2. Stage Manager: off."]
FINANCE = ["F1. Hot corner, bottom right: Lock Screen.", "F2. Keyboard text input: turn off \"Use smart quotes and dashes\"."]


def main(root: Path) -> None:
    it: list[object] = [
        L(72, 750, "Harbor Analytics: Mac setup checklist", 18, bold=True),
        L(72, 730, "Revision A, 16 February 2026. Items 1 to 23 are for all staff; then your team's section only.", 9, gray=0.3),
        L(72, 718, "Leave every other setting as it is.", 9, gray=0.3),
        S.Rule(72, 708, 540, 708),
    ]
    y = 690.0
    for i, t in enumerate(ITEMS, 1):
        it.append(L(72, y, f"{i}.", 9, bold=True))
        it.append(L(92, y, t, 9))
        y -= 17
    y -= 6
    it.append(L(72, y, "Sales team only", 10, bold=True))
    for t in SALES:
        y -= 15
        it.append(L(92, y, t, 9))
    y -= 21
    it.append(L(72, y, "Finance team only", 10, bold=True))
    for t in FINANCE:
        y -= 15
        it.append(L(92, y, t, 9))
    y -= 14
    it.append(S.Rule(72, y, 540, y))
    y -= 16
    it.append(L(72, y, "Revision B, 2 March 2026", 10, bold=True))
    it.append(L(72, y - 14, "Item 5 is reversed: the help desk now relies on recent apps, so keep suggested and recent apps", 9))
    it.append(L(72, y - 26, "showing in the Dock.", 9))
    y -= 46
    it.append(L(72, y, "Revision C, 6 March 2026", 10, bold=True))
    it.append(L(72, y - 14, "Item 19 is changed: set the key repeat rate one step below the fastest setting.", 9))
    it.append(L(72, y - 26, "Where revisions disagree with Revision A, the later revision wins.", 9))
    it.append(L(72, y - 50, "Questions: it-help@harboranalytics.example", 9, gray=0.3))
    S.put(root / "Users" / "admin" / "Documents" / "IT setup checklist.pdf", "2026-03-06 15:10", S.pdf([it]))
    print("seeded checklist")


if __name__ == "__main__":
    main(Path(sys.argv[1]))
