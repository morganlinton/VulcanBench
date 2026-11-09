"""Write small text PDFs by hand, for ComputerUse v1 seeds.

    from pdf import Page, write_pdf
    page = Page()
    page.text(72, 760, "INVOICE", size=18, bold=True)
    page.text(72, 730, "Invoice number: INV-7741")
    page.rule(72, 720, 523, 720)
    page.box(72, 600, 451, 100)
    write_pdf(out_path, [page])

One or more A4 pages of Helvetica text (regular and bold, the standard
fonts every viewer carries, so nothing is embedded), horizontal rules and
rectangles. Coordinates are PDF points with the origin at the bottom left
(A4 is 595 x 842). The bytes are a pure function of the content (no dates,
no ids, no compression), so a committed PDF is identical on every run and
the verifier can hash it. Evince in the base image renders these as a
person's PDF viewer would.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path

A4 = (595, 842)
# Helvetica advance widths (per 1000 em) for the printable ASCII range, from
# the Adobe AFM. Used only to right-align and centre text.
_WIDTHS = (
    278,
    278,
    355,
    556,
    556,
    889,
    667,
    191,
    333,
    333,
    389,
    584,
    278,
    333,
    278,
    278,
    556,
    556,
    556,
    556,
    556,
    556,
    556,
    556,
    556,
    556,
    278,
    278,
    584,
    584,
    584,
    556,
    1015,
    667,
    667,
    722,
    722,
    667,
    611,
    778,
    722,
    278,
    500,
    667,
    556,
    833,
    722,
    778,
    667,
    778,
    722,
    667,
    611,
    722,
    667,
    944,
    667,
    667,
    611,
    278,
    278,
    278,
    469,
    556,
    333,
    556,
    556,
    500,
    556,
    556,
    278,
    556,
    556,
    222,
    222,
    500,
    222,
    833,
    556,
    556,
    556,
    556,
    333,
    500,
    278,
    556,
    500,
    722,
    500,
    500,
    500,
    334,
    260,
    334,
    584,
)
_BOLD_WIDTHS = (
    278,
    333,
    474,
    556,
    556,
    889,
    722,
    238,
    333,
    333,
    389,
    584,
    278,
    333,
    278,
    278,
    556,
    556,
    556,
    556,
    556,
    556,
    556,
    556,
    556,
    556,
    333,
    333,
    584,
    584,
    584,
    611,
    975,
    722,
    722,
    722,
    722,
    667,
    611,
    778,
    722,
    278,
    556,
    722,
    611,
    833,
    722,
    778,
    667,
    778,
    722,
    667,
    611,
    722,
    667,
    944,
    667,
    667,
    611,
    333,
    278,
    333,
    584,
    556,
    333,
    556,
    611,
    556,
    611,
    556,
    333,
    611,
    611,
    278,
    278,
    556,
    278,
    889,
    611,
    611,
    611,
    611,
    389,
    556,
    333,
    611,
    556,
    778,
    556,
    556,
    500,
    389,
    280,
    389,
    584,
)


def text_width(text: str, size: float, bold: bool = False) -> float:
    table = _BOLD_WIDTHS if bold else _WIDTHS
    total = 0
    for ch in text:
        code = ord(ch)
        total += table[code - 32] if 32 <= code < 127 else 556
    return total * size / 1000.0


def _escape(text: str) -> str:
    out = []
    for ch in text:
        if ch in "()\\":
            out.append("\\" + ch)
        elif 32 <= ord(ch) < 127:
            out.append(ch)
        else:
            # WinAnsi code points the standard fonts know (pound, euro-less)
            code = {
                "\u00a3": 0o243,  # pound
                "\u00e9": 0o351,  # e acute
                "\u00e8": 0o350,  # e grave
                "\u00fc": 0o374,  # u diaeresis
                "\u00f6": 0o366,  # o diaeresis
                "\u00e4": 0o344,  # a diaeresis
                "\u2019": 0o222,  # right single quotation mark
                "\u2013": 0o226,  # en dash
                "\u00b0": 0o260,  # degree
            }.get(ch)
            out.append(f"\\{code:03o}" if code is not None else "?")
    return "".join(out)


@dataclass
class Page:
    size: tuple[int, int] = A4
    ops: list[str] = field(default_factory=list)

    def text(
        self, x: float, y: float, s: str, size: float = 11, bold: bool = False, align: str = "left"
    ) -> None:
        """Draw one line of text with its baseline at y. align: left, right or center."""
        w = text_width(s, size, bold)
        if align == "right":
            x -= w
        elif align == "center":
            x -= w / 2
        font = "/F2" if bold else "/F1"
        self.ops.append(f"BT {font} {size:g} Tf {x:.2f} {y:.2f} Td ({_escape(s)}) Tj ET")

    def lines(
        self,
        x: float,
        y: float,
        rows: list[str],
        size: float = 11,
        leading: float | None = None,
        bold: bool = False,
    ) -> float:
        """Several lines from the top; returns the baseline of the next free line."""
        step = leading or size * 1.35
        for row in rows:
            self.text(x, y, row, size=size, bold=bold)
            y -= step
        return y

    def rule(self, x0: float, y0: float, x1: float, y1: float, width: float = 0.6) -> None:
        self.ops.append(f"{width:g} w {x0:.2f} {y0:.2f} m {x1:.2f} {y1:.2f} l S")

    def box(
        self, x: float, y: float, w: float, h: float, width: float = 0.6, fill: float | None = None
    ) -> None:
        """A rectangle with its lower-left corner at (x, y); fill is a grey level 0..1."""
        if fill is not None:
            self.ops.append(f"{fill:g} g {x:.2f} {y:.2f} {w:.2f} {h:.2f} re f 0 g")
        self.ops.append(f"{width:g} w {x:.2f} {y:.2f} {w:.2f} {h:.2f} re S")

    def table(
        self,
        x: float,
        y: float,
        cols: list[tuple[str, float, str]],
        rows: list[list[str]],
        size: float = 10,
        row_h: float = 18,
        header: bool = True,
    ) -> float:
        """A ruled table. cols: (title, width, align). Returns the y below the table."""
        total_w = sum(c[1] for c in cols)
        top = y
        if header:
            self.box(x, y - row_h, total_w, row_h, fill=0.92)
            cx = x
            for title, w, align in cols:
                tx = cx + 4 if align == "left" else (cx + w - 4 if align == "right" else cx + w / 2)
                self.text(tx, y - row_h + 5.5, title, size=size, bold=True, align=align)
                cx += w
            y -= row_h
        for r in rows:
            cx = x
            for (_title, w, align), val in zip(cols, r, strict=True):
                tx = cx + 4 if align == "left" else (cx + w - 4 if align == "right" else cx + w / 2)
                self.text(tx, y - row_h + 5.5, val, size=size, align=align)
                cx += w
            y -= row_h
        self.box(x, y, total_w, top - y)
        cx = x
        for _, w, _ in cols[:-1]:
            cx += w
            self.rule(cx, y, cx, top)
        return y


def write_pdf(path: Path, pages: list[Page], title: str = "") -> Path:
    """Serialise the pages; the output depends only on the pages and the title."""
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    objs: list[bytes] = []

    def add(body: str) -> int:
        objs.append(body.encode("latin-1"))
        return len(objs)

    add("<< /Type /Catalog /Pages 2 0 R >>")  # 1
    add("")  # 2: pages, filled below
    f1 = add("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>")
    f2 = add(
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>"
    )
    page_ids = []
    for pg in pages:
        content = "\n".join(pg.ops) + "\n"
        c_id = add(f"<< /Length {len(content.encode('latin-1'))} >>\nstream\n{content}endstream")
        w, h = pg.size
        p_id = add(
            f"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {w} {h}] "
            f"/Resources << /Font << /F1 {f1} 0 R /F2 {f2} 0 R >> >> /Contents {c_id} 0 R >>"
        )
        page_ids.append(p_id)
    kids = " ".join(f"{i} 0 R" for i in page_ids)
    objs[1] = f"<< /Type /Pages /Kids [{kids}] /Count {len(page_ids)} >>".encode("latin-1")
    info = add(f"<< /Title ({_escape(title)}) /Producer (VulcanBench seeds) >>") if title else None

    out = bytearray(b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n")
    offsets = []
    for i, body in enumerate(objs, 1):
        offsets.append(len(out))
        out += f"{i} 0 obj\n".encode() + body + b"\nendobj\n"
    xref = len(out)
    out += f"xref\n0 {len(objs) + 1}\n0000000000 65535 f \n".encode()
    for off in offsets:
        out += f"{off:010d} 00000 n \n".encode()
    trailer = (
        f"<< /Size {len(objs) + 1} /Root 1 0 R" + (f" /Info {info} 0 R" if info else "") + " >>"
    )
    out += f"trailer\n{trailer}\nstartxref\n{xref}\n%%EOF\n".encode()
    path.write_bytes(bytes(out))
    return path
