#!/usr/bin/env python3
"""expenses-reconcile checker: ~/Documents/Expenses copied from the stopped VM.

    check.py /var/lib/vbmac

The report is read with mac_state.rtf_paragraphs. An expense line is a
paragraph with a date (03 Feb 2026, 3 February 2026, Feb 3, 2026,
2026-02-03 or 2/3/2026) and an amount (the last number with two decimals);
it matches a receipt by date and amount. The 22 business receipts total
1,961.64.

Sub-metrics (reward is 1 only when all are 1):
  artifact_present  the report was collected and parses
  amounts_fixed     the five mis-typed lines now carry their receipts' amounts
  missing_added     O'Hare Coffee, Daybreak Diner and Bloom & Stem have lines
  personal_out      no line for the Rosewood dinner or the Peak Fitness membership
  exact_lines       exactly 22 expense lines, one per business receipt
  date_order        the expense lines are in date order
  total_right       a Total line carries 1,961.64
  receipts_filed    Expenses/Personal holds exactly the Rosewood and Peak Fitness
                    receipts, and the 22 others are still in Receipts
  ui_path           the MCP server logged pointer input
"""

from __future__ import annotations

import re
import sys
from datetime import date
from pathlib import Path

sys.path.insert(0, "/opt/vbdesk")
import cu_check as cu  # noqa: E402
import mac_state as M  # noqa: E402

METRICS = (
    "artifact_present",
    "amounts_fixed",
    "missing_added",
    "personal_out",
    "exact_lines",
    "date_order",
    "total_right",
    "receipts_filed",
    "ui_path",
)
BUSINESS = [
    ("2026-02-02", 46.20, "2026-02-02 Logan Taxi.pdf"), ("2026-02-02", 35.00, "2026-02-02 Skyline Air bag.pdf"),
    ("2026-02-03", 412.80, "2026-02-03 Harbor Hotel.pdf"), ("2026-02-03", 128.45, "2026-02-03 Lakeside Grill.pdf"),
    ("2026-02-04", 22.10, "2026-02-04 Metro Cab.pdf"), ("2026-02-04", 18.75, "2026-02-04 Copy Corner.pdf"),
    ("2026-02-05", 75.00, "2026-02-05 Skyline Air change.pdf"), ("2026-02-05", 6.40, "2026-02-05 OHare Coffee.pdf"),
    ("2026-02-09", 54.99, "2026-02-09 Paper Trail.pdf"), ("2026-02-10", 31.00, "2026-02-10 Garage 7.pdf"),
    ("2026-02-11", 64.30, "2026-02-11 Bistro Nine.pdf"), ("2026-02-12", 299.00, "2026-02-12 DevSummit.pdf"),
    ("2026-02-16", 89.00, "2026-02-16 Northeast Rail.pdf"), ("2026-02-16", 219.60, "2026-02-16 Union Square Inn.pdf"),
    ("2026-02-17", 87.25, "2026-02-17 Daybreak Diner.pdf"), ("2026-02-17", 27.80, "2026-02-17 Yellow Line.pdf"),
    ("2026-02-18", 89.00, "2026-02-18 Northeast Rail.pdf"), ("2026-02-19", 12.00, "2026-02-19 Diagram Pro.pdf"),
    ("2026-02-20", 41.60, "2026-02-20 FastLane.pdf"), ("2026-02-23", 58.00, "2026-02-23 Bloom and Stem.pdf"),
    ("2026-02-24", 31.00, "2026-02-24 Garage 7.pdf"), ("2026-02-25", 112.40, "2026-02-25 Noodle House.pdf"),
]  # fmt: skip
PERSONAL = [("2026-02-14", 186.50, "2026-02-14 Rosewood.pdf"), ("2026-02-21", 49.00, "2026-02-21 Peak Fitness.pdf")]
FIXED = [("2026-02-03", 412.80), ("2026-02-03", 128.45), ("2026-02-12", 299.00), ("2026-02-20", 41.60), ("2026-02-25", 112.40)]
ADDED = [("2026-02-05", 6.40), ("2026-02-17", 87.25), ("2026-02-23", 58.00)]
TOTAL = 1961.64
REPORT = "Users/admin/Documents/Expenses/February expense report.rtf"
EXP = "Users/admin/Documents/Expenses"
MON = {m: i for i, m in enumerate(["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"], 1)}
AMOUNT = re.compile(r"(?<![\d.,])(\d{1,3}(?:,\d{3})*|\d+)\.(\d{2})(?!\d)")


