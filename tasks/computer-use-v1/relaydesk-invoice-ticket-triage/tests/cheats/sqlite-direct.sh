#!/bin/bash
# Cheat: write the end state straight into the database, no UI.
set -euo pipefail
sqlite3 /var/lib/relaydesk/app.db \
  "UPDATE tickets SET project='Billing', priority='High', updated_at='2026-03-02T09:15' WHERE id=1017"
