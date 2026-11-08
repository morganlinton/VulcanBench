#!/bin/bash
# Relay Desk container entrypoint. Harbor replaces the image CMD with
# `sleep infinity` but keeps the ENTRYPOINT, so the app is started here,
# detached, and then the requested command runs as PID 1's child. Same
# behaviour under plain `docker run`, `harbor run -a oracle` and `-a nop`.
# Entrypoints chain: a two-app task lists both as ENTRYPOINT and each one
# execs the rest ("$@").
#   RELAYDESK_DB    database path      (default /var/lib/relaydesk/app.db)
#   RELAYDESK_PORT  listen port        (default 8000)
#   RELAYDESK_SEED  seed for --init    (default /opt/relaydesk/seed.json)
set -euo pipefail
DB="${RELAYDESK_DB:-/var/lib/relaydesk/app.db}"
PORT="${RELAYDESK_PORT:-8000}"
SEED="${RELAYDESK_SEED:-/opt/relaydesk/seed.json}"
DATA="$(dirname "$DB")"
if [ ! -f "$DB" ]; then
  python3 /opt/relaydesk/app.py --init "$DB" --seed "$SEED"
fi
mkdir -p /var/log/relaydesk "$DATA/export"
setsid nohup python3 /opt/relaydesk/app.py --serve "$DB" --port "$PORT" \
  >> /var/log/relaydesk/app.log 2>&1 < /dev/null &
# Record which app process this container booted with, so the verifier can
# tell a restarted app (fresh audit mirror, fresh token secret) from this one.
for _ in $(seq 1 100); do
  curl -fsS -o /dev/null "http://127.0.0.1:${PORT}/healthz" 2>/dev/null && break
  sleep 0.1
done
curl -fsS "http://127.0.0.1:${PORT}/_internal/export" 2>/dev/null \
  | python3 -c 'import json, sys; d = json.load(sys.stdin); json.dump({"boot_id": d["boot_id"], "started_at": d["started_at"]}, sys.stdout)' \
  > "$DATA/boot.json" || true
exec "$@"
