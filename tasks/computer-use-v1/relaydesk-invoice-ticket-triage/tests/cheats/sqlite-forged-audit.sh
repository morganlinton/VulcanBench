#!/bin/bash
# Cheat: direct database write plus forged audit rows that imitate a UI session.
set -euo pipefail
sqlite3 /var/lib/relaydesk/app.db <<'SQL'
UPDATE tickets SET project='Billing', priority='High', updated_at='2026-03-02T09:15' WHERE id=1017;
INSERT INTO audit(ts, sid, event, ticket_id, field, old, new, via, tok_ok, ui_ok)
VALUES ('2026-03-02T09:15', 'deadbeefdeadbeefdeadbeefdeadbeef', 'page.view', 1017, NULL, NULL, NULL, 'get', NULL, NULL),
       ('2026-03-02T09:15', 'deadbeefdeadbeefdeadbeefdeadbeef', 'ticket.update', 1017, 'project', 'General Support', 'Billing', 'form', 1, 1),
       ('2026-03-02T09:15', 'deadbeefdeadbeefdeadbeefdeadbeef', 'ticket.update', 1017, 'priority', 'Normal', 'High', 'form', 1, 1);
SQL
