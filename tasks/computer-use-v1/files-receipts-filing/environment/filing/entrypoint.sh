#!/bin/bash
# Container entrypoint for the Thunar filing task. Brings up the display with
# no browser, opens Thunar on Downloads, and keeps the Thunar window on top of
# the Chromium window harbor's computer-1 agent opens unconditionally. Harbor
# replaces the image CMD with `sleep infinity` but keeps the ENTRYPOINT.
set -euo pipefail
export DISPLAY=:1
mkdir -p /var/lib/vbdesk/export /var/log/filing
# VB_RAISE_WINDOW (image ENV, for the MCP server's later display call) is
# unset here: Thunar does not exist yet, so there is nothing to wait for.
env -u VB_RAISE_WINDOW VB_NO_BROWSER=1 bash /opt/vbdesk/ensure-display.sh >> /var/log/filing/display.log 2>&1
setsid nohup thunar /root/Downloads >> /var/log/filing/thunar.log 2>&1 < /dev/null &
echo $! > /var/lib/vbdesk/app.pid
bash /opt/vbdesk/raise-window.sh 'Downloads|Thunar' 30 >> /var/log/filing/display.log 2>&1 || true
# computer-1 starts Chromium later; raise Thunar again once that window exists.
setsid nohup bash /opt/vbdesk/raise-window.sh 'Downloads|Thunar' 900 --after-chromium \
  >> /var/log/filing/display.log 2>&1 < /dev/null &
exec "$@"
