#!/usr/bin/env python3
"""preview-contract-packet checker: ~/Documents and ~/Downloads copied from
the stopped VM.

    check.py /var/lib/vbmac

The packet's pages are identified by their text (pypdf): the draft's cover
and its pages with sections 1, 4 and 7, Exhibit A's two pages, and the
signed signature page. The originals are identified by SHA-256: Preview
saves edits back into a file it opened, so editing the draft in place
changes it.

Sub-metrics (reward is 1 only when all are 1):
  artifact_present   ~/Documents was collected
  packet_saved       Documents/"Acme MSA - executed.pdf" exists and opens
  content_complete   each of the seven expected pages is in it exactly once
  excluded_pages     no blank page, no second copy of the section 4 page and
                     no unsigned signature page
  order_right        cover, sections 1, 4, 7, Exhibit A 1 and 2, signed page
  originals_kept     the three Acme files (and the two unrelated PDFs) are
                     unchanged, under their names
  ui_path            the MCP server logged pointer input
"""

from __future__ import annotations

import hashlib
import sys
from pathlib import Path

sys.path.insert(0, "/opt/vbdesk")
import cu_check as cu  # noqa: E402
import mac_state as M  # noqa: E402

METRICS = (
    "artifact_present",
    "packet_saved",
    "content_complete",
    "excluded_pages",
    "order_right",
    "originals_kept",
    "ui_path",
)
PACKET = "Documents/Acme MSA - executed.pdf"
EXPECTED = ["cover", "s1", "s4", "s7", "ex1", "ex2", "signed"]
MARKERS = [  # first match wins
    ("signed", "/s/ jordan lee"),
    ("unsigned", "signature: ____"),
    ("ex2", "exhibit a (continued)"),
    ("ex1", "exhibit a: services and fees"),
    ("cover", "draft 3, for signature"),
    ("s1", "1. services"),
    ("s4", "4. fees"),
    ("s7", "7. term"),
]
ORIGINALS = {
    "Downloads/Acme MSA - draft 3.pdf": "8c6503c01ab66fb2a6c119cf41bcb4da0097fe31ec93f0481923631916e2d324",
    "Downloads/Acme MSA - Exhibit A.pdf": "324726f1514862ce86e3f8ff1a23366bbe9f39cd3d8795a5ec4c79ad54cba80b",
    "Downloads/Acme MSA - signature page (signed).pdf": "056b19c4f04a7cf449a549c5b06799cf074684ff4d835dda82ad07cd63d08b42",
    "Downloads/Harbor NDA - template.pdf": "928c4e25f72a2b9a6fc684f72c911db9dbd8fe1f129dfab7292a0473e25d3bcf",
    "Documents/Contracts/Northwind MSA - executed.pdf": "c94fb385d7965e0a5eca0a7a55776084755543fd920ab98df0f4505a2363ecab",
}


def classify(text: str) -> str:
    t = " ".join(text.lower().split())
    if len(t.replace(".", "").strip()) == 0:
        return "blank"
    for name, marker in MARKERS:
        if marker in t:
            return name
    return "other"


def page_kinds(path: Path) -> list[str] | None:
    try:
        from pypdf import PdfReader  # noqa: PLC0415

        return [classify(p.extract_text() or "") for p in PdfReader(str(path)).pages]
    except Exception:  # noqa: BLE001 - any unreadable file is "not saved"
        return None


def main(argv: list[str]) -> int:
    root = Path(argv[1])
    export = root / "export"
    home = export / "fs" / M.HOME
    m = {k: 0 for k in METRICS}
    extra: dict[str, object] = {"shutdown": M.manifest(export).get("shutdown", "none")}
    if not (home / "Documents").is_dir():
        cu.emit(m, METRICS, **extra)
        return 0
    m["artifact_present"] = 1
    kinds = page_kinds(home / PACKET) if (home / PACKET).is_file() else None
    extra["packet_pages"] = kinds
    if kinds is not None:
        m["packet_saved"] = 1
        m["content_complete"] = int(all(kinds.count(k) == 1 for k in EXPECTED))
        m["excluded_pages"] = int(
            "blank" not in kinds and "unsigned" not in kinds and kinds.count("s4") <= 1
        )
        m["order_right"] = int(kinds == EXPECTED)
    kept = {}
    for rel, digest in ORIGINALS.items():
        p = home / rel
        kept[rel] = p.is_file() and hashlib.sha256(p.read_bytes()).hexdigest() == digest
    extra["originals"] = {k: v for k, v in kept.items() if not v}
    m["originals_kept"] = int(all(kept.values()))
    events_log = M.input_events(root / "input.log")
    m["ui_path"] = int(bool({e.get("kind") for e in events_log} & {"click", "drag"}))
    extra["input_events"] = len(events_log)
    cu.emit(m, METRICS, **extra)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
