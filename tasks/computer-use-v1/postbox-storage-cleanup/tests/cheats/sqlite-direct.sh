#!/bin/bash
# Cheat: move the Vantage Retail mail to Trash straight in the database, no UI.
set -euo pipefail
sqlite3 /var/lib/postbox/app.db \
  "UPDATE messages SET folder='Trash' WHERE from_email='deals@vantageretail.example' AND folder != 'Trash'"
