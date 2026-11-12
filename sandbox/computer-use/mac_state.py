"""Readers for macOS app state, for the ComputerUse macOS suite's checkers.

Each trial's guest disk is mounted read-only on the host after a clean
shutdown and the task's declared paths are copied to
/var/lib/vbmac/export/fs/<path relative to the guest's />; checkers read
those copies with these functions (standard library only). Nothing here
talks to a running Mac.

Stores (macOS 27):
- Notes: ~/Library/Group Containers/group.com.apple.notes/NoteStore.sqlite.
  Titles and folders are columns of ZICCLOUDSYNCINGOBJECT; a body is a
  gzipped protobuf in ZICNOTEDATA.ZDATA whose text is
  document(2) -> note(3) -> text(2).
- Reminders, Calendar and Contacts readers follow the same pattern and are
  added with the tasks that need them.
"""

from __future__ import annotations

import gzip
import json
import plistlib
import sqlite3
from dataclasses import dataclass
from pathlib import Path

HOME = "Users/admin"
NOTES_DB = f"{HOME}/Library/Group Containers/group.com.apple.notes/NoteStore.sqlite"
APPLE_EPOCH = 978307200  # 2001-01-01 in Unix time


# -- protobuf (just enough for Notes) ---------------------------------------
def _varint(buf: bytes, i: int) -> tuple[int, int]:
    shift = val = 0
    while True:
        b = buf[i]
        i += 1
        val |= (b & 0x7F) << shift
        if not b & 0x80:
            return val, i
        shift += 7


def pb_fields(buf: bytes) -> list[tuple[int, int, bytes | int]]:
    """Top-level (field number, wire type, value) of a protobuf message."""
    out: list[tuple[int, int, bytes | int]] = []
    i = 0
    while i < len(buf):
        key, i = _varint(buf, i)
        field, wire = key >> 3, key & 7
        if wire == 0:
            val, i = _varint(buf, i)
            out.append((field, wire, val))
        elif wire == 2:
            n, i = _varint(buf, i)
            out.append((field, wire, buf[i : i + n]))
            i += n
        elif wire == 1:
            out.append((field, wire, buf[i : i + 8]))
            i += 8
        elif wire == 5:
            out.append((field, wire, buf[i : i + 4]))
            i += 4
        else:
            raise ValueError(f"unsupported wire type {wire}")
    return out


def _first(buf: bytes, field: int) -> bytes | None:
    for f, w, v in pb_fields(buf):
        if f == field and w == 2 and isinstance(v, bytes):
            return v
    return None


def note_text(zdata: bytes | None) -> str:
    """The plain text of a note body blob (title line included)."""
    if not zdata:
        return ""
    raw = gzip.decompress(zdata) if zdata[:2] == b"\x1f\x8b" else zdata
    doc = _first(raw, 2)
    note = _first(doc, 3) if doc else None
    text = _first(note, 2) if note else None
    return text.decode("utf-8", errors="replace") if text else ""


# -- Notes ------------------------------------------------------------------
@dataclass
class Note:
    pk: int
    title: str
    text: str
    folder: str | None
    folder_type: int | None  # 1 = Recently Deleted
    deleted: bool
    created: float | None  # Unix time
    modified: float | None

    @property
    def body(self) -> str:
        """Everything after the first line (the title)."""
        return self.text.split("\n", 1)[1] if "\n" in self.text else ""


def _cols(con: sqlite3.Connection, table: str) -> set[str]:
    return {r[1] for r in con.execute(f"PRAGMA table_info({table})")}


def notes(root: Path) -> list[Note]:
    """Every note in the store, including ones in Recently Deleted (flagged)."""
    db = Path(root) / NOTES_DB
    con = sqlite3.connect(f"file:{db}?mode=ro", uri=True)
    try:
        cols = _cols(con, "ZICCLOUDSYNCINGOBJECT")
        folder_title = "f.ZTITLE2" if "ZTITLE2" in cols else "NULL"
        folder_type = "f.ZFOLDERTYPE" if "ZFOLDERTYPE" in cols else "NULL"
        created = "o.ZCREATIONDATE3" if "ZCREATIONDATE3" in cols else "o.ZCREATIONDATE1"
        modified = "o.ZMODIFICATIONDATE1"
        rows = con.execute(
            f"""SELECT o.Z_PK, o.ZTITLE1, d.ZDATA, {folder_title}, {folder_type},
                       o.ZMARKEDFORDELETION, {created}, {modified}
                FROM ZICCLOUDSYNCINGOBJECT o
                LEFT JOIN ZICNOTEDATA d ON d.ZNOTE = o.Z_PK
                LEFT JOIN ZICCLOUDSYNCINGOBJECT f ON f.Z_PK = o.ZFOLDER
                WHERE o.ZNOTEDATA IS NOT NULL OR d.ZNOTE IS NOT NULL"""
        ).fetchall()
    finally:
        con.close()
    out = []
    for pk, title, data, folder, ftype, marked, c, m in rows:
        out.append(
            Note(
                pk=pk,
                title=title or "",
                text=note_text(data),
                folder=folder,
                folder_type=ftype,
                deleted=bool(marked) or ftype == 1,
                created=c + APPLE_EPOCH if c else None,
                modified=m + APPLE_EPOCH if m else None,
            )
        )
    return out


def live_notes(root: Path) -> list[Note]:
    return [n for n in notes(root) if not n.deleted]


# -- generic ----------------------------------------------------------------
def read_plist(path: Path) -> dict:  # type: ignore[type-arg]
    with Path(path).open("rb") as f:
        data = plistlib.load(f)
    return data if isinstance(data, dict) else {}


def manifest(export: Path) -> dict:  # type: ignore[type-arg]
    p = Path(export) / "manifest.json"
    return json.loads(p.read_text()) if p.is_file() else {}


def input_events(path: Path) -> list[dict]:  # type: ignore[type-arg]
    """The MCP server's input log (one JSON object per event), or []."""
    p = Path(path)
    if not p.is_file():
        return []
    out = []
    for line in p.read_text(errors="replace").splitlines():
        try:
            out.append(json.loads(line))
        except ValueError:
            continue
    return out
