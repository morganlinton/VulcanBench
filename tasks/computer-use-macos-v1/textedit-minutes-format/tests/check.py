#!/usr/bin/env python3
"""textedit-minutes-format checker: ~/Documents copied from the stopped VM.

    check.py /var/lib/vbmac

The minutes are read paragraph by paragraph (mac_state.rtf_paragraphs):
text, whether every visible character is bold, the point sizes used, and
whether the paragraph is an item of a TextEdit list. Lines typed with a
leading "-" or "*" are not a list.

Sub-metrics (reward is 1 only when all are 1):
  artifact_present  ~/Documents was collected
  saved_rtf         "Minutes 2026-03-09.rtf" is still an RTF file with the
                    same ten lines of text in the same order
  heading           the first line is bold and 18 point
  section_labels    "Decisions" and "Action items" are bold (and not list items)
  bulleted          the two decisions and three action items are list items
  rest_plain        the attendees and next-meeting lines are not bold, not
                    list items, and still 12 point; the items are not bold
  others_untouched  the older minutes file is unchanged
  ui_path           the MCP server logged pointer input
"""

from __future__ import annotations

import hashlib
import sys
from pathlib import Path

sys.path.insert(0, "/opt/vbdesk")
import cu_check as cu  # noqa: E402
import mac_state as M  # noqa: E402

METRICS = (
    "artifact_present",
    "saved_rtf",
    "heading",
    "section_labels",
    "bulleted",
    "rest_plain",
    "others_untouched",
    "ui_path",
)
DOC = "Minutes 2026-03-09.rtf"
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
ITEMS = (3, 4, 6, 7, 8)
OLD_SHA = "349bc634ec289bebb78f22811e4b2be40b8a658267fd0d8a525979973994c218"


def main(argv: list[str]) -> int:
    root = Path(argv[1])
    export = root / "export"
    docs = export / "fs" / M.HOME / "Documents"
    m = {k: 0 for k in METRICS}
    extra: dict[str, object] = {"shutdown": M.manifest(export).get("shutdown", "none")}
    if not docs.is_dir():
        cu.emit(m, METRICS, **extra)
        return 0
    m["artifact_present"] = 1
    f = docs / DOC
    paras: list[M.Para] = []
    if f.is_file() and f.read_bytes()[:6] == b"{\\rtf1":
        try:
            paras = M.rtf_paragraphs(f)
        except Exception:  # noqa: BLE001 - unreadable is "not saved"
            paras = []
    extra["paragraphs"] = [(p.text, p.bold, sorted(p.sizes), p.in_list) for p in paras]
    if [p.text for p in paras] == LINES:
        m["saved_rtf"] = 1
        h = paras[0]
        m["heading"] = int(h.bold and h.sizes == {18.0} and not h.in_list)
        m["section_labels"] = int(all(paras[i].bold and not paras[i].in_list for i in (2, 5)))
        m["bulleted"] = int(all(paras[i].in_list for i in ITEMS))
        m["rest_plain"] = int(
            all(not paras[i].bold and not paras[i].in_list and paras[i].sizes == {12.0} for i in (1, 9))
            and all(not paras[i].bold for i in ITEMS)
        )
    old = docs / "Minutes 2026-03-02.rtf"
    m["others_untouched"] = int(old.is_file() and hashlib.sha256(old.read_bytes()).hexdigest() == OLD_SHA)
    events_log = M.input_events(root / "input.log")
    m["ui_path"] = int(bool({e.get("kind") for e in events_log} & {"click", "drag"}))
    extra["input_events"] = len(events_log)
    cu.emit(m, METRICS, **extra)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
