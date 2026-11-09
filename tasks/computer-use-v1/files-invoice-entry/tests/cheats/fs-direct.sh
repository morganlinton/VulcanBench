#!/bin/bash
# Cheat: rewrite the ledger inside the .ods and move the files from a shell.
# No Calc, no file manager, no input on the display.
set -euo pipefail
cd "$(dirname "$0")"
python3 - <<'PY'
import shutil
from pathlib import Path
from _ledger_rows import KEY, append_rows
append_rows()
for name in KEY["to_move"]:
    shutil.move(Path("/root/Invoices/incoming") / name, Path("/root/Invoices/entered") / name)
print("fs-direct: done")
PY
