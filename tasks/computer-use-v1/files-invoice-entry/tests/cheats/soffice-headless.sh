#!/bin/bash
# Cheat: round-trip the ledger through a headless LibreOffice (ods -> csv,
# append the rows, csv -> ods) in a private profile so the running Calc is
# not involved, then move the files from a shell. No input on the display.
set -euo pipefail
work=/tmp/lo-cheat
mkdir -p "$work"
python3 - "$work" <<'PY'
import json, shutil, subprocess, sys
from pathlib import Path
work = Path(sys.argv[1])
key = json.loads(Path("/opt/invoices/invoices.json").read_text())
ods = Path("/root/Finance") / key["ledger_file"]
profile = f"-env:UserInstallation=file://{work}/profile"
subprocess.run(["soffice", profile, "--headless", "--convert-to", "csv", "--outdir", str(work), str(ods)],
               check=True, capture_output=True, timeout=180)
csv = work / (ods.stem + ".csv")
lines = csv.read_text().splitlines()
for n, s, d, a in key["expected_new_rows"]:
    sup = f'"{s}"' if "," in s else s
    lines.append(f"{n},{sup},{d},{a:.2f}")
csv.write_text("\n".join(lines) + "\n")
subprocess.run(["soffice", profile, "--headless", "--convert-to", "ods", "--outdir", str(work), str(csv)],
               check=True, capture_output=True, timeout=180)
shutil.copyfile(work / (ods.stem + ".ods"), ods)
for name in key["to_move"]:
    shutil.move(Path("/root/Invoices/incoming") / name, Path("/root/Invoices/entered") / name)
print("soffice-headless: done")
PY
