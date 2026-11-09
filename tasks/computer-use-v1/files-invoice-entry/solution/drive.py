#!/usr/bin/env python3
"""Gold trajectory for files-invoice-entry: three native apps through the keyboard.

Every step is real input on the display (xdotool through XTEST), so the
input recorder sees it. For each PDF in incoming: select it in Thunar with
Select by Pattern (Ctrl+S, the file name), open it with Return (Evince,
through the image's mimeapps), look at it, close it with Ctrl+W. Then in Calc: the Name Box to the first
empty row, type the five new rows, Ctrl+S. Then in Thunar: cut each
dealt-with file and paste it into entered. The gold knows the answers from
the task's own answer key; its purpose is a complete trajectory through all
three windows, not the reading.

VB_SKIP=<file>      do not enter that invoice and do not move it (partial)
VB_DOUBLE=1         also enter the Kestrel invoice a second time
VB_CREDIT=1         also enter the credit note as a row (negative amount)
VB_MOVE_CREDIT=1    also move the credit note into entered
VB_NET=1            enter the Kestrel invoice's net figure, not its total
VB_NO_SAVE=1        type the rows but do not save the ledger
VB_LEAVE_FILES=1    enter and save, move nothing
"""

from __future__ import annotations

import json
import os
import subprocess
import time
from pathlib import Path

KEY = json.loads(Path("/opt/invoices/invoices.json").read_text())
INCOMING = Path("/root/Invoices/incoming")
ENTERED = Path("/root/Invoices/entered")
ODS = Path("/root/Finance") / KEY["ledger_file"]
ENV = {**os.environ, "DISPLAY": ":1"}
NAME_BOX = (70, 145)  # Calc maximised at 1024x900 with the suite's profile
FIRST_FREE_ROW = len(KEY["ledger_rows"]) + 2


def xdo(*args: str) -> None:
    subprocess.run(["xdotool", *args], check=True, env=ENV)


def key(*keys: str, pause: float = 0.35) -> None:
    xdo("key", "--clearmodifiers", "--delay", "80", *keys)
    time.sleep(pause)


def type_text(text: str, pause: float = 0.3) -> None:
    xdo("type", "--delay", "35", "--", text)
    time.sleep(pause)


def windows() -> list[tuple[str, str]]:
    out = subprocess.run(["wmctrl", "-l"], capture_output=True, text=True, env=ENV).stdout
    res = []
    for line in out.splitlines():
        parts = line.split(None, 3)
        if len(parts) == 4:
            res.append((parts[0], parts[3]))
    return res


def focus(title_part: str) -> str:
    wid = next((w for w, t in windows() if title_part in t), None)
    if wid is None:
        raise SystemExit(f"gold: no window matching {title_part!r}")
    subprocess.run(["wmctrl", "-i", "-a", wid], env=ENV, check=False)
    time.sleep(0.7)
    return wid


def goto(path: str) -> None:
    key("ctrl+l")
    key("ctrl+a", pause=0.1)
    type_text(path)
    key("Return", pause=1.0)


LIST_BLANK = (600, 760)  # an empty spot in the file list: a click there focuses the view


def focus_list() -> None:
    """After a path is entered the location bar keeps the focus; the view needs it."""
    xdo("mousemove", str(LIST_BLANK[0]), str(LIST_BLANK[1]), "click", "--clearmodifiers", "1")
    time.sleep(0.4)


def select_by_name(name: str) -> None:
    """Edit > Select by Pattern with the exact file name (Thunar 4.18 sends plain
    typing in the view to the location bar, so type-ahead is not usable)."""
    focus_list()
    key("ctrl+s", pause=0.7)
    type_text(name, pause=0.3)
    key("Return", pause=0.7)


