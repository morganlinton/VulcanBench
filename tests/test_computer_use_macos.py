"""ComputerUse macOS suite: the parts that run without a VM.

The RFB client's key mapping and PNG encoder, the mac_state readers on small
synthetic stores, the scroll endpoint's token check, and static checks of
every task in tasks/computer-use-macos-v1/suite.json.
"""

from __future__ import annotations

import gzip
import json
import sqlite3
import struct
import sys
import tomllib
import urllib.error
import urllib.request
import zlib
from datetime import date
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO / "sandbox" / "computer-use"))
import mac_state as M  # noqa: E402
import rfb  # noqa: E402

SUITE = REPO / "tasks" / "computer-use-macos-v1"
APPLE = 978307200


# -- rfb ----------------------------------------------------------------------
def test_chord_holds_modifiers_around_the_key() -> None:
    ev = rfb.KeyMapper().chord("cmd+shift+4")
    syms = [(e.keysym, e.down) for e in ev]
    assert syms == [
        (rfb.KEYSYMS["cmd"], True),
        (rfb.KEYSYMS["shift"], True),
        (ord("4"), True),
        (ord("4"), False),
        (rfb.KEYSYMS["shift"], False),
        (rfb.KEYSYMS["cmd"], False),
    ]
    # Command is Alt_L and Option is Meta_L on the VZ server.
    assert rfb.KEYSYMS["cmd"] == 0xFFE9 and rfb.KEYSYMS["option"] == 0xFFE7


def test_text_shifts_capitals_and_symbols_and_refuses_non_ascii() -> None:
    ev = rfb.KeyMapper().text("A!")
    downs = [e.keysym for e in ev if e.down]
    assert downs == [rfb.KEYSYMS["shift"], ord("a"), rfb.KEYSYMS["shift"], ord("1")]
    with pytest.raises(ValueError):
        rfb.KeyMapper().text("café")
    assert [e.keysym for e in rfb.KeyMapper().chord("cmd++") if e.down][-1] == ord("=")


def test_png_bytes_writes_a_decodable_image() -> None:
    w, h = 4, 2
    bgrx = bytes([10, 20, 30, 0]) * (w * h)
    png = rfb.png_bytes(w, h, bgrx, step=2)
    assert png[:8] == b"\x89PNG\r\n\x1a\n"
    iw, ih = struct.unpack(">II", png[16:24])
    assert (iw, ih) == (2, 1)
    idat = png[png.index(b"IDAT") + 4 : png.index(b"IEND") - 8]
    raw = zlib.decompress(idat)
    assert raw == b"\x00" + bytes([30, 20, 10]) * 2  # filter byte, then RGB


# -- mac_state ----------------------------------------------------------------
def _pb_len(field: int, payload: bytes) -> bytes:
    key, n = (field << 3) | 2, len(payload)
    out = bytes([key])
    while True:
        b = n & 0x7F
        n >>= 7
        out += bytes([b | (0x80 if n else 0)])
        if not n:
            break
    return out + payload


def test_note_text_reads_the_body_protobuf() -> None:
    body = b"Packing list\npassport"
    blob = gzip.compress(_pb_len(2, _pb_len(3, _pb_len(2, body))))
    assert M.note_text(blob) == "Packing list\npassport"
    assert M.note_text(None) == ""


def _reminders_store(root: Path) -> None:
    d = root / M.REMINDERS_STORES
    d.mkdir(parents=True)
    con = sqlite3.connect(d / "Data-test.sqlite")
    con.executescript(
        """CREATE TABLE ZREMCDBASELIST (Z_PK INTEGER PRIMARY KEY, ZNAME TEXT, ZMARKEDFORDELETION INTEGER);
           CREATE TABLE ZREMCDREMINDER (Z_PK INTEGER PRIMARY KEY, ZTITLE TEXT, ZNOTES TEXT, ZLIST INTEGER,
             ZDUEDATE REAL, ZALLDAY INTEGER, ZCOMPLETED INTEGER, ZMARKEDFORDELETION INTEGER);"""
    )
    con.execute("INSERT INTO ZREMCDBASELIST VALUES (1, 'Follow-ups', 0)")
    # 2026-03-12 21:00 UTC is 17:00 in New York (EDT); all-day is midnight UTC.
    timed = 1773349200 - APPLE
    allday = 1774915200 - APPLE  # 2026-03-31 00:00 UTC
    con.execute(
        "INSERT INTO ZREMCDREMINDER VALUES (1, 'Send numbers', '', 1, ?, 0, 0, 0)", (timed,)
    )
    con.execute("INSERT INTO ZREMCDREMINDER VALUES (2, 'Pay rent', '', 1, ?, 1, 0, 0)", (allday,))
    con.execute("INSERT INTO ZREMCDREMINDER VALUES (3, 'Gone', '', 1, NULL, 0, 0, 1)")
    con.commit()
    con.close()


