#!/usr/bin/env python3
"""End-state checker for files-invoice-entry.

    python3 check.py INVOICES_DIR FINANCE_DIR KEY_JSON INPUT_LOG_DIR

INVOICES_DIR and FINANCE_DIR are the copies of /root/Invoices and
/root/Finance taken after the agent stopped; KEY_JSON the answer key the
seed wrote; INPUT_LOG_DIR the copied /var/lib/vbdesk (xinput.log).

  artifact_present          the ledger and both invoice folders are there
  file_is_ods               the ledger is still an OpenDocument spreadsheet
  ledger_existing_untouched header and the fourteen seed rows unchanged
  ledger_new_rows_exact     the rows below them, as a multiset, are exactly the
                            five new invoices (number, supplier, ISO date,
                            amount); nothing missing, nothing extra
  no_duplicate_rows         no invoice number appears twice in the sheet
  files_moved               entered holds its two originals plus the seven
                            dealt-with files, incoming only the credit note,
                            every byte unchanged, nothing else anywhere
  ui_path                   real input on the display before the ledger was
                            written and before the entered folder changed

Shared helpers: /opt/vbdesk/cu_check.py (base image).
"""

from __future__ import annotations

import json
import re
import sys
from collections import Counter
from pathlib import Path

sys.path.insert(0, "/opt/vbdesk")
import cu_check as cu  # noqa: E402

METRICS = (
    "artifact_present", "file_is_ods", "ledger_existing_untouched", "ledger_new_rows_exact",
    "no_duplicate_rows", "files_moved", "ui_path",
)
SUFFIX = re.compile(r"\s+(ltd\.?|limited|llp)$", re.I)


def norm_supplier(s: object) -> str:
    return SUFFIX.sub("", str(s).strip().casefold())


def cell_text(c: cu.Cell) -> str:
    if c[0] == "float":
        return f"{float(c[1]):.2f}"
    return "" if c[1] is None else str(c[1]).strip()


def row_key(r: list[cu.Cell]) -> tuple[str, str, str, str] | None:
    """(invoice, supplier, date, amount) as comparable strings, or None if malformed."""
    cells = [cu.ods_cell([r], 0, i) for i in range(4)]
    if cells[3][0] != "float":
        return None
    inv, sup, date = (cell_text(c) for c in cells[:3])
    return inv, norm_supplier(sup), date, f"{float(cells[3][1]):.2f}"


def main(argv: list[str]) -> int:
    inv_dir, fin_dir, key_path, vb = (Path(a) for a in argv[1:5])
    key = json.loads(key_path.read_text())
    m: dict[str, int] = {k: 0 for k in METRICS}
    ods = fin_dir / key["ledger_file"]
    incoming, entered = inv_dir / "incoming", inv_dir / "entered"
    m["artifact_present"] = int(ods.is_file() and incoming.is_dir() and entered.is_dir())
    if not m["artifact_present"]:
        cu.emit(m, METRICS, note="ledger or invoice folders missing from artifact")
        return 0
    m["file_is_ods"] = cu.ods_is_spreadsheet(ods)

    new_found: list[tuple] = []
    extras: list[str] = []
    try:
        sheets = cu.ods_sheets(ods)
        rows = sheets.get(key["sheet"], [])
        n_seed = len(key["ledger_rows"]) + 1
        seed_cells = cu.ods_rows_from_seed([key["headers"], *key["ledger_rows"]])
        m["ledger_existing_untouched"] = int(
            len(rows) >= n_seed and cu.ods_same_rows(rows[:n_seed], seed_cells)
        )
        tail = [r for r in rows[n_seed:] if any(c[0] != "empty" for c in r)]
        new_found = [row_key(r) for r in tail]
        want = Counter(
            (n, norm_supplier(s), d, f"{float(a):.2f}") for n, s, d, a in key["expected_new_rows"]
        )
        got = Counter(k for k in new_found if k is not None)
        malformed = sum(1 for k in new_found if k is None)
        m["ledger_new_rows_exact"] = int(got == want and malformed == 0)
        extras = [str(k) for k in (got - want).elements()] + [f"malformed x{malformed}"] * bool(malformed)
        numbers = [cell_text(cu.ods_cell([r], 0, 0)) for r in rows[1:] if any(c[0] != "empty" for c in r)]
        m["no_duplicate_rows"] = int(len(numbers) == len(set(numbers)))
    except Exception as e:  # noqa: BLE001 - an unreadable workbook scores 0 with the reason logged
        print(f"ledger unreadable: {e}", file=sys.stderr)

    actual = cu.tree(inv_dir)
    expected: dict[str, str] = {}
    for f in key["files"]:
        if f["folder"] == "entered" or f["file"] in key["to_move"]:
            expected[f"entered/{f['file']}"] = f["sha256"]
        else:
            expected[f"incoming/{f['file']}"] = f["sha256"]
    m["files_moved"] = int({p: h for p, (_, h) in actual.items()} == expected)
    file_diff = sorted(set(actual) ^ set(expected))

    presses = cu.presses(cu.xinput_events(vb / "xinput.log"))
    ledger_ok = cu.ui_path_fs(presses, [ods.stat().st_mtime], window_s=1800.0, min_presses=20)
    dir_ok = cu.presses_before(presses, entered.stat().st_mtime, 1800.0) >= 3
    m["ui_path"] = int(ledger_ok and dir_ok)
    cu.emit(m, METRICS, presses=len(presses), new_rows=[list(k) if k else None for k in new_found],
            row_extras=extras[:10], file_diff=file_diff[:10])
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
