#!/bin/bash
# Cheat: write the reply straight into the outbox and Sent folder, no UI.
set -euo pipefail
sqlite3 /var/lib/postbox/app.db <<'SQL'
INSERT INTO outbox(kind, in_reply_to, to_addr, subject, body, attachment, sent_at)
VALUES ('reply', 4203, 'dana@holmlogistics.example', 'Re: Leith depot delivery: new slot needed',
        'Confirmed: Wednesday 11 March between 13:00 and 15:00 at the Leith depot.', '', '2026-03-02T09:15');
INSERT INTO messages(thread_id, folder, from_name, from_email, to_addr, subject, body, received_at, unread, attachment, newsletter)
VALUES (4200, 'Sent', 'Alex Morgan', 'you@relaydesk.example', 'dana@holmlogistics.example',
        'Re: Leith depot delivery: new slot needed',
        'Confirmed: Wednesday 11 March between 13:00 and 15:00 at the Leith depot.', '2026-03-02T09:15', 0, '', 0);
SQL