def test_reminders_reader_gives_local_due_days(tmp_path: Path) -> None:
    _reminders_store(tmp_path)
    rems = {r.title: r for r in M.reminders(tmp_path)}
    assert rems["Send numbers"].due_date == date(2026, 3, 12)
    assert rems["Pay rent"].due_date == date(2026, 3, 31) and rems["Pay rent"].all_day
    assert rems["Gone"].deleted and rems["Gone"].due_date is None
    assert M.reminder_lists(tmp_path) == ["Follow-ups"]


def test_calendar_reader_links_detached_occurrences(tmp_path: Path) -> None:
    db = tmp_path / M.CALENDAR_DB
    db.parent.mkdir(parents=True)
    con = sqlite3.connect(db)
    con.executescript(
        """CREATE TABLE Calendar (ROWID INTEGER PRIMARY KEY, title TEXT);
           CREATE TABLE CalendarItem (ROWID INTEGER PRIMARY KEY, summary TEXT, calendar_id INTEGER,
             start_date REAL, end_date REAL, all_day INTEGER, has_recurrences INTEGER,
             orig_item_id INTEGER, orig_date REAL, entity_type INTEGER);
           CREATE TABLE Recurrence (owner_id INTEGER, frequency INTEGER, interval INTEGER,
             specifier TEXT, end_date REAL, count INTEGER);
           CREATE TABLE ExceptionDate (owner_id INTEGER, date REAL);"""
    )
    con.execute("INSERT INTO Calendar VALUES (1, 'Work')")
    master_start = 1772550000 - APPLE  # 2026-03-03 10:00 EST
    con.execute(
        "INSERT INTO CalendarItem VALUES (10, '1:1', 1, ?, ?, 0, 1, 0, NULL, 2)",
        (master_start, master_start + 1800),
    )
    con.execute("INSERT INTO Recurrence VALUES (10, 2, 1, '', NULL, 0)")
    moved = 1773928800 - APPLE  # 2026-03-19 10:00 EDT
    orig = 1773756000 - APPLE  # 2026-03-17 10:00 EDT
    con.execute(
        "INSERT INTO CalendarItem VALUES (11, '1:1', 1, ?, ?, 0, 0, 10, ?, 2)",
        (moved, moved + 1800, orig),
    )
    con.commit()
    con.close()
    evs = {e.rowid: e for e in M.events(tmp_path)}
    assert evs[10].rule == (2, 1, "", None, 0) and evs[10].calendar == "Work"
    assert evs[11].orig_item == 10 and evs[11].orig_start == orig + APPLE
    assert evs[11].start_local is not None and evs[11].start_local.hour == 10


