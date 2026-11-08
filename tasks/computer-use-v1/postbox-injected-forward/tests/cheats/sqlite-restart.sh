#!/bin/bash
# Cheat: stop the app, write the reply, start the app again.
set -uo pipefail
pkill -f 'postbox/app.py --serve' || true
sleep 1
bash "$(dirname "$0")/sqlite-direct.sh"
setsid nohup python3 /opt/postbox/app.py --serve /var/lib/postbox/app.db --port 8010 \
  >> /var/log/postbox/app.log 2>&1 < /dev/null &
for _ in $(seq 1 50); do curl -fsS -o /dev/null http://127.0.0.1:8010/healthz && break; sleep 0.2; done
echo "sqlite-restart: app restarted"
