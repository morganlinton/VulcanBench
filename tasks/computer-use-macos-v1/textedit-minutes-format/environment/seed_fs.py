"""textedit-minutes-format seed, run on the host against the task VM's Data
volume (build_task_image.py): python3 seed_fs.py <data-volume-root>.

Plain meeting minutes as TextEdit saves an unformatted rich-text document
(Helvetica 12, one paragraph per line) in ~/Documents, next to an older
formatted set of minutes that must not change.
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[4] / "scripts" / "computer-use-macos"))
import seedlib as S  # noqa: E402

LINES = [
    "Product sync, 9 March 2026",
    "Attendees: Jordan, Priya, Marcus, Elena",
    "Decisions",
    "Ship the onboarding redesign on 23 March.",
    "Keep the trial at 14 days.",
    "Action items",
    "Priya: update the pricing page copy.",
    "Marcus: fix the trial-expiry email.",
    "Elena: draft the Q2 pricing memo.",
    "Next meeting: Monday 16 March",
]
HEAD = (
    "{\\rtf1\\ansi\\ansicpg1252\\cocoartf2907\n"
    "\\cocoatextscaling0\\cocoaplatform0{\\fonttbl\\f0\\fswiss\\fcharset0 Helvetica;}\n"
    "{\\colortbl;\\red255\\green255\\blue255;}\n"
    "{\\*\\expandedcolortbl;;}\n"
    "\\paperw11900\\paperh16840\\margl1440\\margr1440\\vieww11520\\viewh8400\\viewkind0\n"
    "\\pard\\tx720\\tx1440\\tx2160\\tx2880\\tx3600\\tx4320\\tx5040\\tx5760\\tx6480\\tx7200\\tx7920\\tx8640\\pardirnatural\\partightenfactor0\n"
    "\n\\f0\\fs24 \\cf0 "
)


def rtf(lines: list[str]) -> bytes:
    return (HEAD + "\\\n".join(lines) + "}").encode("latin-1")


def main(root: Path) -> None:
    docs = root / "Users" / "admin" / "Documents"
    S.put(docs / "Minutes 2026-03-09.rtf", "2026-03-09 11:05", rtf(LINES))
    S.put(
        docs / "Minutes 2026-03-02.rtf",
        "2026-03-02 11:20",
        rtf(["Product sync, 2 March 2026", "Attendees: Jordan, Priya, Elena", "Decisions", "Pricing test runs to the 18th."]),
    )
    for p in sorted(docs.iterdir()):
        print(p.name, p.stat().st_size)


if __name__ == "__main__":
    main(Path(sys.argv[1]))
