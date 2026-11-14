#!/usr/bin/env python3
"""expenses-audit checker: ~/Documents/Expenses copied from the stopped VM.

    check.py /var/lib/vbmac

The report is read with mac_state.rtf_paragraphs. An expense line is a
paragraph with a date (03 Feb 2026, 3 February 2026, Feb 3, 2026,
2026-02-03 or 2/3/2026) and an amount (the last number with two decimals);
it matches a receipt by date and amount. The 36 reimbursable claims total
3,617.14.

Sub-metrics (reward is 1 only when all are 1):
  artifact_present  the report was collected and parses
  amounts_fixed     the six mistyped lines carry their receipts' amounts
  missing_added     the five receipts left off the report have lines
  mixed_split       Harbor Hotel claims 412.80 (no minibar or movie) and Lakeside
                    Grill 128.45 (no wine, tip kept)
  euros_converted   the four euro receipts are claimed at the USD the card was
                    charged (30.99, 430.61, 154.41, 29.36)
  gift_capped       the Bloom & Stem gift is claimed at 75.00
  personal_out      no line for StreamBox, Rosewood, Peak Fitness or Sweet Layers
  unbacked_out      no line for the City Cab taxi (38.00), which has no receipt
  listed_once       Bistro Nine has exactly one line
  exact_lines       exactly the 36 claims, one line each
  date_order        the expense lines are in date order
  total_right       a Total line carries 3,617.14
  receipts_filed    Expenses/Personal holds the four personal receipts (the
                    duplicate Bistro Nine scan may be there too, or anywhere),
                    and all 36 business receipts are still in Receipts
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
    "mixed_split",
    "euros_converted",
    "gift_capped",
    "personal_out",
    "unbacked_out",
    "listed_once",
    "exact_lines",
    "date_order",
    "total_right",
    "receipts_filed",
    "ui_path",
)
CLAIMS = [
    ("2026-02-02", 46.20, "2026-02-02 Logan Taxi.pdf"), ("2026-02-02", 35.00, "2026-02-02 Skyline Air bag.pdf"),
    ("2026-02-02", 6.40, "2026-02-02 OHare Coffee.pdf"), ("2026-02-03", 128.45, "2026-02-03 Lakeside Grill.pdf"),
    ("2026-02-03", 22.10, "2026-02-03 Metro Cab.pdf"), ("2026-02-04", 18.75, "2026-02-04 Copy Corner.pdf"),
    ("2026-02-04", 17.85, "2026-02-04 Deep Dish.pdf"), ("2026-02-05", 412.80, "2026-02-05 Harbor Hotel.pdf"),
    ("2026-02-05", 75.00, "2026-02-05 Skyline Air change.pdf"), ("2026-02-06", 54.99, "2026-02-06 Paper Trail.pdf"),
    ("2026-02-09", 31.00, "2026-02-09 Garage 7.pdf"), ("2026-02-10", 64.30, "2026-02-10 Bistro Nine.pdf"),
    ("2026-02-10", 59.99, "2026-02-10 Cloudnote.pdf"), ("2026-02-11", 299.00, "2026-02-11 DevSummit.pdf"),
    ("2026-02-12", 89.00, "2026-02-12 Northeast Rail.pdf"), ("2026-02-12", 219.60, "2026-02-12 Union Square Inn.pdf"),
    ("2026-02-13", 87.25, "2026-02-13 Daybreak Diner.pdf"), ("2026-02-13", 27.80, "2026-02-13 Yellow Line.pdf"),
    ("2026-02-13", 8.50, "2026-02-13 Pressed Juice.pdf"), ("2026-02-14", 89.00, "2026-02-14 Northeast Rail.pdf"),
    ("2026-02-16", 12.00, "2026-02-16 Diagram Pro.pdf"), ("2026-02-17", 41.60, "2026-02-17 FastLane.pdf"),
    ("2026-02-17", 31.00, "2026-02-17 Garage 7.pdf"), ("2026-02-18", 36.40, "2026-02-18 Kinko Print Hub.pdf"),
    ("2026-02-18", 75.00, "2026-02-18 Bloom and Stem.pdf"), ("2026-02-19", 642.00, "2026-02-19 Skyline Air flight.pdf"),
    ("2026-02-20", 30.99, "2026-02-20 Taxis Lisboa.pdf"), ("2026-02-20", 430.61, "2026-02-20 Casa Alfama.pdf"),
    ("2026-02-21", 154.41, "2026-02-21 Tasca do Bairro.pdf"), ("2026-02-23", 29.36, "2026-02-23 Taxis Lisboa.pdf"),
    ("2026-02-23", 35.00, "2026-02-23 Skyline Air bag.pdf"), ("2026-02-24", 48.60, "2026-02-24 Logan Taxi.pdf"),
    ("2026-02-25", 112.40, "2026-02-25 Noodle House.pdf"), ("2026-02-26", 89.99, "2026-02-26 Paper Trail.pdf"),
    ("2026-02-26", 23.80, "2026-02-26 FastLane.pdf"), ("2026-02-27", 31.00, "2026-02-27 Garage 7.pdf"),
]  # fmt: skip
PERSONAL = [
    ("2026-02-08", 15.99, "2026-02-08 StreamBox.pdf"), ("2026-02-14", 186.50, "2026-02-14 Rosewood.pdf"),
    ("2026-02-15", 49.00, "2026-02-15 Peak Fitness.pdf"), ("2026-02-22", 42.00, "2026-02-22 Sweet Layers.pdf"),
]  # fmt: skip
DUPLICATE_SCAN = "Scan 2026-02-10.pdf"
FIXED = [("2026-02-12", 219.60), ("2026-02-11", 299.00), ("2026-02-13", 27.80), ("2026-02-17", 41.60),
         ("2026-02-19", 642.00), ("2026-02-25", 112.40)]  # fmt: skip
ADDED = [("2026-02-02", 6.40), ("2026-02-13", 8.50), ("2026-02-18", 36.40), ("2026-02-23", 29.36), ("2026-02-26", 89.99)]
MIXED = [("2026-02-05", 412.80), ("2026-02-03", 128.45)]
EUROS = [("2026-02-20", 30.99), ("2026-02-20", 430.61), ("2026-02-21", 154.41), ("2026-02-23", 29.36)]
GIFT = ("2026-02-18", 75.00)
UNBACKED = ("2026-02-13", 38.00)
BISTRO = ("2026-02-10", 64.30)
TOTAL = 3617.14
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
    have: list[tuple[str, float]] = []
    total_line: float | None = None
    for p in paras:
        d, a = find_date(p.text), last_amount(p.text)
        if d is not None and a is not None:
            have.append((d.isoformat(), a))
        elif re.match(r"\s*total", p.text, re.I) and a is not None:
            total_line = a
    extra["lines"] = have
    extra["total_line"] = total_line
    m["amounts_fixed"] = int(all(x in have for x in FIXED))
    m["missing_added"] = int(all(x in have for x in ADDED))
    m["mixed_split"] = int(all(x in have for x in MIXED))
    m["euros_converted"] = int(all(x in have for x in EUROS))
    m["gift_capped"] = int(GIFT in have and ("2026-02-18", 95.00) not in have)
    m["personal_out"] = int(not any((d, a) in have for d, a, _ in PERSONAL))
    m["unbacked_out"] = int(UNBACKED not in have)
    m["listed_once"] = int(have.count(BISTRO) == 1)
    m["exact_lines"] = int(sorted(have) == sorted((d, a) for d, a, _ in CLAIMS))
    m["date_order"] = int(len(have) > 1 and [d for d, _ in have] == sorted(d for d, _ in have))
    m["total_right"] = int(total_line is not None and abs(total_line - TOTAL) < 0.005)
    rec = fs / EXP / "Receipts"
    per = fs / EXP / "Personal"
    in_rec = {p.name for p in rec.iterdir() if p.is_file() and not p.name.startswith(".")} if rec.is_dir() else set()
    in_per = {p.name for p in per.iterdir() if p.is_file() and not p.name.startswith(".")} if per.is_dir() else set()
    extra["personal_folder"] = sorted(in_per)
    m["receipts_filed"] = int(
        in_per - {DUPLICATE_SCAN} == {f for _, _, f in PERSONAL} and {f for _, _, f in CLAIMS} <= in_rec
    )
    events_log = M.input_events(root / "input.log")
    m["ui_path"] = int(bool({e.get("kind") for e in events_log} & {"click", "drag"}))
    extra["input_events"] = len(events_log)
    cu.emit(m, METRICS, **extra)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
