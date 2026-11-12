#!/usr/bin/env python3
"""notes-actions-to-reminders checker: the Reminders and Notes stores copied
from the stopped VM. Today in the guest is Monday 9 March 2026.

    check.py /var/lib/vbmac

Sub-metrics (reward is 1 only when all are 1):
  artifact_present    the Reminders store was collected
  list_created        a list named "Follow-ups" exists
  items_found         each of Jordan's five action items has exactly one
                      reminder in Follow-ups (matched by its key words)
  due_dates           each of those reminders is due on the day the note gives
  only_mine           Follow-ups holds those five and nothing else: no item
                      assigned to someone else, the shared item Marcus owns,
                      the item already done, or last week's note
  personal_untouched  the Personal list keeps its two reminders, unchanged
  notes_untouched     the three meeting notes are still there
  ui_path             the MCP server logged typing and pointer input
"""

from __future__ import annotations

import sys
from datetime import date
from pathlib import Path

sys.path.insert(0, "/opt/vbdesk")
import cu_check as cu  # noqa: E402
import mac_state as M  # noqa: E402

METRICS = (
    "artifact_present", "list_created", "items_found", "due_dates", "only_mine",
    "personal_untouched", "notes_untouched", "ui_path",
)
# (name, any of these key phrases in the title, due day)
TARGETS = [
    ("funnel", ("funnel",), date(2026, 3, 12)),  # by Thursday
    ("usability", ("usability",), date(2026, 3, 16)),  # next Monday
    ("interview feedback", ("interview feedback", "feedback"), date(2026, 3, 10)),  # end of day tomorrow
    ("debrief", ("debrief",), date(2026, 3, 18)),  # Wednesday 18 March
    ("ssl", ("ssl", "certificate", "cert"), date(2026, 3, 20)),  # by the 20th
]
DISTRACTORS = (
    "trial", "expiry", "pricing", "memo", "designer", "rubric", "incident", "archive",
    "on-call", "on call", "churn", "dashboard",
)
PERSONAL = {"Pick up dry cleaning": date(2026, 3, 10), "Call the dentist": None}
MEETINGS = {"Ops weekly", "Product sync", "Hiring review"}


def matches(title: str, keys: tuple[str, ...]) -> bool:
    t = title.lower()
    return any(k in t for k in keys)


def main(argv: list[str]) -> int:
    root = Path(argv[1])
    export = root / "export"
    fs = export / "fs"
    m = {k: 0 for k in METRICS}
    extra: dict[str, object] = {"shutdown": M.manifest(export).get("shutdown", "none")}
    if not (fs / M.REMINDERS_STORES).is_dir():
        cu.emit(m, METRICS, **extra)
        return 0
    m["artifact_present"] = 1
    rems = [r for r in M.reminders(fs) if not r.deleted]
    m["list_created"] = int("Follow-ups" in M.reminder_lists(fs))
    follow = [r for r in rems if r.list == "Follow-ups"]
    extra["follow_ups"] = [(r.title, str(r.due_date), r.completed) for r in follow]
    found, dates_ok = True, True
    claimed: set[int] = set()
    for _name, keys, due in TARGETS:
        hits = [i for i, r in enumerate(follow) if matches(r.title, keys) and i not in claimed]
        if len(hits) != 1:
            found = False
            continue
        claimed.add(hits[0])
        if follow[hits[0]].due_date != due:
            dates_ok = False
    m["items_found"] = int(found)
    m["due_dates"] = int(found and dates_ok)
    stray = [r.title for i, r in enumerate(follow) if i not in claimed]
    m["only_mine"] = int(
        found and not stray and not any(matches(r.title, DISTRACTORS) for r in follow)
    )
    extra["stray"] = stray
    personal = [r for r in rems if r.list == "Personal"]
    m["personal_untouched"] = int(
        len(personal) == len(PERSONAL)
        and all(
            r.title in PERSONAL and r.due_date == PERSONAL[r.title] and not r.completed
            for r in personal
        )
    )
    if (fs / M.NOTES_DB).is_file():
        live = M.live_notes(fs)
        m["notes_untouched"] = int(
            MEETINGS <= {n.title for n in live if n.folder == "Meetings"}
        )
    events = M.input_events(root / "input.log")
    kinds = {e.get("kind") for e in events}
    m["ui_path"] = int("type" in kinds and bool(kinds & {"click", "key"}))
    extra["input_events"] = len(events)
    cu.emit(m, METRICS, **extra)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
