#!/usr/bin/env python3
"""Create the task's file tree from seed.json (run at image build).

    python3 seed_fs.py SEED_JSON HOME

Every file gets distinct placeholder content so the checker can follow a
file by its hash wherever it is moved or renamed, and a fixed modification
time so Thunar's Date Modified column is the same on every boot (and is the
only source of a date for the receipt whose name has none).
"""

from __future__ import annotations

import hashlib
import json
import os
import sys
import time
from pathlib import Path


def stamp(path: Path, iso: str) -> None:
    t = time.mktime(time.strptime(iso, "%Y-%m-%dT%H:%M"))
    os.utime(path, (t, t))


def write(path: Path, label: str, iso: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    nonce = hashlib.sha256(label.encode()).hexdigest()[:16]
    if path.suffix == ".zip":
        body = b"PK\x05\x06" + b"\x00" * 18 + f"\n{label}\n{nonce}\n".encode()
    else:
        body = f"%PDF-1.4\n% placeholder receipt document\n% {label}\n% {nonce}\n%%EOF\n".encode()
    path.write_bytes(body)
    stamp(path, iso)


def main() -> None:
    seed = json.loads(Path(sys.argv[1]).read_text())
    home = Path(sys.argv[2])
    for entry in seed["prefiled"]:
        write(home / entry["path"], entry["path"], entry["mtime"])
    for entry in seed["downloads"]:
        write(home / "Downloads" / entry["name"], entry["name"], entry["mtime"])
    naming = home / "Finance" / "NAMING.txt"
    naming.write_text(seed["naming_txt"])
    stamp(naming, "2026-01-05T09:00")
    for d in sorted(home.rglob("*")):
        if d.is_dir() and d.name not in (".config", ".cache", ".local"):
            stamp(d, "2026-03-01T08:00")
    print(f"seeded {home}/Downloads and {home}/Finance")


if __name__ == "__main__":
    main()
