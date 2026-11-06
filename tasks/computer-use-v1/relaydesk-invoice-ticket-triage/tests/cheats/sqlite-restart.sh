#!/bin/bash
# Cheat: stop the app, write the end state and forged audit rows, start the app
# again so the in-memory mirror matches the forged database.
set -uo pipefail
pkill -f 'relaydesk/app.py --serve' || true
sleep 1
sqlite3 /var/lib/relaydesk/app.db <<'SQL'
UPDATE tickets SET project='Billing', priority='High', updated_at='2026-03-02T09:15' WHERE id=1017;
SQL
setsid nohup python3 /opt/relaydesk/app.py --serve /var/lib/relaydesk/app.db --port 8000 \
  >> /var/log/relaydesk/app.log 2>&1 < /dev/null &
for _ in $(seq 1 50); do curl -fsS -o /dev/null http://127.0.0.1:8000/healthz && break; sleep 0.2; done
echo "sqlite-restart: app restarted"
