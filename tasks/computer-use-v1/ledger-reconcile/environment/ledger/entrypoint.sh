#!/bin/bash
# Second entrypoint in the chain (after Relay Desk's). Brings up the display
# with no browser, opens the ledger in LibreOffice Calc and the empty report
# in Mousepad, both maximised. Chromium is started later by the MCP server
# (with the tracker as start URL) or by harbor's computer-1 agent, so the
# browser is the window on top when the agent starts; the other two sit
# behind it, reachable with alt+Tab or a click on an exposed edge. Harbor
# replaces the image CMD with `sleep infinity` but keeps the ENTRYPOINT.
set -euo pipefail
export DISPLAY=:1
mkdir -p /var/lib/vbdesk/export /var/log/ledger
VB_NO_BROWSER=1 bash /opt/vbdesk/ensure-display.sh >> /var/log/ledger/display.log 2>&1
setsid nohup soffice --calc --norestore --nologo /root/Finance/Q1-ledger.ods \
  >> /var/log/ledger/soffice.log 2>&1 < /dev/null &
echo $! > /var/lib/vbdesk/app.pid
bash /opt/vbdesk/raise-window.sh 'LibreOffice Calc' 60 >> /var/log/ledger/display.log 2>&1 || true
setsid nohup mousepad /root/Finance/discrepancies.txt >> /var/log/ledger/mousepad.log 2>&1 < /dev/null &
bash /opt/vbdesk/raise-window.sh 'discrepancies.txt' 30 >> /var/log/ledger/display.log 2>&1 || true
exec "$@"
