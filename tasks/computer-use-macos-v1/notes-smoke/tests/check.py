#!/usr/bin/env python3
"""notes-smoke checker: reads the Notes store copied from the stopped VM.

    check.py /var/lib/vbmac

Sub-metrics (reward is 1 only when all are 1):
  artifact_present  the collected Notes store exists
  note_present      exactly one live note titled "Packing list"
  note_in_folder    it is in the Trips folder
  note_items        passport, phone charger and rain jacket, each on its own line
  seeded_untouched  the four seeded notes keep their folder and text
  no_extra_notes    nothing else was created
  ui_path           the MCP server logged typing and pointer input
Extras: shutdown (how the guest went down; "forced" means unsaved edits may be lost).
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, "/opt/vbdesk")
import cu_check as cu  # noqa: E402
import mac_state as M  # noqa: E402

METRICS = (
    "artifact_present", "note_present", "note_in_folder", "note_items",
    "seeded_untouched", "no_extra_notes", "ui_path",
)
SEEDED = {
    "Lisbon itinerary": ("Trips", ["Alfama walk", "tram 15"]),
    "Hotel confirmations": ("Trips", ["booking 48213"]),
    "Q2 planning": ("Work", ["hiring plan"]),
    "Groceries": ("Notes", ["oat milk"]),
}
ITEMS = ("passport", "phone charger", "rain jacket")


def main(argv: list[str]) -> int:
    root = Path(argv[1])
    export = root / "export"
    fs = export / "fs"
    m = {k: 0 for k in METRICS}
    man = M.manifest(export)
    extra: dict[str, object] = {"shutdown": man.get("shutdown", "none")}
    if not (fs / M.NOTES_DB).is_file():
        cu.emit(m, METRICS, **extra)
        return 0
    m["artifact_present"] = 1
    live = M.live_notes(fs)
    extra["notes"] = [(n.title, n.folder) for n in live]
    target = [n for n in live if n.title.strip() == "Packing list"]
    m["note_present"] = int(len(target) == 1)
    if len(target) == 1:
        note = target[0]
        m["note_in_folder"] = int(note.folder == "Trips")
        lines = [ln.strip().lower().strip("-*• ") for ln in note.body.splitlines() if ln.strip()]
        m["note_items"] = int(all(item in lines for item in ITEMS))
        extra["body_lines"] = lines
    seeded_ok = True
    for title, (folder, needles) in SEEDED.items():
        hits = [n for n in live if n.title.strip() == title]
        if len(hits) != 1 or hits[0].folder != folder or not all(s in hits[0].text for s in needles):
            seeded_ok = False
    m["seeded_untouched"] = int(seeded_ok)
    m["no_extra_notes"] = int(len(live) == len(SEEDED) + 1)
    events = M.input_events(root / "input.log")
    kinds = {e.get("kind") for e in events}
    m["ui_path"] = int("type" in kinds and bool(kinds & {"click", "key"}))
    extra["input_events"] = len(events)
    cu.emit(m, METRICS, **extra)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
