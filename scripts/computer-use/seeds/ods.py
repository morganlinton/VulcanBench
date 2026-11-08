"""Write minimal OpenDocument spreadsheets by hand, for ComputerUse v1 seeds.

    from ods import write_ods
    write_ods(out_path, [
        {"name": "Summary", "widths": ["co1", "co2"], "rows": [["Region", "Total"], ["North", None]]},
        {"name": "Data", "widths": ["co1", "co3", "co2"], "rows": [["a", "b", 1.5], {"row": ["c", "d", 2], "hidden": True}]},
    ])

A zip with mimetype (stored first), manifest and content.xml, so a task's
world is reproducible without LibreOffice on the host and the committed file
is byte-identical across runs (fixed zip timestamps). Numbers become float
cells with two decimals and grouping; strings become string cells; None or
"" an empty cell. A row given as {"row": [...], "hidden": true} is written
with table:visibility="collapse". Column style names: co1 (1.1in), co2
(1.3in), co3 (1.8in), co4 (2.6in). The verifier reads these files back with
cu_check.ods_sheets.
"""

from __future__ import annotations

import zipfile
from pathlib import Path
from xml.sax.saxutils import escape

MIME = "application/vnd.oasis.opendocument.spreadsheet"
ZIP_TIME = (2026, 3, 1, 8, 0, 0)
NS = (
    'xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" '
    'xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0" '
    'xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" '
    'xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" '
    'xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0" '
    'xmlns:number="urn:oasis:names:tc:opendocument:xmlns:datastyle:1.0" '
    'office:version="1.3"'
)
STYLES = (
    "<office:automatic-styles>"
    '<style:style style:name="co1" style:family="table-column"><style:table-column-properties style:column-width="1.1in"/></style:style>'
    '<style:style style:name="co2" style:family="table-column"><style:table-column-properties style:column-width="1.3in"/></style:style>'
    '<style:style style:name="co3" style:family="table-column"><style:table-column-properties style:column-width="1.8in"/></style:style>'
    '<style:style style:name="co4" style:family="table-column"><style:table-column-properties style:column-width="2.6in"/></style:style>'
    '<number:number-style style:name="N2"><number:number number:decimal-places="2" number:min-decimal-places="2" number:min-integer-digits="1" number:grouping="true"/></number:number-style>'
    '<style:style style:name="num" style:family="table-cell" style:data-style-name="N2"/>'
    "</office:automatic-styles>"
)
MANIFEST = (
    '<?xml version="1.0" encoding="UTF-8"?>'
    '<manifest:manifest xmlns:manifest="urn:oasis:names:tc:opendocument:xmlns:manifest:1.0" manifest:version="1.3">'
    f'<manifest:file-entry manifest:full-path="/" manifest:media-type="{MIME}"/>'
    '<manifest:file-entry manifest:full-path="content.xml" manifest:media-type="text/xml"/>'
    "</manifest:manifest>"
)


def cell(value: object) -> str:
    if value is None or value == "":
        return "<table:table-cell/>"
    if isinstance(value, bool):
        return f'<table:table-cell office:value-type="string"><text:p>{value}</text:p></table:table-cell>'
    if isinstance(value, (int, float)):
        return (
            f'<table:table-cell office:value-type="float" office:value="{value}" table:style-name="num">'
            f"<text:p>{value:,.2f}</text:p></table:table-cell>"
        )
    return f'<table:table-cell office:value-type="string"><text:p>{escape(str(value))}</text:p></table:table-cell>'


def row(values: list[object], hidden: bool = False) -> str:
    vis = ' table:visibility="collapse"' if hidden else ""
    return f"<table:table-row{vis}>" + "".join(cell(v) for v in values) + "</table:table-row>"


def sheet(name: str, rows: list[object], widths: list[str]) -> str:
    cols = "".join(f'<table:table-column table:style-name="{w}"/>' for w in widths)
    body = []
    for line in rows:
        if isinstance(line, dict):
            body.append(row(list(line["row"]), hidden=bool(line.get("hidden", False))))
        else:
            body.append(row(list(line)))
    return f'<table:table table:name="{escape(name)}">{cols}{"".join(body)}</table:table>'


def content_xml(sheets: list[dict]) -> str:
    tables = "".join(
        sheet(s["name"], s["rows"], s.get("widths") or ["co2"] * _ncols(s["rows"])) for s in sheets
    )
    return (
        '<?xml version="1.0" encoding="UTF-8"?>'
        f"<office:document-content {NS}>{STYLES}<office:body><office:spreadsheet>"
        f"{tables}</office:spreadsheet></office:body></office:document-content>"
    )


def _ncols(rows: list[object]) -> int:
    n = 0
    for line in rows:
        vals = line["row"] if isinstance(line, dict) else line
        n = max(n, len(vals))
    return max(n, 1)


def _entry(name: str) -> zipfile.ZipInfo:
    zi = zipfile.ZipInfo(name, date_time=ZIP_TIME)
    zi.external_attr = 0o644 << 16
    return zi


def write_ods(path: Path, sheets: list[dict]) -> Path:
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(path, "w") as z:
        z.writestr(_entry("mimetype"), MIME, compress_type=zipfile.ZIP_STORED)
        z.writestr(_entry("META-INF/manifest.xml"), MANIFEST, compress_type=zipfile.ZIP_DEFLATED)
        z.writestr(_entry("content.xml"), content_xml(sheets), compress_type=zipfile.ZIP_DEFLATED)
    return path
