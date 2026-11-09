#!/bin/bash
# Container entrypoint for the invoice entry task. Brings up the display with
# no browser, opens the ledger in LibreOffice Calc, then Thunar on the
# incoming folder on top of it, and keeps Thunar above the Chromium window
# harbor's computer-1 agent opens unconditionally. Evince is launched by
# Thunar (application/pdf in the image's mimeapps.list) and inherits
# GSETTINGS_BACKEND=keyfile from here, so it reads the image's keyfile (no
# sidebar, fit to width) instead of in-memory defaults. Harbor replaces the
# image CMD with `sleep infinity` but keeps the ENTRYPOINT.
set -euo pipefail
export DISPLAY=:1
export GSETTINGS_BACKEND=keyfile
mkdir -p /var/lib/vbdesk/export /var/log/invoices
# VB_RAISE_WINDOW (image ENV, for the MCP server's later display call) is
# unset here: Thunar does not exist yet, so there is nothing to wait for.
env -u VB_RAISE_WINDOW VB_NO_BROWSER=1 bash /opt/vbdesk/ensure-display.sh >> /var/log/invoices/display.log 2>&1
setsid nohup soffice --calc --norestore --nologo /root/Finance/Payables.ods \
  >> /var/log/invoices/soffice.log 2>&1 < /dev/null &
echo $! > /var/lib/vbdesk/app.pid
bash /opt/vbdesk/raise-window.sh 'LibreOffice Calc' 60 >> /var/log/invoices/display.log 2>&1 || true
setsid nohup thunar /root/Invoices/incoming >> /var/log/invoices/thunar.log 2>&1 < /dev/null &
bash /opt/vbdesk/raise-window.sh 'incoming|Thunar' 30 >> /var/log/invoices/display.log 2>&1 || true
# computer-1 starts Chromium later; raise Thunar again once that window exists.
setsid nohup bash /opt/vbdesk/raise-window.sh 'incoming|Thunar' 900 --after-chromium \
  >> /var/log/invoices/display.log 2>&1 < /dev/null &
exec "$@"
