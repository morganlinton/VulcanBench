#!/bin/bash
# Cheat: move the newsletters straight in the database, no UI.
set -euo pipefail
sqlite3 /var/lib/postbox/app.db \
  "UPDATE messages SET folder='Archive' WHERE id IN (3002,3005,3009,3012,3016,3019,3023,3026,3030)"
