#!/usr/bin/env python3
"""Gold trajectory for calc-quarterly-rollup: Calc driven through the keyboard.

Jumps to cells through the Name Box (clicked at its fixed position in the
maximised window, then typed), types the totals computed from the answer key
and the statuses, and saves with Ctrl+S. Every step is real input on the
display, so the input recorder sees it.

VB_WRONG_STATUS=1  write OK everywhere (status check)
VB_SKIP_WEST=1     leave West's total empty (partial check)
VB_EDIT_SOURCE=1   also overwrite a cell on the July sheet (source guard)
VB_NO_SAVE=1       do everything but save (end state unchanged)
VB_NO_HIDDEN=1     total West without the hidden August row (the trap)
"""

from __future__ import annotations

import json
import os
import subprocess
import time
from pathlib import Path

WB = json.loads(Path("/opt/rollup/workbook.json").read_text())
ODS = Path("/root/Documents") / WB["file"]
ENV = {**os.environ, "DISPLAY": ":1"}
NAME_BOX = (70, 145)  # Calc maximised at 1024x900 with the suite's profile


def xdo(*args: str) -> None:
    subprocess.run(["xdotool", *args], check=True, env=ENV)


def key(*keys: str, pause: float = 0.4) -> None:
    xdo("key", "--clearmodifiers", "--delay", "80", *keys)
    time.sleep(pause)


def type_text(text: str, pause: float = 0.3) -> None:
    xdo("type", "--delay", "35", "--", text)
    time.sleep(pause)


def focus_calc() -> None:
    out = subprocess.run(["wmctrl", "-l"], capture_output=True, text=True, env=ENV).stdout
    wid = next((line.split()[0] for line in out.splitlines() if "LibreOffice Calc" in line), None)
    if wid is None:
        raise SystemExit("gold: no Calc window")
    subprocess.run(["wmctrl", "-i", "-a", wid], env=ENV, check=False)
    time.sleep(0.8)


def goto_cell(ref: str) -> None:
    xdo("mousemove", str(NAME_BOX[0]), str(NAME_BOX[1]), "click", "--clearmodifiers", "1")
    time.sleep(0.4)
    key("ctrl+a", pause=0.15)
    type_text(ref)
    key("Return", pause=0.6)


def put(ref: str, text: str) -> None:
    goto_cell(ref)
    type_text(text)
    key("Return", pause=0.5)


def main() -> None:
    focus_calc()
    totals = dict(WB["expected"]["totals"])
    status = dict(WB["expected"]["status"])
    if os.environ.get("VB_NO_HIDDEN"):
        totals["West"] = WB["expected"]["west_total_without_hidden_row"]
        status["West"] = "Below target" if totals["West"] < WB["targets"]["West"] else "OK"
    if os.environ.get("VB_WRONG_STATUS"):
        status = {k: "OK" for k in status}
    regions = [r["region"] for r in WB["summary"]["rows"]]
    # look at the monthly sheets first, as a person would
    for sheet in ("July", "August", "September", "Targets", "Summary"):
        goto_cell(f"${sheet}.A1")
    for i, reg in enumerate(regions):
        row = i + 2
        if reg == "West" and os.environ.get("VB_SKIP_WEST"):
            continue
        put(f"$Summary.B{row}", str(totals[reg]))
        put(f"$Summary.D{row}", status[reg])
    if os.environ.get("VB_EDIT_SOURCE"):
        put("$July.C2", "1")
    goto_cell("$Summary.A1")
    before = ODS.stat().st_mtime
    if not os.environ.get("VB_NO_SAVE"):
        key("ctrl+s", pause=2.5)
        for _ in range(20):
            if ODS.stat().st_mtime != before:
                break
            time.sleep(0.5)
        key("Return", pause=0.5)  # dismiss a format dialog if one appeared (none expected)
    time.sleep(1.0)
    out_dir = Path("/logs/agent") if Path("/logs/agent").is_dir() else Path("/tmp")
    subprocess.run(["import", "-window", "root", str(out_dir / "gold-final.png")], env=ENV, check=False)
    print(f"gold: saved={ODS.stat().st_mtime != before} totals={totals} status={status}")


if __name__ == "__main__":
    main()
