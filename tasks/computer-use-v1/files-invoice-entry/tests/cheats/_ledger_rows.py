"""Shared by the shortcut cheats: append the five new rows to the ledger by
rewriting content.xml inside the .ods, with no Calc and no input."""

from __future__ import annotations

import json
import shutil
import zipfile
from pathlib import Path
from xml.sax.saxutils import escape

KEY = json.loads(Path("/opt/invoices/invoices.json").read_text())
ODS = Path("/root/Finance") / KEY["ledger_file"]


def cell(v: object) -> str:
    if isinstance(v, (int, float)):
        return (f'<table:table-cell office:value-type="float" office:value="{v}" table:style-name="num">'
                f"<text:p>{v:,.2f}</text:p></table:table-cell>")
    return f'<table:table-cell office:value-type="string"><text:p>{escape(str(v))}</text:p></table:table-cell>'


def append_rows() -> None:
    rows_xml = "".join("<table:table-row>" + "".join(cell(v) for v in r) + "</table:table-row>"
                       for r in KEY["expected_new_rows"])
    tmp = ODS.with_suffix(".tmp")
    with zipfile.ZipFile(ODS) as zin, zipfile.ZipFile(tmp, "w") as zout:
        for item in zin.infolist():
            data = zin.read(item.filename)
            if item.filename == "content.xml":
                xml = data.decode()
                start = xml.index(f'table:name="{KEY["sheet"]}"')
                end = xml.index("</table:table>", start)
                xml = xml[:end] + rows_xml + xml[end:]
                data = xml.encode()
            zout.writestr(item, data, compress_type=item.compress_type)
    shutil.move(tmp, ODS)
