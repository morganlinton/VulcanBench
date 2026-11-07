#!/bin/bash
# Verifier collect hook for Postbox: the live process's audit mirror, its
# start time relative to PID 1, and the agent's final message (see the Relay
# Desk hook for why each matters).
set -u
DB="${POSTBOX_DB:-/var/lib/postbox/app.db}"
PORT="${POSTBOX_PORT:-8010}"
out="$(dirname "$DB")/export"
mkdir -p "$out"
if ! curl -fsS --max-time 10 "http://127.0.0.1:${PORT}/_internal/export" > "$out/audit.json"; then
  echo '{"error": "export unreachable"}' > "$out/audit.json"
fi
pid="$(pgrep -f "postbox/app.py --serve $DB" | head -1 || true)"
clk="$(getconf CLK_TCK)"
if [ -n "$pid" ] && [ -r "/proc/$pid/stat" ]; then
  app_ticks="$(awk '{print $22}' "/proc/$pid/stat")"
else
  app_ticks=null
fi
pid1_ticks="$(awk '{print $22}' /proc/1/stat)"
printf '{"app_pid": %s, "app_start_ticks": %s, "pid1_start_ticks": %s, "clk_tck": %s}\n' \
  "${pid:-null}" "$app_ticks" "$pid1_ticks" "$clk" > "$out/process.json"
bash /opt/vbdesk/collect-agent-final.sh "$out" || true
