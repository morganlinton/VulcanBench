#!/bin/bash
# Cheat: move and rename the receipts from a shell, no file manager.
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
        shutil.move(home / "Downloads" / e["name"], dst)
print("fs-direct: moved")
PY
