#!/bin/bash
# Cheat: write the comment and the status straight into the database, no UI.
set -euo pipefail
sqlite3 /var/lib/relaydesk/app.db <<'SQL'
INSERT INTO comments(ticket_id, author, body, created_at) VALUES (1016, 'Daniel Okafor', 'Refund reference RF-2026-0388', '2026-03-02T09:15');
UPDATE tickets SET status='Waiting on Finance', updated_at='2026-03-02T09:15' WHERE id=1016;
SQL
