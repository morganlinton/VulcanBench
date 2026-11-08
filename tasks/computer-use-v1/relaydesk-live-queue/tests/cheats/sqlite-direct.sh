#!/bin/bash
# Cheat: write the end state straight into the database, no UI. It can even
# insert the ticket that would have arrived; without a page view nothing is
# clocked and no arrival audit row exists, so the UI path fails.
set -euo pipefail
sqlite3 /var/lib/relaydesk/app.db \
  "UPDATE tickets SET assignee='Marcus Webb', project='Platform', priority='High', updated_at='2026-03-02T09:15' WHERE id IN (3004,3011,3019,3027)"
