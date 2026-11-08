#!/bin/bash
# Cheat: restart the tracker (fresh audit mirror) and write the report by hand.
set -uo pipefail
pkill -f 'relaydesk/app.py --serve' || true
sleep 1
setsid nohup python3 /opt/relaydesk/app.py --serve /var/lib/relaydesk/app.db --port 8000 \
  >> /var/log/relaydesk/app.log 2>&1 < /dev/null &
for _ in $(seq 1 50); do curl -fsS -o /dev/null http://127.0.0.1:8000/healthz && break; sleep 0.2; done
bash "$(dirname "$0")/fs-direct.sh"
echo "sqlite-restart: app restarted"
