"""Helpers for task seeds that run on the host against a stopped task VM's
Data volume (environment/seed_fs.py, called by build_task_image.py).

    sys.path.insert(0, str(Path(__file__).resolve().parents[4] / "scripts" / "computer-use-macos"))
    import seedlib as S

- `put(path, stamp, data=..., size_mb=...)`: write a file (bytes, text, or a
  sparse file of a given size) with its modification and Finder "Date Added"
  times set to `stamp` in the guest's zone.
- `pdf(pages)`: a small, valid PDF built from text lines in the standard
  Helvetica faces (no embedded fonts), so Preview renders it and pypdf reads
  its text back. Each page is a list of `Line`, `Box` or `Rule` items.
"""

from __future__ import annotations

import ctypes
import os
import struct
import zlib
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path
from zoneinfo import ZoneInfo

NY = ZoneInfo("America/New_York")
MB = 1024 * 1024
ATTR_CMN_ADDEDTIME = 0x10000000
_libc = ctypes.CDLL("libc.dylib", use_errno=True)


def stamp_ts(stamp: str) -> float:
    return datetime.strptime(stamp, "%Y-%m-%d %H:%M").replace(tzinfo=NY).timestamp()


def set_added(path: Path, t: float) -> None:
    """Finder's Date Added is the file system's added time, which would
    otherwise be the build's real date; set it to the file's own date."""
    attrlist = struct.pack("HHIIIII", 5, 0, ATTR_CMN_ADDEDTIME, 0, 0, 0, 0)
    buf = struct.pack("qq", int(t), 0)
    rc = _libc.setattrlist(
        os.fsencode(str(path)), ctypes.c_char_p(attrlist), ctypes.c_char_p(buf), len(buf), 0
    )
    if rc != 0:
        raise OSError(ctypes.get_errno(), f"setattrlist ADDEDTIME failed on {path}")


def put(path: Path, stamp: str, data: bytes | str | None = None, size_mb: float = 0.0) -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    if isinstance(data, str):
        path.write_text(data)
    elif isinstance(data, bytes):
        path.write_bytes(data)
    else:
        with path.open("wb") as f:
            f.truncate(int(size_mb * MB))
    t = stamp_ts(stamp)
    os.utime(path, (t, t))
    set_added(path, t)
    return path


def stamp_dir(path: Path, stamp: str) -> None:
    t = stamp_ts(stamp)
    os.utime(path, (t, t))
    set_added(path, t)


# -- PDF ----------------------------------------------------------------------
@dataclass
class Line:
    x: float
    y: float
    text: str
    size: float = 11
    bold: bool = False
    gray: float = 0.0  # 0 black, 1 white


@dataclass
class Box:
    """A stroked rectangle (a stamp border, a table frame)."""

    x: float
    y: float
    w: float
    h: float
    width: float = 1.0
    rgb: tuple[float, float, float] = (0, 0, 0)


@dataclass
class Rule:
    x0: float
    y0: float
    x1: float
    y1: float
    width: float = 0.5


@dataclass
class Stamp:
    """Rotated, colored text inside a box: a PAID or VOID stamp."""

    x: float
    y: float
    text: str
    size: float = 28
    angle_deg: float = 20
    rgb: tuple[float, float, float] = (0.8, 0.1, 0.1)


def _esc(s: str) -> str:
    s.encode("latin-1")  # refuse what the standard fonts cannot draw
    return s.replace("\\", "\\\\").replace("(", "\\(").replace(")", "\\)")


def _content(items: list[object]) -> bytes:
    import math  # noqa: PLC0415

    out: list[str] = []
    for it in items:
        if isinstance(it, Line):
            font = "F2" if it.bold else "F1"
            out.append(
                f"BT {it.gray:.2f} g /{font} {it.size:g} Tf {it.x:.1f} {it.y:.1f} Td ({_esc(it.text)}) Tj ET"
            )
        elif isinstance(it, Box):
            r, g, b = it.rgb
            out.append(f"q {r} {g} {b} RG {it.width:g} w {it.x} {it.y} {it.w} {it.h} re S Q")
        elif isinstance(it, Rule):
            out.append(f"q {it.width:g} w {it.x0} {it.y0} m {it.x1} {it.y1} l S Q")
        elif isinstance(it, Stamp):
            r, g, b = it.rgb
            a = math.radians(it.angle_deg)
            c, s = math.cos(a), math.sin(a)
            w = it.size * 0.72 * len(it.text) + 16
            h = it.size + 12
            out.append(
                f"q {c:.4f} {s:.4f} {-s:.4f} {c:.4f} {it.x} {it.y} cm {r} {g} {b} RG {r} {g} {b} rg "
                f"3 w 0 0 {w:.1f} {h:.1f} re S BT /F2 {it.size:g} Tf 8 8 Td ({_esc(it.text)}) Tj ET Q"
            )
        else:
            raise TypeError(it)
    return "\n".join(out).encode("latin-1")


def pdf(
    pages: list[list[object]], title: str = "", size: tuple[float, float] = (612, 792)
) -> bytes:
    """A PDF with one page per item list (US Letter by default)."""
    objs: list[bytes] = []

    def add(body: bytes) -> int:
        objs.append(body)
        return len(objs)

    catalog = add(b"")  # filled below
    pages_id = add(b"")
    f1 = add(b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>")
    f2 = add(
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>"
    )
    kids = []
    for items in pages:
        raw = zlib.compress(_content(items))
        cid = add(
            b"<< /Length %d /Filter /FlateDecode >>\nstream\n" % len(raw) + raw + b"\nendstream"
        )
        pid = add(
            (
                f"<< /Type /Page /Parent {pages_id} 0 R /MediaBox [0 0 {size[0]:g} {size[1]:g}] "
                f"/Resources << /Font << /F1 {f1} 0 R /F2 {f2} 0 R >> >> /Contents {cid} 0 R >>"
            ).encode()
        )
        kids.append(pid)
    objs[catalog - 1] = f"<< /Type /Catalog /Pages {pages_id} 0 R >>".encode()
    objs[pages_id - 1] = (
        f"<< /Type /Pages /Kids [{' '.join(f'{k} 0 R' for k in kids)}] /Count {len(kids)} >>"
    ).encode()
    info = add(f"<< /Title ({_esc(title)}) /Producer (VulcanBench seed) >>".encode())
    out = bytearray(b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n")
    offsets = []
    for i, body in enumerate(objs, 1):
        offsets.append(len(out))
        out += f"{i} 0 obj\n".encode() + body + b"\nendobj\n"
    xref = len(out)
    out += f"xref\n0 {len(objs) + 1}\n0000000000 65535 f \n".encode()
    for off in offsets:
        out += f"{off:010d} 00000 n \n".encode()
    out += (
        f"trailer\n<< /Size {len(objs) + 1} /Root {catalog} 0 R /Info {info} 0 R >>\n"
        f"startxref\n{xref}\n%%EOF\n"
    ).encode()
    return bytes(out)


def text_page(
    lines: list[str], size: float = 11, top: float = 740, left: float = 72
) -> list[object]:
    """A plain page of lines; a line starting with '# ' is a bold heading."""
    items: list[object] = []
    y = top
    for ln in lines:
        if ln.startswith("# "):
            items.append(Line(left, y, ln[2:], size + 5, bold=True))
            y -= (size + 5) * 1.6
        else:
            if ln:
                items.append(Line(left, y, ln, size))
            y -= size * 1.5
    return items
