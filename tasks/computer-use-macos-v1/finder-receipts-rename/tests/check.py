#!/usr/bin/env python3
"""finder-receipts-rename checker: ~/Documents copied from the stopped VM.

    check.py /var/lib/vbmac

Each seeded file is found by its content (SHA-256; renaming and moving keep
the bytes). A receipt is right when it sits in the expected folder under the
name "YYYY-MM-DD <merchant>.pdf": the date it was paid and a merchant name
containing the merchant's key word (case and punctuation ignored).

  Scan.pdf    Office Depot        2026-03-02  Amex   Receipts
  Scan 1.pdf  Whole Foods Market  2026-02-27  Visa   Receipts/Personal
  Scan 2.pdf  Delta Air Lines     2026-02-18  Amex   (date of issue, not the flight)
  Scan 3.pdf  Hilton Garden Inn   2026-03-04  Amex   (UK folio: paid 04/03/2026)
  Scan 4.pdf  Uber                2026-03-05  Amex
  Scan 5.pdf  Shell               2026-02-21  Amex
  Scan 6.pdf  CVS Pharmacy        2026-03-06  Visa   Personal
  Scan 7.pdf  Apple Store         2026-02-24  Amex
  Scan 8.pdf  Blue Bottle Coffee  2026-03-03  Amex
  Scan 9.pdf  Staples             2026-03-06  Visa   Personal

Sub-metrics (reward is 1 only when all are 1):
  artifact_present   ~/Documents was collected
  business_renamed   the seven Amex receipts are in Receipts, correctly named
  personal_moved     the three Visa receipts are in Receipts/Personal,
                     correctly named
  uk_date            the London folio is dated 2026-03-04 (not 2026-04-03)
  nothing_lost       every seeded file exists exactly once, the expense
                     policy and the Corner Bakery receipt keep their names
                     and places, and nothing else was added to Receipts
  ui_path            the MCP server logged pointer input
"""

from __future__ import annotations

import hashlib
import re
import sys
from pathlib import Path

sys.path.insert(0, "/opt/vbdesk")
import cu_check as cu  # noqa: E402
import mac_state as M  # noqa: E402

METRICS = (
    "artifact_present",
    "business_renamed",
    "personal_moved",
    "uk_date",
    "nothing_lost",
    "ui_path",
)
DOCS = f"{M.HOME}/Documents"
P = "Receipts/Personal"
# sha256 -> (seeded name, payment date, merchant key word, folder)
RECEIPTS = {
    "80d59d065f77d26a0cded1021cd0c102298cc6bb164eb26f8cbf8e9e8aca4645": ("Scan.pdf", "2026-03-02", "office depot", "Receipts"),
    "4694bab28189cf86286ebae188bc5186953151c0e7c2736a713ac9cbd3073094": ("Scan 1.pdf", "2026-02-27", "whole foods", P),
    "3ea21f9987a480e33701fc506fcb8468202996c3106074d9dff3303d0ebf50e6": ("Scan 2.pdf", "2026-02-18", "delta", "Receipts"),
    "3c48596f4f8543411c03124165df6b1f04d45da3bb6db3cc6be2bc38c31c1931": ("Scan 3.pdf", "2026-03-04", "hilton", "Receipts"),
    "ad4bbd987eecf195f20846a5663911d7129018bd28d0cf4d381d30a4a3cd9574": ("Scan 4.pdf", "2026-03-05", "uber", "Receipts"),
    "d874b08317d3894beb051e86a5057ce16a924d1ba6b65da34831ae7b433ba1ee": ("Scan 5.pdf", "2026-02-21", "shell", "Receipts"),
    "93393a5e47fa69f2d7bcf91342ec6b7b4264e90e15a74470740b5b1750782ea2": ("Scan 6.pdf", "2026-03-06", "cvs", P),
    "7668f58e9dd59d69cccf64d799353de3bb7f9f162c0d3170c29262b3c4c94828": ("Scan 7.pdf", "2026-02-24", "apple", "Receipts"),
    "e2a12e400d3e004cb856496ee7f7501470f3d574a52d9270c690f0ac43ed56ed": ("Scan 8.pdf", "2026-03-03", "blue bottle", "Receipts"),
    "393d44d7e6b225354e9451f463533c7023bb48dd45cf5007738e2261ec99d89e": ("Scan 9.pdf", "2026-03-06", "staples", P),
}
FIXED = {
    "1f03331f6ab7e29620a4a10a55886c0c1b4755d0a40db80ffd2fc5c5fc728068": "Receipts/Expense policy.pdf",
    "ad0101be0ad3a16c769b979362cf96ab7a5fa7698bfbaad9e3a54d20ae160d77": "Receipts/Personal/2026-01-30 Corner Bakery.pdf",
}
HILTON = "3c48596f4f8543411c03124165df6b1f04d45da3bb6db3cc6be2bc38c31c1931"
NAME = re.compile(r"^(\d{4}-\d{2}-\d{2}) (.+)\.pdf$", re.IGNORECASE)


def words(s: str) -> str:
    return " ".join(re.sub(r"[^a-z0-9]+", " ", s.lower()).split())


def right(rel: str, date: str, key: str, folder: str) -> bool:
    path = Path(rel)
    if str(path.parent) != folder:
        return False
    mt = NAME.match(path.name)
    return bool(mt) and mt.group(1) == date and key in words(mt.group(2))


def main(argv: list[str]) -> int:
    root = Path(argv[1])
    export = root / "export"
    docs = export / "fs" / DOCS
    m = {k: 0 for k in METRICS}
    extra: dict[str, object] = {"shutdown": M.manifest(export).get("shutdown", "none")}
    if not (docs / "Receipts").is_dir():
        cu.emit(m, METRICS, **extra)
        return 0
    m["artifact_present"] = 1
    where: dict[str, list[str]] = {}
    for p in sorted(docs.rglob("*")):
        if p.is_file() and p.name != ".DS_Store" and not p.name.startswith("._"):
            digest = hashlib.sha256(p.read_bytes()).hexdigest()
            where.setdefault(digest, []).append(str(p.relative_to(docs)))
    extra["files"] = {RECEIPTS.get(h, (FIXED.get(h, h[:12]),))[0]: v for h, v in where.items()}
    placed = {
        h: len(where.get(h, [])) == 1 and right(where[h][0], date, key, folder)
        for h, (_, date, key, folder) in RECEIPTS.items()
    }
    m["business_renamed"] = int(all(ok for h, ok in placed.items() if RECEIPTS[h][3] == "Receipts"))
    m["personal_moved"] = int(all(ok for h, ok in placed.items() if RECEIPTS[h][3] == P))
    hil = where.get(HILTON, [])
    m["uk_date"] = int(len(hil) == 1 and Path(hil[0]).name.startswith("2026-03-04 "))
    fixed_ok = all(where.get(h) == [rel] for h, rel in FIXED.items())
    all_once = all(len(where.get(h, [])) == 1 for h in RECEIPTS)
    known = set(RECEIPTS) | set(FIXED)
    added = [v for h, v in where.items() if h not in known and any(x.startswith("Receipts/") for x in v)]
    extra["added_in_receipts"] = added
    m["nothing_lost"] = int(fixed_ok and all_once and not added)
    events_log = M.input_events(root / "input.log")
    m["ui_path"] = int(bool({e.get("kind") for e in events_log} & {"click", "drag"}))
    extra["input_events"] = len(events_log)
    cu.emit(m, METRICS, **extra)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
