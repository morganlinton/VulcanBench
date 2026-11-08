#!/bin/bash
# Oracle solution: read the tracker through the browser (Playwright over CDP)
# and type the report into the editor with xdotool. `harbor run -a oracle`
# starts no agent, so bring up the browser the MCP server would have started
# (a no-op when it is already up), with Calc and the editor if needed.
set -euo pipefail
export DISPLAY=:1
VB_START_URL=http://127.0.0.1:8000/ bash /opt/vbdesk/ensure-display.sh
if ! pgrep -f soffice.bin >/dev/null; then
  setsid nohup soffice --calc --norestore --nologo /root/Finance/Q1-ledger.ods >/dev/null 2>&1 </dev/null &
  bash /opt/vbdesk/raise-window.sh 'LibreOffice Calc' 90
fi
if ! pgrep -x mousepad >/dev/null; then
  setsid nohup mousepad /root/Finance/discrepancies.txt >/dev/null 2>&1 </dev/null &
  bash /opt/vbdesk/raise-window.sh 'discrepancies.txt' 30
fi
python3 "$(dirname "$0")/drive.py"
