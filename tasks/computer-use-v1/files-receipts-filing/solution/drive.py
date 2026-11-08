#!/usr/bin/env python3
"""Gold trajectory for files-receipts-filing: Thunar driven through the keyboard.

Every step is real input on the display (xdotool through XTEST), so the
input recorder sees it: the location entry (Ctrl+L) to change folder, type-
ahead find to select a file, F2 to rename, Ctrl+X and Ctrl+V to move,
Ctrl+Shift+N for the one month folder that does not exist yet.

VB_SKIP=<file name>       leave that receipt in Downloads (partial check)
VB_ALSO_MOVE=<file name>  also file that non-receipt (guard check)
VB_NO_RENAME=1            move without renaming (convention check)
"""

from __future__ import annotations

import json
import os
import subprocess
import time
from pathlib import Path

HOME = Path("/root")
SEED = json.loads(Path("/opt/filing/seed.json").read_text())
ENV = {**os.environ, "DISPLAY": ":1"}


def xdo(*args: str) -> None:
    subprocess.run(["xdotool", *args], check=True, env=ENV)


def key(*keys: str, pause: float = 0.35) -> None:
    xdo("key", "--clearmodifiers", "--delay", "80", *keys)
    time.sleep(pause)


def type_text(text: str, pause: float = 0.3) -> None:
    xdo("type", "--delay", "35", "--", text)
    time.sleep(pause)


def focus_thunar() -> None:
    out = subprocess.run(["wmctrl", "-l"], capture_output=True, text=True, env=ENV).stdout
    wid = next((line.split()[0] for line in out.splitlines() if "Thunar" in line), None)
    if wid is None:
        raise SystemExit("gold: no Thunar window")
    subprocess.run(["wmctrl", "-i", "-a", wid], env=ENV, check=False)
    time.sleep(0.5)


def goto(path: str) -> None:
    key("ctrl+l")
    key("ctrl+a", pause=0.1)
    type_text(path)
    key("Return", pause=0.9)


def select_by_name(name: str) -> None:
    """Type-ahead find: typing in the file list selects the first match."""
    key("Escape", pause=0.2)
    type_text(name, pause=0.6)
    key("Escape", pause=0.3)


def rename_selected(new_name: str) -> None:
    key("F2", pause=0.8)
    key("ctrl+a", pause=0.15)
    type_text(new_name)
    key("Return", pause=0.9)


def move_selected_to(folder: str) -> None:
    key("ctrl+x", pause=0.4)
    goto(folder)
    key("ctrl+v", pause=1.2)


def new_folder(parent: str, name: str) -> None:
    goto(parent)
    key("ctrl+shift+n", pause=0.9)
    type_text(name)
    key("Return", pause=0.9)


def main() -> None:
    skip = os.environ.get("VB_SKIP", "")
    also = os.environ.get("VB_ALSO_MOVE", "")
    no_rename = bool(os.environ.get("VB_NO_RENAME"))
    focus_thunar()
    # read the convention first, as a person would (opens Mousepad, then closes it)
    goto(str(HOME / "Finance"))
    select_by_name("NAMING.txt")
    key("Return", pause=2.0)
    key("ctrl+q", pause=1.0)
    focus_thunar()

    jobs = [e for e in SEED["downloads"] if e["dest"] and e["name"] != skip]
    if also:
        jobs.append({"name": also, "dest": f"Finance/2026/02-February/{also}", "no_rename": True})
    needed_dirs = sorted({str((HOME / e["dest"]).parent) for e in jobs})
    for d in needed_dirs:
        if not Path(d).is_dir():
            new_folder(str(Path(d).parent), Path(d).name)
    for e in jobs:
        goto(str(HOME / "Downloads"))
        select_by_name(e["name"])
        dest = HOME / e["dest"]
        if not (no_rename or e.get("no_rename")):
            rename_selected(dest.name)
        move_selected_to(str(dest.parent))
    goto(str(HOME / "Finance"))
    time.sleep(1.0)
    out_dir = Path("/logs/agent") if Path("/logs/agent").is_dir() else Path("/tmp")
    subprocess.run(["import", "-window", "root", str(out_dir / "gold-final.png")], env=ENV, check=False)
    filed = sum(1 for e in jobs if (HOME / e["dest"]).is_file() or (no_rename and (HOME / e["dest"]).parent.joinpath(e["name"]).is_file()))
    print(f"gold: filed {filed} of {len(jobs)}")


if __name__ == "__main__":
    main()
