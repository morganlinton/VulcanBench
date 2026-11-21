#!/usr/bin/env python3
"""settings-corners-shortcuts checker: the Dock and Notes preference files
and the text replacement store copied from the stopped VM.

    check.py /var/lib/vbmac

Hot corners live in com.apple.dock as wvous-<corner>-corner (the action:
2 Mission Control, 12 Notification Center, 13 Lock Screen, 14 Quick Note)
and wvous-<corner>-modifier (0 none, 1048576 Command). App shortcuts are the
app's NSUserKeyEquivalents ('@' Command, '^' Control, '~' Option, '$' Shift);
text replacements are rows of KeyboardServices/TextReplacements.db.

Sub-metrics (reward is 1 only when all are 1):
  artifact_present    the Dock preferences were collected
  lock_corner         bottom-right: Lock Screen, no modifier
  mission_cmd_corner  top-left: Mission Control with Command held
  notes_shortcut      Notes' "Checklist" is Control-Command-L
  text_replacement    "addr;" expands to "1 Harbor Way, Boston, MA 02110"
  scale_minimize      the Dock minimizes with the scale effect
  others_untouched    top-right still Notification Center with no modifier,
                      bottom-left still empty, and "omw" still expands
  ui_path             the MCP server logged pointer input
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, "/opt/vbdesk")
import cu_check as cu  # noqa: E402
import mac_state as M  # noqa: E402

METRICS = (
    "artifact_present",
    "lock_corner",
    "mission_cmd_corner",
    "notes_shortcut",
    "text_replacement",
    "scale_minimize",
    "others_untouched",
    "ui_path",
)
PREFS = f"{M.HOME}/Library/Preferences"
CMD = 1048576
ADDRESS = "1 Harbor Way, Boston, MA 02110"


def shortcut(s: str) -> tuple[frozenset[str], str]:
    """'@^l' and '^@l' are the same shortcut: modifiers as a set, then the key."""
    mods = frozenset(ch for ch in s if ch in "@^~$")
    key = "".join(ch for ch in s if ch not in "@^~$").lower()
    return mods, key


def main(argv: list[str]) -> int:
    root = Path(argv[1])
    export = root / "export"
    fs = export / "fs"
    m = {k: 0 for k in METRICS}
    extra: dict[str, object] = {"shutdown": M.manifest(export).get("shutdown", "none")}
    dock_p = fs / PREFS / "com.apple.dock.plist"
    if not dock_p.is_file():
        cu.emit(m, METRICS, **extra)
        return 0
    m["artifact_present"] = 1
    dock = M.read_plist(dock_p)
    corners = {
        c: (dock.get(f"wvous-{c}-corner"), dock.get(f"wvous-{c}-modifier", 0)) for c in ("tl", "tr", "bl", "br")
    }
    extra["corners"] = corners
    m["lock_corner"] = int(corners["br"] == (13, 0))
    m["mission_cmd_corner"] = int(corners["tl"] == (2, CMD))
    notes = M.app_shortcuts(fs, "com.apple.Notes")
    extra["notes_shortcuts"] = notes
    m["notes_shortcut"] = int(
        "Checklist" in notes and shortcut(str(notes["Checklist"])) == (frozenset("@^"), "l")
    )
    repl = M.text_replacements(fs)
    extra["replacements"] = repl
    m["text_replacement"] = int(" ".join(repl.get("addr;", "").split()) == ADDRESS)
    extra["mineffect"] = dock.get("mineffect")
    m["scale_minimize"] = int(dock.get("mineffect") == "scale")
    bl_empty = corners["bl"][0] in (None, 0, 1)  # 1 is "-" (no action)
    omw_ok = not repl or repl.get("omw") == "On my way!"
    m["others_untouched"] = int(corners["tr"] == (12, 0) and bl_empty and omw_ok)
    events_log = M.input_events(root / "input.log")
    m["ui_path"] = int(bool({e.get("kind") for e in events_log} & {"click", "drag"}))
    extra["input_events"] = len(events_log)
    cu.emit(m, METRICS, **extra)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