def find_date(text: str) -> date | None:
    t = text.lower()
    for rx, order in (
        (r"\b(20\d\d)-(\d\d)-(\d\d)\b", "ymd"),
        (r"\b(\d{1,2})\s+([a-z]{3})[a-z]*\.?,?\s+(20\d\d)\b", "dmy"),
        (r"\b([a-z]{3})[a-z]*\.?\s+(\d{1,2}),?\s+(20\d\d)\b", "mdy"),
        (r"\b(\d{1,2})/(\d{1,2})/(20\d\d)\b", "nmdy"),
    ):
        hit = re.search(rx, t)
        if not hit:
            continue
        a, b, c = hit.groups()
        try:
            if order == "ymd":
                return date(int(a), int(b), int(c))
            if order == "dmy" and b in MON:
                return date(int(c), MON[b], int(a))
            if order == "mdy" and a in MON:
                return date(int(c), MON[a], int(b))
            if order == "nmdy":
                return date(int(c), int(a), int(b))
        except ValueError:
            return None
    return None


def last_amount(text: str) -> float | None:
    hits = AMOUNT.findall(text)
    if not hits:
        return None
    whole, cents = hits[-1]
    return round(float(whole.replace(",", "") + "." + cents), 2)


def iso(d: date) -> str:
    return d.isoformat()


def main(argv: list[str]) -> int:
    root = Path(argv[1])
    export = root / "export"
    fs = export / "fs"
    m = {k: 0 for k in METRICS}
    extra: dict[str, object] = {"shutdown": M.manifest(export).get("shutdown", "none")}
    try:
        paras = M.rtf_paragraphs(fs / REPORT)
    except (OSError, ValueError):
        paras = []
    if not paras:
        cu.emit(m, METRICS, **extra)
        return 0
    m["artifact_present"] = 1
    lines: list[tuple[str, float]] = []
    total_line: float | None = None
    for p in paras:
        d, a = find_date(p.text), last_amount(p.text)
        if d is not None and a is not None:
            lines.append((iso(d), a))
        elif re.match(r"\s*total", p.text, re.I) and a is not None:
            total_line = a
    extra["lines"] = lines
    extra["total_line"] = total_line
    have = list(lines)
    m["amounts_fixed"] = int(all(x in have for x in FIXED))
    m["missing_added"] = int(all(x in have for x in ADDED))
    m["personal_out"] = int(not any((d, a) in have for d, a, _ in PERSONAL))
    want = sorted((d, a) for d, a, _ in BUSINESS)
    m["exact_lines"] = int(sorted(have) == want)
    m["date_order"] = int(len(have) > 1 and [d for d, _ in have] == sorted(d for d, _ in have))
    m["total_right"] = int(total_line is not None and abs(total_line - TOTAL) < 0.005)
    rec = fs / EXP / "Receipts"
    per = fs / EXP / "Personal"
    in_rec = {p.name for p in rec.iterdir() if p.is_file() and not p.name.startswith(".")} if rec.is_dir() else set()
    in_per = {p.name for p in per.iterdir() if p.is_file() and not p.name.startswith(".")} if per.is_dir() else set()
    extra["personal_folder"] = sorted(in_per)
    m["receipts_filed"] = int(in_per == {f for _, _, f in PERSONAL} and in_rec == {f for _, _, f in BUSINESS})
    events_log = M.input_events(root / "input.log")
    m["ui_path"] = int(bool({e.get("kind") for e in events_log} & {"click", "drag"}))
    extra["input_events"] = len(events_log)
    cu.emit(m, METRICS, **extra)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
