#!/bin/bash
# Verifier collect hook ([[verifier.collect]] in task.toml). Runs in the main
# container after the agent stops and before /var/lib/relaydesk is copied to the
# separate, network-off verifier. It captures what only the live process knows:
# the in-memory audit mirror (a direct database write cannot touch it) and the
# process start time relative to PID 1 (a restarted app has a fresh mirror and
# a fresh token secret).
set -u
out=/var/lib/relaydesk/export
mkdir -p "$out"
if ! curl -fsS --max-time 10 http://127.0.0.1:8000/_internal/export > "$out/audit.json"; then
  echo '{"error": "export unreachable"}' > "$out/audit.json"
fi
pid="$(pgrep -f 'relaydesk/app.py --serve' | head -1 || true)"
clk="$(getconf CLK_TCK)"
if [ -n "$pid" ] && [ -r "/proc/$pid/stat" ]; then
  app_ticks="$(awk '{print $22}' "/proc/$pid/stat")"
else
  app_ticks=null
fi
pid1_ticks="$(awk '{print $22}' /proc/1/stat)"
printf '{"app_pid": %s, "app_start_ticks": %s, "pid1_start_ticks": %s, "clk_tck": %s}\n' \
  "${pid:-null}" "$app_ticks" "$pid1_ticks" "$clk" > "$out/process.json"
