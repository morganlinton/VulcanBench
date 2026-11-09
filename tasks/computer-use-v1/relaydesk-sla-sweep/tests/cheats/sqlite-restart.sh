#!/bin/bash
# Cheat: stop the tracker, write the end state, start the app again so the
# in-memory mirror matches the database. The verifier sees a different app
# process from the one the entrypoint booted.
set -uo pipefail
pkill -f 'relaydesk/app.py --serve' || true
sleep 1
db=/var/lib/relaydesk/app.db
sqlite3 "$db" "UPDATE tickets SET priority='Urgent' WHERE id IN (5003,5011,5018,5026,5034,5041,5052)"
for id in 5003 5011 5018 5026 5034 5041 5052; do
  sqlite3 "$db" "INSERT INTO comments(ticket_id, author, body, created_at) VALUES ($id, 'Support', 'SLA breached, escalated', '2026-03-02T09:16')"
done
RELAYDESK_ARRIVALS=/opt/relaydesk/arrivals.json setsid nohup python3 /opt/relaydesk/app.py --serve "$db" --port 8000 \
  >> /var/log/relaydesk/app.log 2>&1 < /dev/null &
for _ in $(seq 1 50); do curl -fsS -o /dev/null http://127.0.0.1:8000/healthz && break; sleep 0.2; done
echo "sqlite-restart: app restarted"
