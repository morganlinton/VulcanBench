#!/bin/bash
# Oracle solution: drive Thunar, Evince and LibreOffice Calc with the keyboard
# through xdotool. `harbor run -a oracle` starts no display, so bring up the
# same stack the entrypoint uses (a no-op when it is already up) and the two
# apps if needed.
set -euo pipefail
export DISPLAY=:1
export GSETTINGS_BACKEND=keyfile
VB_NO_BROWSER=1 bash /opt/vbdesk/ensure-display.sh
if ! pgrep -f soffice.bin >/dev/null; then
  setsid nohup soffice --calc --norestore --nologo /root/Finance/Payables.ods >/dev/null 2>&1 </dev/null &
  bash /opt/vbdesk/raise-window.sh 'LibreOffice Calc' 90
fi
pgrep -x thunar >/dev/null || { setsid nohup thunar /root/Invoices/incoming >/dev/null 2>&1 </dev/null & }
bash /opt/vbdesk/raise-window.sh 'incoming|Thunar' 30
python3 "$(dirname "$0")/drive.py"
