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
- Reminders: ~/Library/Group Containers/group.com.apple.reminders/
  Container_v1/Stores/Data-*.sqlite (one file per account; the local one
  holds the rows). ZREMCDREMINDER has ZTITLE, ZNOTES, ZDUEDATE (seconds
  from 2001-01-01, UTC; an all-day date is midnight UTC), ZALLDAY,
  ZCOMPLETED, ZMARKEDFORDELETION and ZLIST -> ZREMCDBASELIST.ZNAME.
- Calendar: ~/Library/Group Containers/group.com.apple.calendar/
  Calendar.sqlitedb. CalendarItem rows carry summary, start_date and
  end_date (seconds from 2001-01-01, UTC), all_day, calendar_id ->
  Calendar.title and has_recurrences; a series is one master row, and an
  occurrence edited on its own becomes a detached row whose orig_item_id is
  the master and orig_date its original start. Recurrence (owner_id) holds
  the rule and ExceptionDate (owner_id) the occurrences removed from a series.
"""

from __future__ import annotations

import gzip
import json
import plistlib
import sqlite3
from dataclasses import dataclass
from datetime import UTC, date, datetime
from pathlib import Path
from zoneinfo import ZoneInfo

HOME = "Users/admin"
NOTES_DB = f"{HOME}/Library/Group Containers/group.com.apple.notes/NoteStore.sqlite"
CALENDAR_DB = f"{HOME}/Library/Group Containers/group.com.apple.calendar/Calendar.sqlitedb"
REMINDERS_STORES = f"{HOME}/Library/Group Containers/group.com.apple.reminders/Container_v1/Stores"
APPLE_EPOCH = 978307200  # 2001-01-01 in Unix time
GUEST_TZ = ZoneInfo("America/New_York")  # base-config.sh


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


# -- Reminders --------------------------------------------------------------
@dataclass
class Reminder:
    title: str
    notes: str
    list: str | None
    due: float | None  # Unix time
    all_day: bool
    completed: bool
    deleted: bool
    pk: int = 0
    parent: int | None = None  # Z_PK of the reminder this is a subtask of
    flagged: bool = False
    priority: int = 0  # 0 none, 1 high, 5 medium, 9 low

    @property
    def due_date(self) -> date | None:
        """The calendar day it is due, as the guest shows it."""
        if self.due is None:
            return None
        if self.all_day:  # stored as midnight UTC of that day
            return datetime.fromtimestamp(self.due, UTC).date()
        return datetime.fromtimestamp(self.due, GUEST_TZ).date()


def reminders(root: Path) -> list[Reminder]:
    """Every reminder in every local Reminders store, deleted ones flagged."""
    out: list[Reminder] = []
    stores = sorted((Path(root) / REMINDERS_STORES).glob("Data-*.sqlite"))
    for db in stores:
        con = sqlite3.connect(f"file:{db}?mode=ro", uri=True)
        try:
            cols = _cols(con, "ZREMCDREMINDER")
            opt = ", ".join(
                f"r.{c}" if c in cols else "NULL" for c in ("ZPARENTREMINDER", "ZFLAGGED", "ZPRIORITY")
            )
            rows = con.execute(
                f"""SELECT r.ZTITLE, r.ZNOTES, l.ZNAME, r.ZDUEDATE, r.ZALLDAY, r.ZCOMPLETED,
                          r.ZMARKEDFORDELETION, l.ZMARKEDFORDELETION, r.Z_PK, {opt}
                   FROM ZREMCDREMINDER r LEFT JOIN ZREMCDBASELIST l ON l.Z_PK = r.ZLIST"""
            ).fetchall()
        except sqlite3.DatabaseError:
            rows = []
        finally:
            con.close()
        for title, notes_, lname, due, allday, done, gone, list_gone, pk, parent, flag, prio in rows:
            out.append(
                Reminder(
                    title=title or "",
                    notes=notes_ or "",
                    list=lname,
                    due=due + APPLE_EPOCH if due is not None else None,
                    all_day=bool(allday),
                    completed=bool(done),
                    deleted=bool(gone) or bool(list_gone),
                    pk=pk,
                    parent=parent,
                    flagged=bool(flag),
                    priority=int(prio or 0),
                )
            )
    return out


def reminder_lists(root: Path) -> list[str]:
    names: list[str] = []
    for db in sorted((Path(root) / REMINDERS_STORES).glob("Data-*.sqlite")):
        con = sqlite3.connect(f"file:{db}?mode=ro", uri=True)
        try:
            names += [
                r[0]
                for r in con.execute(
                    "SELECT ZNAME FROM ZREMCDBASELIST WHERE ZNAME IS NOT NULL "
                    "AND (ZMARKEDFORDELETION IS NULL OR ZMARKEDFORDELETION = 0)"
                )
            ]
        except sqlite3.DatabaseError:
            pass
        finally:
            con.close()
    return names


# -- Calendar ---------------------------------------------------------------
@dataclass
class Event:
    rowid: int
    summary: str
    calendar: str | None
    start: float | None  # Unix time
    end: float | None
    all_day: bool
    recurring: bool
    orig_item: int  # master ROWID for a detached occurrence, else 0
    orig_start: float | None  # a detached occurrence's original start
    rule: tuple[int, int, str, float | None, int] | None  # (freq, interval, specifier, end, count)
    exceptions: list[float]

    def local(self, t: float | None) -> datetime | None:
        return datetime.fromtimestamp(t, GUEST_TZ) if t is not None else None

    @property
    def start_local(self) -> datetime | None:
        return self.local(self.start)


def _apple(t: float | None) -> float | None:
    return t + APPLE_EPOCH if t is not None else None


def events(root: Path) -> list[Event]:
    db = Path(root) / CALENDAR_DB
    con = sqlite3.connect(f"file:{db}?mode=ro", uri=True)
    try:
        rules = {
            r[0]: (r[1], r[2], r[3] or "", _apple(r[4]), r[5] or 0)
            for r in con.execute(
                "SELECT owner_id, frequency, interval, specifier, end_date, count FROM Recurrence"
            )
        }
        exc: dict[int, list[float]] = {}
        for owner, d in con.execute("SELECT owner_id, date FROM ExceptionDate"):
            if d is not None:
                exc.setdefault(owner, []).append(d + APPLE_EPOCH)
        rows = con.execute(
            """SELECT ci.ROWID, ci.summary, c.title, ci.start_date, ci.end_date, ci.all_day,
                      ci.has_recurrences, ci.orig_item_id, ci.orig_date
               FROM CalendarItem ci LEFT JOIN Calendar c ON c.ROWID = ci.calendar_id
               WHERE ci.entity_type IS NULL OR ci.entity_type = 2 OR ci.entity_type = 0"""
        ).fetchall()
    finally:
        con.close()
    return [
        Event(
            rowid=rid,
            summary=summ or "",
            calendar=cal,
            start=_apple(sd),
            end=_apple(ed),
            all_day=bool(ad),
            recurring=bool(hr),
            orig_item=oi or 0,
            orig_start=_apple(od) if oi else None,
            rule=rules.get(rid),
            exceptions=sorted(exc.get(rid, [])),
        )
        for rid, summ, cal, sd, ed, ad, hr, oi, od in rows
    ]


# -- Contacts ---------------------------------------------------------------
CONTACTS_DB = f"{HOME}/Library/Application Support/AddressBook/AddressBook-v22.abcddb"
_ENT_CONTACT, _ENT_GROUP = "ABCDContact", "ABCDGroup"


def _label(raw: str | None) -> str:
    """'_$!<Mobile>!$_' (a built-in label) or 'mobile' -> 'mobile'."""
    s = (raw or "").strip()
    if s.startswith("_$!<") and s.endswith(">!$_"):
        s = s[4:-4]
    return s.lower()


def phone_key(number: str) -> str:
    """A phone number compared by its last ten digits."""
    digits = "".join(ch for ch in number if ch.isdigit())
    return digits[-10:]


@dataclass
class Contact:
    pk: int
    first: str
    last: str
    org: str
    title: str
    nickname: str
    phones: list[tuple[str, str]]  # (label, number)
    emails: list[tuple[str, str]]  # (label, address)
    addresses: list[dict]  # type: ignore[type-arg]
    note: str
    groups: list[str]

    @property
    def name(self) -> str:
        return " ".join(p for p in (self.first, self.last) if p).strip()

    @property
    def phone_keys(self) -> set[str]:
        return {phone_key(n) for _, n in self.phones if phone_key(n)}

    @property
    def email_keys(self) -> set[str]:
        return {a.strip().lower() for _, a in self.emails if a.strip()}


def _ents(con: sqlite3.Connection) -> dict[str, int]:
    return {name: ent for ent, name in con.execute("SELECT Z_ENT, Z_NAME FROM Z_PRIMARYKEY")}


def contacts(root: Path) -> list[Contact]:
    """Every person card in the local Contacts store ("On My Mac")."""
    db = Path(root) / CONTACTS_DB
    con = sqlite3.connect(f"file:{db}?mode=ro", uri=True)
    try:
        ents = _ents(con)
        pe, ge = ents.get(_ENT_CONTACT, 22), ents.get(_ENT_GROUP, 19)
        nick = "ZNICKNAME" if "ZNICKNAME" in _cols(con, "ZABCDRECORD") else "NULL"
        people = con.execute(
            f"""SELECT Z_PK, ZFIRSTNAME, ZLASTNAME, ZORGANIZATION, ZJOBTITLE, {nick}
                FROM ZABCDRECORD WHERE Z_ENT = ?""",
            (pe,),
        ).fetchall()
        groups = dict(con.execute("SELECT Z_PK, ZNAME FROM ZABCDRECORD WHERE Z_ENT = ?", (ge,)))
        membership: dict[int, list[str]] = {}
        table = f"Z_{pe}PARENTGROUPS"
        mcols = _cols(con, table)
        if mcols:
            ccol = next(c for c in mcols if c.endswith("CONTACTS"))
            gcol = next(c for c in mcols if "PARENTGROUPS" in c)
            for cpk, gpk in con.execute(f"SELECT {ccol}, {gcol} FROM {table}"):
                if gpk in groups:
                    membership.setdefault(cpk, []).append(groups[gpk])
        phones: dict[int, list[tuple[str, str]]] = {}
        for owner, label, num in con.execute(
            "SELECT ZOWNER, ZLABEL, ZFULLNUMBER FROM ZABCDPHONENUMBER ORDER BY ZOWNER, Z_PK"
        ):
            phones.setdefault(owner, []).append((_label(label), num or ""))
        emails: dict[int, list[tuple[str, str]]] = {}
        for owner, label, addr in con.execute(
            "SELECT ZOWNER, ZLABEL, ZADDRESS FROM ZABCDEMAILADDRESS ORDER BY ZOWNER, Z_PK"
        ):
            emails.setdefault(owner, []).append((_label(label), addr or ""))
        addrs: dict[int, list[dict]] = {}  # type: ignore[type-arg]
        for owner, label, street, city, state, zipc in con.execute(
            "SELECT ZOWNER, ZLABEL, ZSTREET, ZCITY, ZSTATE, ZZIPCODE FROM ZABCDPOSTALADDRESS"
        ):
            addrs.setdefault(owner, []).append(
                {
                    "label": _label(label),
                    "street": street or "",
                    "city": city or "",
                    "state": state or "",
                    "zip": zipc or "",
                }
            )
        notes_ = {
            owner: text or ""
            for owner, text in con.execute("SELECT ZCONTACT, ZTEXT FROM ZABCDNOTE")
            if owner is not None
        }
    finally:
        con.close()
    return [
        Contact(
            pk=pk,
            first=(first or "").strip(),
            last=(last or "").strip(),
            org=(org or "").strip(),
            title=(title or "").strip(),
            nickname=(nk or "").strip(),
            phones=phones.get(pk, []),
            emails=emails.get(pk, []),
            addresses=addrs.get(pk, []),
            note=notes_.get(pk, ""),
            groups=sorted(membership.get(pk, [])),
        )
        for pk, first, last, org, title, nk in people
    ]


def contact_groups(root: Path) -> list[str]:
    db = Path(root) / CONTACTS_DB
    con = sqlite3.connect(f"file:{db}?mode=ro", uri=True)
    try:
        ge = _ents(con).get(_ENT_GROUP, 19)
        return [r[0] for r in con.execute("SELECT ZNAME FROM ZABCDRECORD WHERE Z_ENT = ?", (ge,))]
    finally:
        con.close()


# -- preferences --------------------------------------------------------------
GLOBAL_PREFS = f"{HOME}/Library/Preferences/.GlobalPreferences.plist"
SAVED_SEARCHES = f"{HOME}/Library/Saved Searches"
TEXT_REPLACEMENTS_DB = f"{HOME}/Library/KeyboardServices/TextReplacements.db"


def text_replacements(root: Path) -> dict[str, str]:
    """Settings > Keyboard > Text Replacements: {shortcut: phrase}, from the
    store the keyboard services read (KeyboardServices/TextReplacements.db).
    The copy System Settings mirrors into the global domain
    (NSUserDictionaryReplacementItems) is not what makes a replacement work,
    so it is not read."""
    db = Path(root) / TEXT_REPLACEMENTS_DB
    if not db.is_file():
        return {}
    con = sqlite3.connect(f"file:{db}?mode=ro", uri=True)
    try:
        rows = con.execute(
            "SELECT ZSHORTCUT, ZPHRASE FROM ZTEXTREPLACEMENTENTRY "
            "WHERE ZWASDELETED IS NULL OR ZWASDELETED = 0 ORDER BY ZTIMESTAMP"
        ).fetchall()
    except sqlite3.DatabaseError:
        rows = []
    finally:
        con.close()
    return {str(k or ""): str(v or "") for k, v in rows}


def app_shortcuts(root: Path, domain: str) -> dict[str, str]:
    """App Shortcuts for one app (its NSUserKeyEquivalents): {menu title:
    key equivalent}, where '@' is Command, '^' Control, '~' Option, '$' Shift."""
    name = ".GlobalPreferences" if domain == "NSGlobalDomain" else domain
    p = Path(root) / HOME / "Library" / "Preferences" / f"{name}.plist"
    return dict(read_plist(p).get("NSUserKeyEquivalents", {})) if p.is_file() else {}


def saved_searches(root: Path) -> dict[str, dict]:  # type: ignore[type-arg]
    """Finder smart folders saved in ~/Library/Saved Searches: {name: plist}."""
    d = Path(root) / SAVED_SEARCHES
    if not d.is_dir():
        return {}
    return {p.stem: read_plist(p) for p in sorted(d.glob("*.savedSearch"))}


# -- RTF (TextEdit) -----------------------------------------------------------
@dataclass
class Para:
    text: str
    bold: bool  # every visible character is bold
    sizes: set[float]  # point sizes of the visible characters
    in_list: bool  # a list item (\lsN, as TextEdit's lists write)


_RTF_TOKEN = None
_SKIP_DESTS = {
    "fonttbl", "colortbl", "expandedcolortbl", "stylesheet", "info", "listtable",
    "listoverridetable", "pict", "header", "footer", "themedata", "colorschememapping",
}  # fmt: skip


def rtf_paragraphs(path: Path) -> list[Para]:  # noqa: PLR0912, PLR0915 - one tokenizer loop
    """The paragraphs of an RTF file as TextEdit writes it, with per-paragraph
    bold, point sizes and list membership. Enough of RTF for that: groups,
    \\b, \\fs, \\pard, \\lsN, \\par (or a backslash before a newline),
    \\'hh and \\uN escapes; destinations (font and color tables, list
    tables, \\* groups) and list markers ({\\listtext ...}) are skipped."""
    import re  # noqa: PLC0415

    global _RTF_TOKEN  # noqa: PLW0603
    if _RTF_TOKEN is None:
        _RTF_TOKEN = re.compile(
            r"\\([a-zA-Z]+)(-?\d+)? ?|\\'([0-9a-fA-F]{2})|\\([^a-zA-Z])|([{}])|([^\\{}\r\n]+)|[\r\n]"
        )
    data = Path(path).read_bytes().decode("latin-1")
    paras: list[Para] = []
    chars: list[tuple[str, bool, float]] = []
    in_list = False
    stack: list[dict] = []  # type: ignore[type-arg]
    st = {"b": False, "fs": 24, "skip": False, "uc": 1}
    pending_skip = 0
    first_in_group = False

    def end_para() -> None:
        nonlocal chars, in_list
        text = "".join(c for c, _, _ in chars)
        vis = [(c, b, s) for c, b, s in chars if not c.isspace()]
        paras.append(
            Para(
                text=text.strip(),
                bold=bool(vis) and all(b for _, b, _ in vis),
                sizes={s for _, _, s in vis},
                in_list=in_list,
            )
        )
        chars = []

    def emit(s: str) -> None:
        nonlocal pending_skip
        if st["skip"]:
            return
        for ch in s:
            if pending_skip:
                pending_skip -= 1
                continue
            chars.append((ch, st["b"], st["fs"] / 2))

    for mt in _RTF_TOKEN.finditer(data):
        word, num, hexd, sym, brace, text = mt.groups()
        if brace == "{":
            stack.append(dict(st))
            first_in_group = True
            continue
        if brace == "}":
            if stack:
                st = stack.pop()
            first_in_group = False
            continue
        was_first, first_in_group = first_in_group, False
        if word is not None:
            if was_first and (word in _SKIP_DESTS or word == "listtext"):
                st["skip"] = True
                if word == "listtext":
                    in_list = True
                continue
            if word == "b":
                st["b"] = num != "0"
            elif word == "fs" and num:
                st["fs"] = int(num)
            elif word == "pard":
                in_list = False
            elif word == "ls":
                in_list = True
            elif word == "par" and not st["skip"]:
                end_para()
            elif word == "uc" and num:
                st["uc"] = int(num)
            elif word == "u" and num:
                emit(chr(int(num) % 65536))
                pending_skip = st["uc"]
            elif word == "tab":
                emit("\t")
            continue
        if sym is not None:
            if sym == "*" and was_first:
                st["skip"] = True
            elif sym in "\n\r" and not st["skip"]:
                end_para()
            elif sym in "\\{}":
                emit(sym)
            elif sym == "~":
                emit(" ")
            continue
        if hexd is not None:
            emit(bytes([int(hexd, 16)]).decode("cp1252", errors="replace"))
            continue
        if text:
            emit(text)
    if chars:
        end_para()
    return [p for p in paras if p.text or p.in_list]


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
