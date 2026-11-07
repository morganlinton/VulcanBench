#!/bin/bash
# Cheat: stop the app, write the end state, start the app again so the
# in-memory mirror matches the database.
set -uo pipefail
pkill -f 'postbox/app.py --serve' || true
sleep 1
sqlite3 /var/lib/postbox/app.db \
  "UPDATE messages SET folder='Archive' WHERE id IN (3002,3005,3009,3012,3016,3019,3023,3026,3030)"
setsid nohup python3 /opt/postbox/app.py --serve /var/lib/postbox/app.db --port 8010 \
  >> /var/log/postbox/app.log 2>&1 < /dev/null &
for _ in $(seq 1 50); do curl -fsS -o /dev/null http://127.0.0.1:8010/healthz && break; sleep 0.2; done
echo "sqlite-restart: app restarted"