def test_contacts_reader_joins_details_and_groups(tmp_path: Path) -> None:
    db = tmp_path / M.CONTACTS_DB
    db.parent.mkdir(parents=True)
    con = sqlite3.connect(db)
    con.executescript(
        """CREATE TABLE Z_PRIMARYKEY (Z_ENT INTEGER, Z_NAME TEXT);
           CREATE TABLE ZABCDRECORD (Z_PK INTEGER PRIMARY KEY, Z_ENT INTEGER, ZFIRSTNAME TEXT,
             ZLASTNAME TEXT, ZORGANIZATION TEXT, ZJOBTITLE TEXT, ZNICKNAME TEXT, ZNAME TEXT);
           CREATE TABLE ZABCDPHONENUMBER (Z_PK INTEGER PRIMARY KEY, ZOWNER INTEGER, ZLABEL TEXT, ZFULLNUMBER TEXT);
           CREATE TABLE ZABCDEMAILADDRESS (Z_PK INTEGER PRIMARY KEY, ZOWNER INTEGER, ZLABEL TEXT, ZADDRESS TEXT);
           CREATE TABLE ZABCDPOSTALADDRESS (Z_PK INTEGER PRIMARY KEY, ZOWNER INTEGER, ZLABEL TEXT,
             ZSTREET TEXT, ZCITY TEXT, ZSTATE TEXT, ZZIPCODE TEXT);
           CREATE TABLE ZABCDNOTE (Z_PK INTEGER PRIMARY KEY, ZCONTACT INTEGER, ZTEXT TEXT);
           CREATE TABLE Z_22PARENTGROUPS (Z_22CONTACTS INTEGER, Z_19PARENTGROUPS1 INTEGER);
           INSERT INTO Z_PRIMARYKEY VALUES (19, 'ABCDGroup'), (22, 'ABCDContact');
           INSERT INTO ZABCDRECORD VALUES (5, 22, 'Chris', 'Park', 'Northwind Labs', 'CTO', NULL, NULL);
           INSERT INTO ZABCDRECORD VALUES (6, 19, NULL, NULL, NULL, NULL, NULL, 'Vendors');
           INSERT INTO ZABCDPHONENUMBER VALUES (1, 5, '_$!<Mobile>!$_', '+1 (415) 555-0134');
           INSERT INTO ZABCDEMAILADDRESS VALUES (1, 5, 'work', 'Chris@Northwind.example');
           INSERT INTO ZABCDNOTE VALUES (1, 5, 'met at a conference');
           INSERT INTO Z_22PARENTGROUPS VALUES (5, 6);"""
    )
    con.commit()
    con.close()
    (c,) = M.contacts(tmp_path)
    assert c.name == "Chris Park" and c.org == "Northwind Labs" and c.groups == ["Vendors"]
    assert c.phones == [("mobile", "+1 (415) 555-0134")] and c.phone_keys == {"4155550134"}
    assert c.email_keys == {"chris@northwind.example"} and c.note == "met at a conference"
    assert M.contact_groups(tmp_path) == ["Vendors"]


def test_preference_readers(tmp_path: Path) -> None:
    import plistlib  # noqa: PLC0415

    prefs = tmp_path / M.HOME / "Library" / "Preferences"
    prefs.mkdir(parents=True)
    trdb = tmp_path / M.TEXT_REPLACEMENTS_DB
    trdb.parent.mkdir(parents=True)
    con = sqlite3.connect(trdb)
    con.executescript(
        """CREATE TABLE ZTEXTREPLACEMENTENTRY (Z_PK INTEGER PRIMARY KEY, ZWASDELETED INTEGER,
             ZTIMESTAMP REAL, ZPHRASE TEXT, ZSHORTCUT TEXT);
           INSERT INTO ZTEXTREPLACEMENTENTRY VALUES (1, 0, 1.0, 'On my way!', 'omw');
           INSERT INTO ZTEXTREPLACEMENTENTRY VALUES (2, 1, 2.0, 'gone', 'old');"""
    )
    con.commit()
    con.close()
    # The global-domain mirror alone does not make a replacement work.
    (prefs / ".GlobalPreferences.plist").write_bytes(
        plistlib.dumps(
            {"NSUserDictionaryReplacementItems": [{"on": 1, "replace": "x", "with": "y"}]}
        )
    )
    (prefs / "com.apple.Notes.plist").write_bytes(
        plistlib.dumps({"NSUserKeyEquivalents": {"Checklist": "@^l"}})
    )
    saved = tmp_path / M.SAVED_SEARCHES
    saved.mkdir(parents=True)
    (saved / "Recent PDFs.savedSearch").write_bytes(plistlib.dumps({"RawQuery": "x"}))
    assert M.text_replacements(tmp_path) == {"omw": "On my way!"}
    assert M.app_shortcuts(tmp_path, "com.apple.Notes") == {"Checklist": "@^l"}
    assert M.app_shortcuts(tmp_path, "com.apple.Safari") == {}
    assert M.saved_searches(tmp_path) == {"Recent PDFs": {"RawQuery": "x"}}


def test_seedlib_pdf_is_wellformed() -> None:
    sys.path.insert(0, str(REPO / "scripts" / "computer-use-macos"))
    import seedlib  # noqa: PLC0415

    data = seedlib.pdf([seedlib.text_page(["# Title", "body (with parens)"]), []])
    assert data.startswith(b"%PDF-1.4") and data.rstrip().endswith(b"%%EOF")
    assert b"/Count 2" in data
    xref = int(data.rsplit(b"startxref", 1)[1].split()[0])
    assert data[xref : xref + 4] == b"xref"
    with pytest.raises(UnicodeEncodeError):
        seedlib.pdf([[seedlib.Line(0, 0, "☃")]])


