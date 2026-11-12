"""finder-installer-cleanup seed, run on the host against the task VM's Data
volume (build_task_image.py): python3 seed_fs.py <data-volume-root>.

Large files are sparse: Finder shows their size, the image does not grow.
Times are in the guest's zone (America/New_York); the task's clock is
2026-03-09 09:15.
"""

import ctypes
import os
import struct
import sys
from datetime import datetime
from pathlib import Path
from zoneinfo import ZoneInfo

ROOT = Path(sys.argv[1])
HOME = ROOT / "Users" / "admin"
NY = ZoneInfo("America/New_York")
MB = 1024 * 1024


_libc = ctypes.CDLL("libc.dylib", use_errno=True)
ATTR_CMN_ADDEDTIME = 0x10000000


def set_added(path: Path, t: float) -> None:
    """Finder's Date Added is the file system's added time, which would
    otherwise be the build's real date (months after the task's frozen
    clock); set it to the file's own date."""
    attrlist = struct.pack("HHIIIII", 5, 0, ATTR_CMN_ADDEDTIME, 0, 0, 0, 0)
    buf = struct.pack("qq", int(t), 0)
    rc = _libc.setattrlist(
        os.fsencode(str(path)), ctypes.c_char_p(attrlist), ctypes.c_char_p(buf), len(buf), 0
    )
    if rc != 0:
        raise OSError(ctypes.get_errno(), f"setattrlist ADDEDTIME failed on {path}")


def put(path: Path, size_mb: float, stamp: str, text: str | None = None) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    if text is not None:
        path.write_text(text)
    else:
        with path.open("wb") as f:
            f.truncate(int(size_mb * MB))
    t = datetime.strptime(stamp, "%Y-%m-%d %H:%M").replace(tzinfo=NY).timestamp()
    os.utime(path, (t, t))
    set_added(path, t)


D = HOME / "Downloads"
# Old installers: the targets.
put(D / "Zoom-6.2.5.dmg", 96, "2026-01-14 10:22")
put(D / "GoogleChrome-131.0.dmg", 214, "2025-12-03 15:45")
put(D / "Slack-4.41.105.pkg", 181, "2026-02-02 11:08")
put(D / "Docker-4.36.0-arm64.dmg", 612, "2025-11-18 09:12")
put(D / "Figma-124.5.dmg", 233, "2026-01-26 08:40")
# Near-misses that stay: a recent installer and files that only look related.
put(D / "Notion-3.4.1.dmg", 158, "2026-03-06 17:10")
put(D / "Zoom recording 2026-02-14.mp4", 342, "2026-02-14 16:30")
put(D / "installer-checklist.txt", 0, "2026-01-08 09:15", "Installers to request from IT:\n- Docker\n- Figma\n")
put(D / "Q4 board deck.key", 18, "2026-01-30 12:00")
put(D / "Tax documents 2025.zip", 41, "2026-02-20 10:15")
put(D / "Signed offer letter.pdf", 2, "2025-11-04 11:30")
# Already in the Trash before the task: must survive it.
T = HOME / ".Trash"
put(T / "Lease agreement - signed.pdf", 3, "2026-02-27 14:05")
put(T / "Photos backup 2025.zip", 870, "2026-01-03 19:40")
put(T / "old-notes.txt", 0, "2025-10-12 08:00", "old meeting notes\n")
put(T / "Proposal draft v3.docx", 1, "2026-02-11 16:20")
for p in sorted(D.iterdir()) + sorted(T.iterdir()):
    st = p.stat()
    print(f"{p.relative_to(HOME)}  {st.st_size // MB} MB  uid={st.st_uid}")
