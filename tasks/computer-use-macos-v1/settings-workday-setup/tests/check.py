#!/usr/bin/env python3
"""settings-workday-setup checker: the user's preference files copied from the
stopped VM (cfprefsd writes them out on a clean shutdown).

    check.py /var/lib/vbmac

Sub-metrics (reward is 1 only when all are 1):
  artifact_present     the Dock preferences were collected
  dock_left            com.apple.dock orientation is "left"
  dock_autohide        com.apple.dock autohide is on
  no_recents           com.apple.dock show-recents is off
  clock_24h            AppleICUForce24HourTime is on (global domain)
  wallpaper_click      WindowManager EnableStandardClickToShowDesktop is off
                       ("Only in Stage Manager")
  neighbours_unchanged Dock size, magnification, minimize-into-app-icon and
                       Stage Manager itself are as they started
  ui_path              the MCP server logged pointer input
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, "/opt/vbdesk")
import cu_check as cu  # noqa: E402
import mac_state as M  # noqa: E402

METRICS = (
    "artifact_present", "dock_left", "dock_autohide", "no_recents", "clock_24h",
    "wallpaper_click", "neighbours_unchanged", "ui_path",
)
PREFS = f"{M.HOME}/Library/Preferences"


def flag(d: dict, key: str, default: bool) -> bool:  # type: ignore[type-arg]
    v = d.get(key, default)
    return bool(v) if not isinstance(v, str) else v.lower() in ("1", "true", "yes")


def main(argv: list[str]) -> int:
    root = Path(argv[1])
    export = root / "export"
    prefs = export / "fs" / PREFS
    m = {k: 0 for k in METRICS}
    extra: dict[str, object] = {"shutdown": M.manifest(export).get("shutdown", "none")}
    dock_p = prefs / "com.apple.dock.plist"
    if not dock_p.is_file():
        cu.emit(m, METRICS, **extra)
        return 0
    m["artifact_present"] = 1
    dock = M.read_plist(dock_p)
    glob = M.read_plist(prefs / ".GlobalPreferences.plist") if (prefs / ".GlobalPreferences.plist").is_file() else {}
    wm = M.read_plist(prefs / "com.apple.WindowManager.plist") if (prefs / "com.apple.WindowManager.plist").is_file() else {}
    extra["dock"] = {k: dock.get(k) for k in ("orientation", "autohide", "show-recents", "tilesize", "magnification", "minimize-to-application")}
    extra["clock24"] = glob.get("AppleICUForce24HourTime")
    extra["wm"] = {k: wm.get(k) for k in ("EnableStandardClickToShowDesktop", "GloballyEnabled")}
    m["dock_left"] = int(dock.get("orientation") == "left")
    m["dock_autohide"] = int(flag(dock, "autohide", False))
    m["no_recents"] = int(not flag(dock, "show-recents", True))
    m["clock_24h"] = int(flag(glob, "AppleICUForce24HourTime", False))
    m["wallpaper_click"] = int(not flag(wm, "EnableStandardClickToShowDesktop", True))
    m["neighbours_unchanged"] = int(
        dock.get("tilesize") == 48
        and not flag(dock, "magnification", False)
        and not flag(dock, "minimize-to-application", False)
        and not flag(wm, "GloballyEnabled", False)
    )
    events = M.input_events(root / "input.log")
    kinds = {e.get("kind") for e in events}
    m["ui_path"] = int(bool(kinds & {"click"}))
    extra["input_events"] = len(events)
    cu.emit(m, METRICS, **extra)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