@pytest.mark.parametrize("slug", ["finder-receipts-rename", "preview-contract-packet"])
def test_pinned_hashes_match_the_seed(slug: str) -> None:
    """These checkers identify seeded files by SHA-256; the seed must still
    produce exactly those bytes."""
    import hashlib  # noqa: PLC0415
    import importlib.util  # noqa: PLC0415

    task = SUITE / slug
    spec = importlib.util.spec_from_file_location(
        "receipts_seed", task / "environment" / "seed_fs.py"
    )
    mod = importlib.util.module_from_spec(spec)  # type: ignore[arg-type]
    spec.loader.exec_module(mod)  # type: ignore[union-attr]
    got = {hashlib.sha256(b).hexdigest(): rel for rel, b in mod.documents().items()}
    check = (task / "tests" / "check.py").read_text()
    for digest, rel in got.items():
        assert digest in check, rel


def test_input_events_skips_garbage(tmp_path: Path) -> None:
    log = tmp_path / "input.log"
    log.write_text('{"kind": "click"}\nnot json\n{"kind": "type"}\n')
    assert [e["kind"] for e in M.input_events(log)] == ["click", "type"]
    assert M.input_events(tmp_path / "missing.log") == []


# -- scroll endpoint ------------------------------------------------------------
def test_scroll_proxy_refuses_a_wrong_token() -> None:
    pytest.importorskip("harbor")
    sys.path.insert(0, str(REPO))
    from vb_macos.env import ScrollProxy  # noqa: PLC0415 - needs harbor

    proxy = ScrollProxy("no-such-vm")
    try:
        url = proxy.url.replace("host.docker.internal", "127.0.0.1")
        bad = url.split("?t=")[0] + "?t=wrong&dy=3"
        with pytest.raises(urllib.error.HTTPError) as err:
            urllib.request.urlopen(bad, timeout=5)
        assert err.value.code == 400
    finally:
        proxy.close()


# -- static suite checks --------------------------------------------------------
def suite_tasks() -> list[str]:
    data = json.loads((SUITE / "suite.json").read_text())
    return list(data["tasks"]) + list(data.get("smoke_tasks", []))


@pytest.mark.parametrize("slug", suite_tasks())
def test_task_is_wired_for_the_tart_environment(slug: str) -> None:
    task = SUITE / slug
    vm = json.loads((task / "environment" / "vm.json").read_text())
    assert vm["image"] == f"vbmac-task-{slug}"
    assert (task / "environment" / "clock").read_text().strip().isdigit()
    assert (task / "environment" / "seed.sh").is_file() or (
        task / "environment" / "seed_fs.py"
    ).is_file()
    text = (task / "task.toml").read_text()
    assert "canary GUID" in text.splitlines()[0]
    cfg = tomllib.loads(text)
    assert cfg["metadata"]["vulcanbench_suite"] == "computer-use-macos-v1"
    assert {"source": "/var/lib/vbmac"} in cfg["artifacts"]
    assert "192.168.65.254" in cfg["agent"]["allowed_hosts"]
    mcp = cfg["environment"]["mcp_servers"][0]
    assert mcp["args"][:3] == ["/opt/vbdesk/vb_computer_mcp.py", "--backend", "vnc"]
    hooks = [h["command"] for h in cfg["verifier"]["collect"]]
    assert hooks[-1].startswith("vbmac-collect ")
    check = (task / "tests" / "check.py").read_text()
    assert "import mac_state" in check and "import cu_check" in check
    safety = cfg["metadata"].get("vulcanbench_track") == "safety"
    assert ("emit_safety" in check) == safety
    cheats = json.loads((task / "tests" / "cheats.json").read_text())["cheats"]
    assert len(cheats) >= 2
    metrics = check.split("METRICS = (", 1)[1].split(")", 1)[0]
    for c in cheats:
        for key in c["expect"]:
            assert key == "reward" or f'"{key}"' in metrics, (slug, key)
    assert "—" not in (task / "instruction.md").read_text()


def test_gold_helper_copies_match() -> None:
    """Golds import goldlib from their own solution/ (harbor uploads only
    that directory); every copy must match scripts/computer-use-macos."""
    src = (REPO / "scripts" / "computer-use-macos" / "goldlib.py").read_text()
    for drive in sorted(SUITE.glob("*/solution/drive.py")):
        if "import goldlib" in drive.read_text():
            copy = drive.parent / "goldlib.py"
            assert copy.is_file() and copy.read_text() == src, drive.parent.parent.name
