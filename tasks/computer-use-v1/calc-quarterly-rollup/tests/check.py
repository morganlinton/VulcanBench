#!/usr/bin/env python3
"""End-state checker for calc-quarterly-rollup.

    python3 check.py DOCUMENTS_DIR WORKBOOK_JSON INPUT_LOG_DIR

Parses the saved Q3-rollup.ods (content.xml inside the zip, through
cu_check.ods_sheets) and compares it with the answer key in workbook.json:
the three totals and statuses on the Summary sheet, every source sheet
unchanged, nothing else on Summary changed, and real input on the display
before the file was written. Shared helpers: /opt/vbdesk/cu_check.py (base
image).
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

sys.path.insert(0, "/opt/vbdesk")
import cu_check as cu  # noqa: E402

METRICS = (
    "artifact_present", "file_is_ods", "totals_correct", "status_correct",
    "sources_untouched", "summary_untouched", "ui_path",
)


def seed_sheets(wb: dict) -> dict[str, list[list[cu.Cell]]]:
    out: dict[str, list[list[cu.Cell]]] = {}
    for month, lines in wb["months"].items():
        out[month] = cu.ods_rows_from_seed([["Region", "Customer", "Amount"], *lines])
    out["Targets"] = cu.ods_rows_from_seed([["Region", "Target"], *[[k, v] for k, v in wb["targets"].items()]])
    return out


def main(argv: list[str]) -> int:
    docs, wb_path, vb = Path(argv[1]), Path(argv[2]), Path(argv[3])
    wb = json.loads(wb_path.read_text())
    m: dict[str, int] = {k: 0 for k in METRICS}
    ods = docs / wb["file"]
    m["artifact_present"] = int(ods.is_file())
    if not m["artifact_present"]:
        cu.emit(m, METRICS, note="workbook missing")
        return 0
    m["file_is_ods"] = cu.ods_is_spreadsheet(ods)
    try:
        sheets = cu.ods_sheets(ods)
    except Exception as e:  # noqa: BLE001 - any unreadable file scores 0 with the reason logged
        cu.emit(m, METRICS, note=f"unreadable: {e}")
        return 0

    get = cu.ods_cell
    summary = sheets.get("Summary", [])
    regions = [r["region"] for r in wb["summary"]["rows"]]
    expected_totals, expected_status = wb["expected"]["totals"], wb["expected"]["status"]
    got_totals = {reg: get(summary, i + 1, 1) for i, reg in enumerate(regions)}
    got_status = {reg: get(summary, i + 1, 3) for i, reg in enumerate(regions)}
    m["totals_correct"] = int(all(
        c[0] == "float" and abs(float(c[1]) - expected_totals[reg]) < 0.005 for reg, c in got_totals.items()
    ))
    m["status_correct"] = int(all(
        c[0] == "string" and str(c[1]).strip().casefold() == expected_status[reg].casefold()
        for reg, c in got_status.items()
    ))
    seeds = seed_sheets(wb)
    m["sources_untouched"] = int(all(name in sheets and cu.ods_same_rows(sheets[name], seeds[name]) for name in seeds))
    # Summary: headers, region names and the given targets unchanged; West's
    # target cell may keep its note or hold the number from the Targets sheet.
    headers_ok = [get(summary, 0, i) for i in range(4)] == [("string", h) for h in wb["summary"]["headers"]]
    regions_ok = all(get(summary, i + 1, 0) == ("string", reg) for i, reg in enumerate(regions))
    targets_ok = True
    for i, r in enumerate(wb["summary"]["rows"]):
        c = get(summary, i + 1, 2)
        if isinstance(r["target"], (int, float)):
            targets_ok &= c[0] == "float" and abs(float(c[1]) - r["target"]) < 0.005
        else:
            targets_ok &= (c == ("string", r["target"])) or (
                c[0] == "float" and abs(float(c[1]) - wb["targets"][r["region"]]) < 0.005
            )
    no_extra = len(summary) <= len(regions) + 1 and all(len(row) <= 4 for row in summary)
    m["summary_untouched"] = int(headers_ok and regions_ok and targets_ok and no_extra)

    presses = cu.presses(cu.xinput_events(vb / "xinput.log"))
    m["ui_path"] = int(cu.ui_path_fs(presses, [ods.stat().st_mtime], window_s=900.0, min_presses=10))
    cu.emit(m, METRICS, presses=len(presses),
            totals={k: v[1] for k, v in got_totals.items()}, status={k: v[1] for k, v in got_status.items()})
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