def open_and_close(entry: dict) -> None:
    before = {w for w, _ in windows()}
    focus("Thunar")
    goto(str(INCOMING))
    select_by_name(entry["file"])
    key("Return", pause=1.0)
    new = None
    for _ in range(60):
        new = next((w for w, _ in windows() if w not in before), None)
        if new:
            break
        time.sleep(0.5)
    if new is None:
        raise SystemExit(f"gold: Evince never opened {entry['file']}")
    time.sleep(1.5)
    subprocess.run(["wmctrl", "-i", "-a", new], env=ENV, check=False)
    time.sleep(0.5)
    key("Next", pause=0.6)  # look down the page, as a person would
    key("ctrl+w", pause=1.0)
    for _ in range(20):
        if new not in {w for w, _ in windows()}:
            break
        time.sleep(0.3)
    else:
        subprocess.run(["wmctrl", "-i", "-c", new], env=ENV, check=False)
        time.sleep(1.0)


def goto_cell(ref: str) -> None:
    xdo("mousemove", str(NAME_BOX[0]), str(NAME_BOX[1]), "click", "--clearmodifiers", "1")
    time.sleep(0.4)
    key("ctrl+a", pause=0.15)
    type_text(ref)
    key("Return", pause=0.6)


def type_row(row_no: int, values: list[object]) -> None:
    goto_cell(f"A{row_no}")
    for i, v in enumerate(values):
        text = f"{v:.2f}" if isinstance(v, float) else str(v)
        type_text(text, pause=0.15)
        key("Tab" if i < len(values) - 1 else "Return", pause=0.3)


def move_file(name: str) -> None:
    focus("Thunar")
    goto(str(INCOMING))
    select_by_name(name)
    key("ctrl+x", pause=0.4)
    goto(str(ENTERED))
    focus_list()
    key("ctrl+v", pause=1.2)


def main() -> None:
    skip = os.environ.get("VB_SKIP", "")
    by_file = {f["file"]: f for f in KEY["files"] if f["folder"] == "incoming"}
    order = [f for f in KEY["files"] if f["folder"] == "incoming"]
    # 1. read every PDF, in folder order
    for entry in order:
        open_and_close(entry)
    # 2. the ledger
    rows: list[list[object]] = []
    for f in order:
        if f["kind"] != "invoice" or f["file"] == skip:
            continue
        amount = f["amount"]
        if os.environ.get("VB_NET") and f["number"] == "KO-INV-00318":
            amount = 1925.00
        rows.append([f["number"], f["supplier"], f["date"], float(amount)])
        if os.environ.get("VB_DOUBLE") and f["number"] == "KO-INV-00318":
            rows.append([f["number"], f["supplier"], f["date"], float(amount)])
    if os.environ.get("VB_CREDIT"):
        cn = next(f for f in order if f["kind"] == "credit-note")
        rows.append([cn["number"], cn["supplier"], cn["date"], float(cn["amount"])])
    focus("LibreOffice Calc")
    key("ctrl+Home", pause=0.4)
    key("Next", pause=0.5)  # look at the bottom of the ledger first
    for i, row in enumerate(rows):
        type_row(FIRST_FREE_ROW + i, row)
    goto_cell("A1")
    before = ODS.stat().st_mtime
    if not os.environ.get("VB_NO_SAVE"):
        key("ctrl+s", pause=2.5)
        key("Return", pause=1.0)  # Keep current format, if the dialog appeared
        for _ in range(20):
            if ODS.stat().st_mtime != before:
                break
            time.sleep(0.5)
    # 3. the files
    moved = 0
    if not os.environ.get("VB_LEAVE_FILES"):
        to_move = [n for n in KEY["to_move"] if n != skip]
        if os.environ.get("VB_MOVE_CREDIT"):
            to_move += KEY["credit_notes"]
        for name in to_move:
            move_file(name)
            moved += int((ENTERED / name).is_file())
        focus("Thunar")
        goto(str(INCOMING))
    time.sleep(1.0)
    out_dir = Path("/logs/agent") if Path("/logs/agent").is_dir() else Path("/tmp")
    subprocess.run(["import", "-window", "root", str(out_dir / "gold-final.png")], env=ENV, check=False)
    print(f"gold: typed {len(rows)} rows, saved={ODS.stat().st_mtime != before}, moved {moved}; "
          f"{len(by_file)} PDFs opened")


if __name__ == "__main__":
    main()
