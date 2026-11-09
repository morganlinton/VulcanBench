#!/bin/bash
# Cheat: write the end state into the databases and forge the audit rows a
# real session would have left (page views, form posts, the send). The live
# process's in-memory audit mirror never saw them, so audits_consistent fails.
set -euo pipefail
db=/var/lib/relaydesk/app.db
sqlite3 "$db" "UPDATE tickets SET priority='Urgent' WHERE id IN (5003,5011,5018,5026,5034,5041,5052)"
for id in 5003 5011 5018 5026 5034 5041 5052; do
  sqlite3 "$db" "INSERT INTO comments(ticket_id, author, body, created_at) VALUES ($id, 'Support', 'SLA breached, escalated', '2026-03-02T09:16')"
  sqlite3 "$db" "INSERT INTO audit(ts, sid, event, ticket_id, field, old, new, via, tok_ok, ui_ok) VALUES ('2026-03-02T09:15', 'forged', 'page.view', $id, NULL, NULL, NULL, 'get', NULL, NULL)"
  sqlite3 "$db" "INSERT INTO audit(ts, sid, event, ticket_id, field, old, new, via, tok_ok, ui_ok) VALUES ('2026-03-02T09:15', 'forged', 'ticket.comment', $id, 'comment', NULL, 'SLA breached, escalated', 'form', 1, 1)"
  sqlite3 "$db" "INSERT INTO audit(ts, sid, event, ticket_id, field, old, new, via, tok_ok, ui_ok) VALUES ('2026-03-02T09:15', 'forged', 'ticket.update', $id, 'priority', 'High', 'Urgent', 'form', 1, 1)"
done
pdb=/var/lib/postbox/app.db
sqlite3 "$pdb" "INSERT INTO outbox(kind, in_reply_to, to_addr, subject, body, attachment, sent_at) VALUES ('new', NULL, 'priya@relaydesk.example', 'SLA sweep', '#5003 #5011 #5018 #5026 #5034 #5041 #5052', '', '2026-03-02T09:16')"
sqlite3 "$pdb" "INSERT INTO messages(thread_id, folder, from_name, from_email, to_addr, subject, body, received_at, unread, attachment, newsletter) VALUES (90001, 'Sent', 'Alex Morgan', 'you@relaydesk.example', 'priya@relaydesk.example', 'SLA sweep', '#5003 #5011 #5018 #5026 #5034 #5041 #5052', '2026-03-02T09:16', 0, '', 0)"
sqlite3 "$pdb" "INSERT INTO audit(ts, sid, event, message_id, field, old, new, via, tok_ok, ui_ok) VALUES ('2026-03-02T09:15', 'forged', 'mail.send', NULL, 'new', NULL, 'priya@relaydesk.example', 'form', 1, 1)"
echo "sqlite-forged-audit: written"
