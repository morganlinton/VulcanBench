#!/usr/bin/env python3
"""End-state checker for files-receipts-filing.

    python3 check.py HOME SEED_JSON INPUT_LOG_DIR

HOME holds the copied Downloads/ and Finance/ trees; INPUT_LOG_DIR the copied
/var/lib/vbdesk (xinput.log). Every receipt must sit at its expected path with
its original content, the non-receipts and the pre-filed examples must be
untouched, nothing may be duplicated or left over, and the folders that
changed must have been preceded by real input on the display.
Shared helpers: /opt/vbdesk/cu_check.py (base image).
"""

from __future__ import annotations

import hashlib
import json
import sys
from pathlib import Path

sys.path.insert(0, "/opt/vbdesk")
import cu_check as cu  # noqa: E402

METRICS = (
    "artifact_present", "receipts_filed", "non_receipts_untouched", "examples_untouched",
    "no_leftovers", "ui_path",
)


def label_hash(label: str) -> str:
    return hashlib.sha256(label.encode()).hexdigest()[:16]


def main(argv: list[str]) -> int:
    home, seed_path, vb = Path(argv[1]), Path(argv[2]), Path(argv[3])
    seed = json.loads(seed_path.read_text())
    m: dict[str, int] = {k: 0 for k in METRICS}
    downloads, finance = home / "Downloads", home / "Finance"
    m["artifact_present"] = int(downloads.is_dir() and finance.is_dir())
    if not m["artifact_present"]:
        cu.emit(m, METRICS, note="Downloads or Finance missing from artifact")
        return 0

    def holds(path: Path, label: str) -> bool:
        return path.is_file() and label_hash(label).encode() in path.read_bytes()

    receipts = [e for e in seed["downloads"] if e["dest"]]
    others = [e for e in seed["downloads"] if not e["dest"]]
    m["receipts_filed"] = int(all(holds(home / e["dest"], e["name"]) for e in receipts))
    m["non_receipts_untouched"] = int(all(holds(downloads / e["name"], e["name"]) for e in others))
    m["examples_untouched"] = int(
        all(holds(home / e["path"], e["path"]) for e in seed["prefiled"])
        and (finance / "NAMING.txt").is_file()
        and (finance / "NAMING.txt").read_text() == seed["naming_txt"]
    )
    expected = {e["dest"] for e in receipts} | {e["path"] for e in seed["prefiled"]} | {"Finance/NAMING.txt"}
    expected |= {f"Downloads/{e['name']}" for e in others}
    actual = {p.relative_to(home).as_posix() for p in list(downloads.rglob("*")) + list(finance.rglob("*")) if p.is_file()}
    extra = sorted(actual - expected)
    missing = sorted(expected - actual)
    m["no_leftovers"] = int(not extra)

    # UI path: the month folders that gained files were written after real input
    events = cu.xinput_events(vb / "xinput.log")
    presses = cu.presses(events)
    changed_dirs = {(home / e["dest"]).parent for e in receipts}
    mtimes = [d.stat().st_mtime for d in changed_dirs if d.is_dir()]
    m["ui_path"] = int(len(mtimes) == len(changed_dirs) and cu.ui_path_fs(presses, mtimes, window_s=900.0, min_presses=3))
    cu.emit(m, METRICS, presses=len(presses), extra_files=extra[:10], missing_files=missing[:10])
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
