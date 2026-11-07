#!/usr/bin/env python3
"""End-state checker for calc-quarterly-rollup.

    python3 check.py DOCUMENTS_DIR WORKBOOK_JSON INPUT_LOG_DIR

Parses the saved Q3-rollup.ods (content.xml inside the zip) and compares it
with the answer key in workbook.json: the three totals and statuses on the
Summary sheet, every source sheet unchanged, nothing else on Summary
changed, and real input on the display before the file was written.
Shared helpers: /opt/vbdesk/cu_check.py (base image).
"""

from __future__ import annotations

import json
import sys
import zipfile
from pathlib import Path
from xml.etree import ElementTree as ET

sys.path.insert(0, "/opt/vbdesk")
import cu_check as cu  # noqa: E402

T = "{urn:oasis:names:tc:opendocument:xmlns:table:1.0}"
O = "{urn:oasis:names:tc:opendocument:xmlns:office:1.0}"
X = "{urn:oasis:names:tc:opendocument:xmlns:text:1.0}"
METRICS = (
    "artifact_present", "file_is_ods", "totals_correct", "status_correct",
    "sources_untouched", "summary_untouched", "ui_path",
)
Cell = tuple[str, object]  # ("float", 12.5) | ("string", "text") | ("empty", None)


def read_sheets(ods: Path) -> dict[str, list[list[Cell]]]:
    with zipfile.ZipFile(ods) as z:
        root = ET.fromstring(z.read("content.xml"))
    sheets: dict[str, list[list[Cell]]] = {}
    for table in root.iter(f"{T}table"):
        rows: list[list[Cell]] = []
        for tr in table.iter(f"{T}table-row"):
            cells: list[Cell] = []
            for tc in tr:
                if tc.tag not in (f"{T}table-cell", f"{T}covered-table-cell"):
                    continue
                rep = int(tc.get(f"{T}number-columns-repeated", "1"))
                vt = tc.get(f"{O}value-type")
                text = "".join(p.text or "" for p in tc.iter(f"{X}p")).strip()
                if vt in ("float", "currency", "percentage"):
                    c: Cell = ("float", float(tc.get(f"{O}value", "nan")))
                elif text:
                    c = ("string", text)
                else:
                    c = ("empty", None)
                cells.extend([c] * min(rep, 64))
            while cells and cells[-1][0] == "empty":
                cells.pop()
            rrep = int(tr.get(f"{T}number-rows-repeated", "1"))
            rows.extend([cells] * min(rrep, 64))
        while rows and not rows[-1]:
            rows.pop()
        sheets[table.get(f"{T}name", "")] = rows
    return sheets


def get(rows: list[list[Cell]], r: int, c: int) -> Cell:
    try:
        return rows[r][c]
    except IndexError:
        return ("empty", None)


def seed_sheets(wb: dict) -> dict[str, list[list[Cell]]]:
    def conv(v: object) -> Cell:
        if v is None or v == "":
            return ("empty", None)
        if isinstance(v, (int, float)):
            return ("float", float(v))
        return ("string", str(v))

    out: dict[str, list[list[Cell]]] = {}
    for month, lines in wb["months"].items():
        rows = [[conv("Region"), conv("Customer"), conv("Amount")]]
        for line in lines:
            vals = line["row"] if isinstance(line, dict) else line
            rows.append([conv(v) for v in vals])
        out[month] = rows
    out["Targets"] = [[conv("Region"), conv("Target")]] + [[conv(k), conv(v)] for k, v in wb["targets"].items()]
    return out


def same_cell(a: Cell, b: Cell) -> bool:
    if a[0] == "float" and b[0] == "float":
        return abs(float(a[1]) - float(b[1])) < 0.005
    if a[0] == "string" and b[0] == "string":
        return str(a[1]).strip() == str(b[1]).strip()
    return a[0] == b[0] == "empty"


def same_rows(a: list[list[Cell]], b: list[list[Cell]]) -> bool:
    if len(a) != len(b):
        return False
    for ra, rb in zip(a, b, strict=True):
        n = max(len(ra), len(rb))
        if not all(same_cell(get([ra], 0, i), get([rb], 0, i)) for i in range(n)):
            return False
    return True


def main(argv: list[str]) -> int:
    docs, wb_path, vb = Path(argv[1]), Path(argv[2]), Path(argv[3])
    wb = json.loads(wb_path.read_text())
    m: dict[str, int] = {k: 0 for k in METRICS}
    ods = docs / wb["file"]
    m["artifact_present"] = int(ods.is_file())
    if not m["artifact_present"]:
        cu.emit(m, METRICS, note="workbook missing")
        return 0
    try:
        with zipfile.ZipFile(ods) as z:
            m["file_is_ods"] = int(z.read("mimetype").startswith(b"application/vnd.oasis.opendocument.spreadsheet"))
        sheets = read_sheets(ods)
    except (zipfile.BadZipFile, KeyError, ET.ParseError) as e:
        cu.emit(m, METRICS, note=f"unreadable: {e}")
        return 0

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
    m["sources_untouched"] = int(all(name in sheets and same_rows(sheets[name], seeds[name]) for name in seeds))
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
            targets_ok &= (c == ("string", r["target"])) or (c[0] == "float" and abs(float(c[1]) - wb["targets"][r["region"]]) < 0.005)
    no_extra = len(summary) <= len(regions) + 1 and all(len(row) <= 4 for row in summary)
    m["summary_untouched"] = int(headers_ok and regions_ok and targets_ok and no_extra)

    presses = cu.presses(cu.xinput_events(vb / "xinput.log"))
    m["ui_path"] = int(cu.ui_path_fs(presses, [ods.stat().st_mtime], window_s=900.0, min_presses=10))
    cu.emit(m, METRICS, presses=len(presses),
            totals={k: v[1] for k, v in got_totals.items()}, status={k: v[1] for k, v in got_status.items()})
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
