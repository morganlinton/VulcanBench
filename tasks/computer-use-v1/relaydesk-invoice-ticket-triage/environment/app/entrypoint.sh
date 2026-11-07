#!/bin/bash
# Container entrypoint. Harbor replaces the image CMD with `sleep infinity` but
# keeps the ENTRYPOINT, so the app is started here, detached, and then the
# requested command runs as PID 1's child. Same behaviour under plain
# `docker run`, `harbor run -a oracle` and `-a nop`.
set -euo pipefail
DB=/var/lib/relaydesk/app.db
if [ ! -f "$DB" ]; then
  python3 /opt/relaydesk/app.py --init "$DB" --seed /opt/relaydesk/seed.json
fi
mkdir -p /var/log/relaydesk
setsid nohup python3 /opt/relaydesk/app.py --serve "$DB" --port 8000 \
  >> /var/log/relaydesk/app.log 2>&1 < /dev/null &
# Record which app process this container booted with, so the verifier can
# tell a restarted app (fresh audit mirror, fresh token secret) from this one.
for _ in $(seq 1 100); do
  curl -fsS -o /dev/null http://127.0.0.1:8000/healthz 2>/dev/null && break
  sleep 0.1
done
mkdir -p /var/lib/relaydesk/export
curl -fsS http://127.0.0.1:8000/_internal/export 2>/dev/null \
  | python3 -c 'import json, sys; d = json.load(sys.stdin); json.dump({"boot_id": d["boot_id"], "started_at": d["started_at"]}, sys.stdout)' \
  > /var/lib/relaydesk/boot.json || true
exec "$@"
