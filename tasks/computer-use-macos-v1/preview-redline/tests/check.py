#!/usr/bin/env python3
"""preview-redline checker: ~/Documents/Contracts copied from the stopped VM.

    check.py /var/lib/vbmac

The output is "Northwind MSA - for Northwind.pdf", read with pypdf. Line
positions are PDF points on the unrotated page (environment/seed_fs.py).

Sub-metrics (reward is 1 only when all are 1):
  artifact_present  the Contracts folder was collected
  original_kept     the draft is byte-for-byte as seeded
  exported          the output exists and has four pages
  redacted          none of the three bank numbers can be extracted from the
                    output's text, while the text around them still can (so
                    the pages were not flattened to images)
  highlights        a highlight annotation covers clause 5.2 on page 3 and
                    one covers clause 7.1 on page 4
  note_added        a note or text box on page 2 says 45
  rotation_fixed    page 3 is upright (rotation 0 modulo 360)
  ui_path           the MCP server logged pointer input
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
    "original_kept",
    "exported",
    "redacted",
    "highlights",
    "note_added",
    "rotation_fixed",
    "ui_path",
)
DIR = f"{M.HOME}/Documents/Contracts"
DRAFT_SHA = "1da2674551334713e5502fd7ee75e46dec0130819337ff67df766ba8989c89f7"
SECRETS = ("44172290118503", "021000089", "773100459921")
KEEP = {1: ("Fees and payment", "operating account"), 3: ("Termination", "Remittance details")}
HIGHLIGHT = {2: (585.0, 625.0), 3: (675.0, 715.0)}  # page index: clause y range


def annots(page) -> list[dict]:  # type: ignore[no-untyped-def,type-arg]
    out = []
    for ref in page.get("/Annots") or []:
        try:
            a = ref.get_object()
        except Exception:  # noqa: BLE001
            continue
        out.append(a)
    return out


def covers(a: dict, lo: float, hi: float) -> bool:  # type: ignore[type-arg]
    rect = a.get("/Rect")
    if rect is None:
        return False
    y0, y1 = sorted((float(rect[1]), float(rect[3])))
    return y0 < hi and y1 > lo


def main(argv: list[str]) -> int:
    root = Path(argv[1])
    export = root / "export"
    d = export / "fs" / DIR
    m = {k: 0 for k in METRICS}
    extra: dict[str, object] = {"shutdown": M.manifest(export).get("shutdown", "none")}
    if not d.is_dir():
        cu.emit(m, METRICS, **extra)
        return 0
    m["artifact_present"] = 1
    draft = d / "Northwind MSA - draft.pdf"
    m["original_kept"] = int(draft.is_file() and hashlib.sha256(draft.read_bytes()).hexdigest() == DRAFT_SHA)
    out = d / "Northwind MSA - for Northwind.pdf"
    extra["files"] = sorted(p.name for p in d.iterdir())
    if out.is_file():
        from pypdf import PdfReader  # noqa: PLC0415

        try:
            pages = PdfReader(str(out)).pages
        except Exception as exc:  # noqa: BLE001
            extra["read_error"] = repr(exc)[:200]
            pages = []
        m["exported"] = int(len(pages) == 4)
        if len(pages) == 4:
            texts = [p.extract_text() or "" for p in pages]
            digits = re.sub(r"\D", "", " ".join(texts))
            leaked = [s for s in SECRETS if s in digits]
            kept = all(all(k in texts[i] for k in ks) for i, ks in KEEP.items())
            extra["leaked"] = leaked
            extra["context_text_kept"] = kept
            m["redacted"] = int(not leaked and kept)
            hl = {}
            for i, (lo, hi) in HIGHLIGHT.items():
                hl[i] = [
                    [round(float(v)) for v in a.get("/Rect", [])] for a in annots(pages[i]) if a.get("/Subtype") == "/Highlight"
                ]
            extra["highlights"] = hl
            m["highlights"] = int(
                all(
                    any(a.get("/Subtype") == "/Highlight" and covers(a, lo, hi) for a in annots(pages[i]))
                    for i, (lo, hi) in HIGHLIGHT.items()
                )
            )
            notes = [
                str(a.get("/Contents") or "") for a in annots(pages[1]) if a.get("/Subtype") in ("/Text", "/FreeText")
            ]
            extra["notes"] = notes
            m["note_added"] = int(any("45" in n for n in notes))
            rot = int(pages[2].get("/Rotate") or 0) % 360
            extra["page3_rotate"] = rot
            m["rotation_fixed"] = int(rot == 0)
    events_log = M.input_events(root / "input.log")
    m["ui_path"] = int(bool({e.get("kind") for e in events_log} & {"click", "drag"}))
    extra["input_events"] = len(events_log)
    cu.emit(m, METRICS, **extra)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
