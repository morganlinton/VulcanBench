#!/usr/bin/env python3
"""Workbook for tasks/computer-use-v1/calc-quarterly-rollup (deterministic).

    python scripts/computer-use/seeds/calc_quarterly_rollup.py

Reads the task's answer key, environment/rollup/workbook.json, checks that
the expected totals and statuses follow from the monthly rows, and writes
environment/rollup/Q3-rollup.ods (the file the task's image copies into
/root/Documents) and tests/seed-rollup.json (the verifier's copy of the
key). One row on the August sheet is hidden; its amount is part of the
correct total. The .ods is written by ods.py, byte-identical on every run.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from ods import write_ods

TASK = Path(__file__).resolve().parents[3] / "tasks" / "computer-use-v1" / "calc-quarterly-rollup"
KEY = TASK / "environment" / "rollup" / "workbook.json"


def check_answer_key(wb: dict) -> None:
    totals: dict[str, float] = {}
    for lines in wb["months"].values():
        for line in lines:
            region, _, amount = line["row"] if isinstance(line, dict) else line
            totals[region] = totals.get(region, 0) + amount
    assert totals == wb["expected"]["totals"], (totals, wb["expected"]["totals"])
    for region, total in totals.items():
        want = "Below target" if total < wb["targets"][region] else "OK"
        assert wb["expected"]["status"][region] == want, (region, total, want)
    hidden = sum(
        line["row"][2]
        for lines in wb["months"].values()
        for line in lines
        if isinstance(line, dict) and line.get("hidden")
    )
    assert (
        wb["expected"]["totals"]["West"] - hidden == wb["expected"]["west_total_without_hidden_row"]
    )


def sheets(wb: dict) -> list[dict]:
    s = wb["summary"]
    out = [
        {
            "name": "Summary",
            "widths": ["co1", "co2", "co2", "co3"],
            "rows": [s["headers"]] + [[r["region"], None, r["target"], None] for r in s["rows"]],
        }
    ]
    for month, lines in wb["months"].items():
        out.append(
            {
                "name": month,
                "widths": ["co1", "co3", "co2"],
                "rows": [["Region", "Customer", "Amount"], *lines],
            }
        )
    out.append(
        {
            "name": "Targets",
            "widths": ["co1", "co2"],
            "rows": [["Region", "Target"]] + [[k, v] for k, v in wb["targets"].items()],
        }
    )
    return out


def main() -> None:
    wb = json.loads(KEY.read_text())
    check_answer_key(wb)
    ods = write_ods(TASK / "environment" / "rollup" / wb["file"], sheets(wb))
    (TASK / "tests" / "seed-rollup.json").write_text(KEY.read_text())
    print(f"wrote {ods.relative_to(TASK.parent)} and tests/seed-rollup.json")


if __name__ == "__main__":
    main()
