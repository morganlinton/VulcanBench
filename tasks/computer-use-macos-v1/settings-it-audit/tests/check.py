#!/usr/bin/env python3
"""settings-it-audit checker: preference files copied from the stopped VM.

    check.py /var/lib/vbmac

The checklist (Documents/IT setup checklist.pdf) for a Sales user, with
Revisions B and C:
  changed: 1 Dark (AppleInterfaceStyle), 2 scroll bars Always, 4 Scale
    minimize effect, 6 prefer tabs Always (AppleWindowTabbingMode always),
    7 ask to keep changes (NSCloseAlwaysConfirmsChanges), 9 double-click title
    bar Minimize (AppleActionOnDoubleClick), 10 24-hour clock, 11 clock seconds
    (menuextra.clock ShowSeconds), 12 all extensions, 13 no extension-change
    warning, 15 path bar, 17 folders on top (_FXSortFoldersFirst), 18 search
    the current folder (FXDefaultSearchScope SCcf), 19 key repeat one step
    below fastest (KeyRepeat 6, Revision C), 20 spelling correction off,
    21 week starts Monday, 23 click wallpaper only in Stage Manager
    (WindowManager EnableStandardClickToShowDesktop false), S1 bottom-right
    hot corner Desktop (wvous-br-corner 4)
  already true: 3 Dock auto-hide, 8 close windows when quitting, 14 new
    windows show Documents, 16 status bar, 22 Spaces not rearranged, S2 Stage
    Manager off
  reversed by Revision B: 5 recent apps stay in the Dock
  Finance only, not for this user: F1 hot corner Lock Screen (13), F2 smart
    quotes off

Sub-metrics (reward is 1 only when all are 1):
  artifact_present      the Dock and global preference files were collected
  applied_windows       items 1, 2, 4, 6, 7 and 9
  applied_clock_input   items 10, 11, 20 and 21
  applied_finder        items 12, 13, 15, 17 and 18
  applied_desktop       item 23 and S1 (bottom-right hot corner Desktop)
  revision_b            recent apps still show in the Dock
  revision_c            key repeat is one step below fastest (6), not fastest (2)
  team_followed         no Finance item applied: the hot corner is not Lock
                        Screen and smart quotes are still on
  kept_compliant        items 3, 8, 14, 16, 22 and S2 still hold
  neighbours_unchanged  Dock at the bottom, size 48, no magnification, no accent
                        color, initial key repeat delay 25, smart dashes on, no
                        other hot corner set
  ui_path               the MCP server logged pointer input
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, "/opt/vbdesk")
import cu_check as cu  # noqa: E402
import mac_state as M  # noqa: E402

METRICS = (
    "artifact_present",
    "applied_windows",
    "applied_clock_input",
    "applied_finder",
    "applied_desktop",
    "revision_b",
    "revision_c",
    "team_followed",
    "kept_compliant",
    "neighbours_unchanged",
    "ui_path",
)
PREFS = f"{M.HOME}/Library/Preferences"


def flag(d: dict, key: str, default: bool) -> bool:  # type: ignore[type-arg]
    v = d.get(key, default)
    return bool(v) if not isinstance(v, str) else v.lower() in ("1", "true", "yes")


def plist(p: Path) -> dict:  # type: ignore[type-arg]
    return M.read_plist(p) if p.is_file() else {}


def num(d: dict, key: str) -> int | None:  # type: ignore[type-arg]
    v = d.get(key)
    try:
        return int(v) if v is not None else None
    except (TypeError, ValueError):
        return None


def main(argv: list[str]) -> int:
    root = Path(argv[1])
    export = root / "export"
    prefs = export / "fs" / PREFS
    m = {k: 0 for k in METRICS}
    extra: dict[str, object] = {"shutdown": M.manifest(export).get("shutdown", "none")}
    if not (prefs / "com.apple.dock.plist").is_file() or not (prefs / ".GlobalPreferences.plist").is_file():
        cu.emit(m, METRICS, **extra)
        return 0
    m["artifact_present"] = 1
    dock = plist(prefs / "com.apple.dock.plist")
    glob = plist(prefs / ".GlobalPreferences.plist")
    finder = plist(prefs / "com.apple.finder.plist")
    clock = plist(prefs / "com.apple.menuextra.clock.plist")
    wm = plist(prefs / "com.apple.WindowManager.plist")
    week = glob.get("AppleFirstWeekday") or {}
    keys = {
        "dock": ("autohide", "show-recents", "mineffect", "mru-spaces", "tilesize", "orientation", "magnification",
                 "wvous-tl-corner", "wvous-tr-corner", "wvous-bl-corner", "wvous-br-corner"),
        "glob": ("AppleInterfaceStyle", "AppleShowScrollBars", "AppleWindowTabbingMode", "NSCloseAlwaysConfirmsChanges",
                 "NSQuitAlwaysKeepsWindows", "AppleActionOnDoubleClick", "AppleICUForce24HourTime", "AppleShowAllExtensions",
                 "KeyRepeat", "InitialKeyRepeat", "NSAutomaticSpellingCorrectionEnabled",
                 "NSAutomaticQuoteSubstitutionEnabled", "NSAutomaticDashSubstitutionEnabled", "AppleFirstWeekday",
                 "AppleAccentColor"),
        "finder": ("FXEnableExtensionChangeWarning", "NewWindowTarget", "ShowPathbar", "ShowStatusBar",
                   "_FXSortFoldersFirst", "FXDefaultSearchScope"),
        "clock": ("ShowSeconds",),
        "wm": ("GloballyEnabled", "EnableStandardClickToShowDesktop"),
    }  # fmt: skip
    src = {"dock": dock, "glob": glob, "finder": finder, "clock": clock, "wm": wm}
    extra["state"] = {f"{s}.{k}": src[s].get(k) for s, ks in keys.items() for k in ks}
    m["applied_windows"] = int(
        glob.get("AppleInterfaceStyle") == "Dark"
        and glob.get("AppleShowScrollBars") == "Always"
        and dock.get("mineffect") == "scale"
        and glob.get("AppleWindowTabbingMode") == "always"
        and flag(glob, "NSCloseAlwaysConfirmsChanges", False)
        and glob.get("AppleActionOnDoubleClick") == "Minimize"
    )
    m["applied_clock_input"] = int(
        flag(glob, "AppleICUForce24HourTime", False)
        and flag(clock, "ShowSeconds", False)
        and not flag(glob, "NSAutomaticSpellingCorrectionEnabled", True)
        and isinstance(week, dict)
        and week.get("gregorian") == 2
    )
    m["applied_finder"] = int(
        flag(glob, "AppleShowAllExtensions", False)
        and not flag(finder, "FXEnableExtensionChangeWarning", True)
        and flag(finder, "ShowPathbar", False)
        and flag(finder, "_FXSortFoldersFirst", False)
        and finder.get("FXDefaultSearchScope") == "SCcf"
    )
    m["applied_desktop"] = int(
        not flag(wm, "EnableStandardClickToShowDesktop", True) and num(dock, "wvous-br-corner") == 4
    )
    m["revision_b"] = int(flag(dock, "show-recents", True))
    m["revision_c"] = int(num(glob, "KeyRepeat") == 6)
    m["team_followed"] = int(
        num(dock, "wvous-br-corner") != 13 and flag(glob, "NSAutomaticQuoteSubstitutionEnabled", True)
    )
    m["kept_compliant"] = int(
        flag(dock, "autohide", False)
        and not flag(glob, "NSQuitAlwaysKeepsWindows", False)
        and finder.get("NewWindowTarget") == "PfDo"
        and flag(finder, "ShowStatusBar", False)
        and not flag(dock, "mru-spaces", True)
        and not flag(wm, "GloballyEnabled", False)
    )
    m["neighbours_unchanged"] = int(
        dock.get("orientation", "bottom") == "bottom"
        and num(dock, "tilesize") == 48
        and not flag(dock, "magnification", False)
        and "AppleAccentColor" not in glob
        and num(glob, "InitialKeyRepeat") == 25
        and flag(glob, "NSAutomaticDashSubstitutionEnabled", True)
        and all(num(dock, f"wvous-{c}-corner") in (None, 0, 1) for c in ("tl", "tr", "bl"))
    )
    events = M.input_events(root / "input.log")
    m["ui_path"] = int(bool({e.get("kind") for e in events} & {"click", "drag"}))
    extra["input_events"] = len(events)
    cu.emit(m, METRICS, **extra)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
