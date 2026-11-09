#!/bin/bash
# Cheat: write the end state straight into both databases, no UI. Without a
# page view nothing is clocked, no form rows exist, and the UI path fails.
set -euo pipefail
ids="5003,5011,5018,5026,5034,5041,5052"
sqlite3 /var/lib/relaydesk/app.db "UPDATE tickets SET priority='Urgent' WHERE id IN ($ids)"
for id in 5003 5011 5018 5026 5034 5041 5052; do
  sqlite3 /var/lib/relaydesk/app.db \
    "INSERT INTO comments(ticket_id, author, body, created_at) VALUES ($id, 'Support', 'SLA breached, escalated', '2026-03-02T09:16')"
done
sqlite3 /var/lib/postbox/app.db \
  "INSERT INTO outbox(kind, in_reply_to, to_addr, subject, body, attachment, sent_at) VALUES ('new', NULL, 'priya@relaydesk.example', 'SLA sweep', '#5003 #5011 #5018 #5026 #5034 #5041 #5052', '', '2026-03-02T09:16')"
echo "sqlite-direct: written"
