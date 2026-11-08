#!/bin/bash
# Container entrypoint for the Calc rollup task. Brings up the display with no
# browser, opens the workbook in LibreOffice Calc, and keeps the Calc window
# on top of the Chromium window harbor's computer-1 agent opens
# unconditionally. Harbor replaces the image CMD with `sleep infinity` but
# keeps the ENTRYPOINT.
set -euo pipefail
export DISPLAY=:1
mkdir -p /var/lib/vbdesk/export /var/log/rollup
# VB_RAISE_WINDOW (image ENV, for the MCP server's later display call) is
# unset here: Calc does not exist yet, so there is nothing to wait for.
env -u VB_RAISE_WINDOW VB_NO_BROWSER=1 bash /opt/vbdesk/ensure-display.sh >> /var/log/rollup/display.log 2>&1
setsid nohup soffice --calc --norestore --nologo /root/Documents/Q3-rollup.ods \
  >> /var/log/rollup/soffice.log 2>&1 < /dev/null &
echo $! > /var/lib/vbdesk/app.pid
bash /opt/vbdesk/raise-window.sh 'LibreOffice Calc' 60 >> /var/log/rollup/display.log 2>&1 || true
# computer-1 starts Chromium later; raise Calc again once that window exists.
setsid nohup bash /opt/vbdesk/raise-window.sh 'LibreOffice Calc' 900 --after-chromium \
  >> /var/log/rollup/display.log 2>&1 < /dev/null &
exec "$@"
