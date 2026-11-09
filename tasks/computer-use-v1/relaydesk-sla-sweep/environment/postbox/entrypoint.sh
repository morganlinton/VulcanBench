#!/bin/bash
# Postbox container entrypoint; same shape as the Relay Desk one (chainable:
# a two-app task lists both entrypoints and each execs the rest, "$@").
#   POSTBOX_DB    database path    (default /var/lib/postbox/app.db)
#   POSTBOX_PORT  listen port      (default 8010)
#   POSTBOX_SEED  seed for --init  (default /opt/postbox/seed.json)
set -euo pipefail
DB="${POSTBOX_DB:-/var/lib/postbox/app.db}"
PORT="${POSTBOX_PORT:-8010}"
SEED="${POSTBOX_SEED:-/opt/postbox/seed.json}"
DATA="$(dirname "$DB")"
if [ ! -f "$DB" ]; then
  python3 /opt/postbox/app.py --init "$DB" --seed "$SEED"
fi
mkdir -p /var/log/postbox "$DATA/export"
setsid nohup python3 /opt/postbox/app.py --serve "$DB" --port "$PORT" \
  >> /var/log/postbox/app.log 2>&1 < /dev/null &
for _ in $(seq 1 100); do
  curl -fsS -o /dev/null "http://127.0.0.1:${PORT}/healthz" 2>/dev/null && break
  sleep 0.1
done
curl -fsS "http://127.0.0.1:${PORT}/_internal/export" 2>/dev/null \
  | python3 -c 'import json, sys; d = json.load(sys.stdin); json.dump({"boot_id": d["boot_id"], "started_at": d["started_at"]}, sys.stdout)' \
  > "$DATA/boot.json" || true
exec "$@"
