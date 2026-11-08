#!/bin/bash
# Oracle solution: drive LibreOffice Calc with the keyboard through xdotool.
# `harbor run -a oracle` starts no display, so bring up the same stack the
# entrypoint uses (a no-op when it is already up) and the document if needed.
set -euo pipefail
export DISPLAY=:1
VB_NO_BROWSER=1 bash /opt/vbdesk/ensure-display.sh
if ! pgrep -f soffice.bin >/dev/null; then
  setsid nohup soffice --calc --norestore --nologo /root/Documents/Q3-rollup.ods >/dev/null 2>&1 </dev/null &
fi
bash /opt/vbdesk/raise-window.sh 'LibreOffice Calc' 90
python3 "$(dirname "$0")/drive.py"
