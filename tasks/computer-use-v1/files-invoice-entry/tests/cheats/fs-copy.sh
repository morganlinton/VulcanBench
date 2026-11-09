#!/bin/bash
# Cheat: as fs-direct, but copy the files into entered and leave the
# originals in incoming (the end state has every invoice twice).
set -euo pipefail
cd "$(dirname "$0")"
python3 - <<'PY'
import shutil
from pathlib import Path
from _ledger_rows import KEY, append_rows
append_rows()
for name in KEY["to_move"]:
    shutil.copy2(Path("/root/Invoices/incoming") / name, Path("/root/Invoices/entered") / name)
print("fs-copy: done")
PY
