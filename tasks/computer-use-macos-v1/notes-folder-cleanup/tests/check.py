#!/usr/bin/env python3
"""notes-folder-cleanup checker: the Notes store copied from the stopped VM.

    check.py /var/lib/vbmac

Notes are found by title among the live notes (not in Recently Deleted).
An empty note is a live note with no text. Moving a note to Recently
Deleted counts as deleting it.

Sub-metrics (reward is 1 only when all are 1):
  artifact_present  the Notes store was collected
  recipes_filed     the four recipes are in Recipes
  travel_filed      the four travel notes (the Tokyo restaurant list among
                    them) are in Travel
  work_filed        the four work notes are in Work
  empties_deleted   no empty note is left outside Recently Deleted
  others_in_place   the five other notes are still in Notes, and no filed
                    note was deleted or duplicated
  ui_path           the MCP server logged pointer input
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, "/opt/vbdesk")
import cu_check as cu  # noqa: E402
import mac_state as M  # noqa: E402

METRICS = (
    "artifact_present",
    "recipes_filed",
    "travel_filed",
    "work_filed",
    "empties_deleted",
    "others_in_place",
    "ui_path",
)
FILE = {
    "recipes_filed": ("Recipes", ["Lemon ricotta pancakes", "Weeknight chili", "Grandma's banana bread", "Miso salmon"]),
    "travel_filed": ("Travel", ["Tokyo restaurant list", "Packing list: Japan", "Lisbon trip ideas", "Rental car confirmation"]),
    "work_filed": ("Work", ["1:1 notes with Priya", "Q2 hiring plan", "Offsite agenda draft", "Vendor contract questions"]),
}
STAY = ["Gift ideas for Mom", "Dinner party guest list", "Books to read", "Garden planting schedule", "Wedding RSVP list"]


def norm(t: str) -> str:
    return " ".join(t.replace("’", "'").split())


def main(argv: list[str]) -> int:
    root = Path(argv[1])
    export = root / "export"
    fs = export / "fs"
    m = {k: 0 for k in METRICS}
    extra: dict[str, object] = {"shutdown": M.manifest(export).get("shutdown", "none")}
    if not (fs / M.NOTES_DB).is_file():
        cu.emit(m, METRICS, **extra)
        return 0
    m["artifact_present"] = 1
    live = M.live_notes(fs)
    by_title: dict[str, list[M.Note]] = {}
    for n in live:
        by_title.setdefault(norm(n.title), []).append(n)
    placed = {}
    for key, (folder, titles) in FILE.items():
        ok = all(len(by_title.get(t, [])) == 1 and by_title[t][0].folder == folder for t in titles)
        m[key] = int(ok)
        placed[key] = {t: [n.folder for n in by_title.get(t, [])] for t in titles}
    extra["placed"] = placed
    empties = [n for n in live if not n.text.strip()]
    extra["empty_left"] = len(empties)
    m["empties_deleted"] = int(not empties)
    stay_ok = all(len(by_title.get(t, [])) == 1 and by_title[t][0].folder == "Notes" for t in STAY)
    extra["stay"] = {t: [n.folder for n in by_title.get(t, [])] for t in STAY}
    m["others_in_place"] = int(stay_ok)
    events_log = M.input_events(root / "input.log")
    m["ui_path"] = int(bool({e.get("kind") for e in events_log} & {"click", "drag"}))
    extra["input_events"] = len(events_log)
    cu.emit(m, METRICS, **extra)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
