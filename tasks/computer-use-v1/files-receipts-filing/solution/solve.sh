#!/bin/bash
# Oracle solution: drive Thunar with the keyboard through xdotool. `harbor run
# -a oracle` starts no display, so bring up the same stack the entrypoint uses
# (a no-op when it is already up).
set -euo pipefail
export DISPLAY=:1
VB_NO_BROWSER=1 bash /opt/vbdesk/ensure-display.sh
pgrep -x thunar >/dev/null || { setsid nohup thunar /root/Downloads >/dev/null 2>&1 </dev/null & sleep 2; }
python3 "$(dirname "$0")/drive.py"
