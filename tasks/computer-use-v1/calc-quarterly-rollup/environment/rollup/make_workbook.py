#!/usr/bin/env python3
"""Write the task's spreadsheet from workbook.json (run at image build).

    python3 make_workbook.py WORKBOOK_JSON OUT.ods

A minimal OpenDocument spreadsheet written by hand (zip with mimetype,
manifest and content.xml), so the world is reproducible without LibreOffice
at build time. One row on the August sheet is hidden
(table:visibility="collapse"); its amount is part of the correct total.
The expected totals in the JSON are checked against the data here, so the
seed cannot drift from its own answer key.
"""

from __future__ import annotations

import json
import sys
import zipfile
from pathlib import Path
from xml.sax.saxutils import escape

NS = (
    'xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" '
    'xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0" '
    'xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" '
    'xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" '
    'xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0" '
    'xmlns:number="urn:oasis:names:tc:opendocument:xmlns:datastyle:1.0" '
    'office:version="1.3"'
)


def cell(value: object) -> str:
    if value is None or value == "":
        return "<table:table-cell/>"
    if isinstance(value, (int, float)):
        return (f'<table:table-cell office:value-type="float" office:value="{value}" table:style-name="num">'
                f"<text:p>{value:,.2f}</text:p></table:table-cell>")
    return f'<table:table-cell office:value-type="string"><text:p>{escape(str(value))}</text:p></table:table-cell>'


def row(values: list[object], hidden: bool = False) -> str:
    vis = ' table:visibility="collapse"' if hidden else ""
    return f"<table:table-row{vis}>" + "".join(cell(v) for v in values) + "</table:table-row>"


def sheet(name: str, rows: list[str], widths: list[str]) -> str:
    cols = "".join(f'<table:table-column table:style-name="{w}"/>' for w in widths)
    return f'<table:table table:name="{escape(name)}">{cols}{"".join(rows)}</table:table>'


def build(wb: dict) -> str:
    sheets = []
    s = wb["summary"]
    rows = [row(s["headers"])]
    for r in s["rows"]:
        rows.append(row([r["region"], None, r["target"], None]))
    sheets.append(sheet("Summary", rows, ["co1", "co2", "co2", "co3"]))
    for month, lines in wb["months"].items():
        rows = [row(["Region", "Customer", "Amount"])]
        for line in lines:
            if isinstance(line, dict):
                rows.append(row(line["row"], hidden=line.get("hidden", False)))
            else:
                rows.append(row(line))
        sheets.append(sheet(month, rows, ["co1", "co3", "co2"]))
    rows = [row(["Region", "Target"])] + [row([k, v]) for k, v in wb["targets"].items()]
    sheets.append(sheet("Targets", rows, ["co1", "co2"]))
    styles = (
        '<office:automatic-styles>'
        '<style:style style:name="co1" style:family="table-column"><style:table-column-properties style:column-width="1.1in"/></style:style>'
        '<style:style style:name="co2" style:family="table-column"><style:table-column-properties style:column-width="1.3in"/></style:style>'
        '<style:style style:name="co3" style:family="table-column"><style:table-column-properties style:column-width="1.8in"/></style:style>'
        '<number:number-style style:name="N2"><number:number number:decimal-places="2" number:min-decimal-places="2" number:min-integer-digits="1" number:grouping="true"/></number:number-style>'
        '<style:style style:name="num" style:family="table-cell" style:data-style-name="N2"/>'
        '</office:automatic-styles>'
    )
    return (
        '<?xml version="1.0" encoding="UTF-8"?>'
        f"<office:document-content {NS}>{styles}<office:body><office:spreadsheet>"
        + "".join(sheets) + "</office:spreadsheet></office:body></office:document-content>"
    )


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


def main() -> None:
    wb = json.loads(Path(sys.argv[1]).read_text())
    check_answer_key(wb)
    out = Path(sys.argv[2])
    out.parent.mkdir(parents=True, exist_ok=True)
    manifest = (
        '<?xml version="1.0" encoding="UTF-8"?>'
        '<manifest:manifest xmlns:manifest="urn:oasis:names:tc:opendocument:xmlns:manifest:1.0" manifest:version="1.3">'
        '<manifest:file-entry manifest:full-path="/" manifest:media-type="application/vnd.oasis.opendocument.spreadsheet"/>'
        '<manifest:file-entry manifest:full-path="content.xml" manifest:media-type="text/xml"/>'
        "</manifest:manifest>"
    )
    with zipfile.ZipFile(out, "w") as z:
        z.writestr(zipfile.ZipInfo("mimetype"), "application/vnd.oasis.opendocument.spreadsheet", compress_type=zipfile.ZIP_STORED)
        z.writestr("META-INF/manifest.xml", manifest, compress_type=zipfile.ZIP_DEFLATED)
        z.writestr("content.xml", build(wb), compress_type=zipfile.ZIP_DEFLATED)
    print(f"wrote {out}")


if __name__ == "__main__":
    main()
