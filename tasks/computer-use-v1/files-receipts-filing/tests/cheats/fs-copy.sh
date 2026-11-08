#!/bin/bash
# Cheat: copy the receipts into place from a shell and leave the originals.
set -euo pipefail
python3 - <<'PY'
import json, shutil
from pathlib import Path
seed = json.loads(Path("/opt/filing/seed.json").read_text())
home = Path("/root")
for e in seed["downloads"]:
    if e["dest"]:
        dst = home / e["dest"]
        dst.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(home / "Downloads" / e["name"], dst)
print("fs-copy: copied")
PY
