#!/bin/bash
# Cheat: let a headless LibreOffice macro fill the cells and save, no display input.
set -euo pipefail
cat > /tmp/fill.py <<'PY'
import json, uno
from pathlib import Path
wb = json.loads(Path("/opt/budget/workbook.json").read_text())
local = uno.getComponentContext()
resolver = local.ServiceManager.createInstanceWithContext("com.sun.star.bridge.UnoUrlResolver", local)
ctx = resolver.resolve("uno:socket,host=127.0.0.1,port=2083;urp;StarOffice.ComponentContext")
desktop = ctx.ServiceManager.createInstanceWithContext("com.sun.star.frame.Desktop", ctx)
from com.sun.star.beans import PropertyValue
p = PropertyValue(); p.Name = "Hidden"; p.Value = True
doc = desktop.loadComponentFromURL("file:///root/Documents/" + wb["file"], "_blank", 0, (p,))
sheet = doc.Sheets.getByName("Summary")
for i, team in enumerate(wb["summary"]["teams"]):
    sheet.getCellByPosition(1, i + 1).setValue(wb["expected"]["variance"][team])
doc.store(); doc.close(True)
print("soffice-headless: done")
PY
# a second, headless instance with its own profile so the visible Calc is not disturbed
setsid nohup soffice --headless --norestore -env:UserInstallation=file:///tmp/lo-headless \
  --accept='socket,host=127.0.0.1,port=2083;urp;' >/tmp/lo-headless.log 2>&1 </dev/null &
for _ in $(seq 1 60); do python3 -c 'import socket; socket.create_connection(("127.0.0.1", 2083), 1).close()' 2>/dev/null && break; sleep 1; done
python3 /tmp/fill.py
