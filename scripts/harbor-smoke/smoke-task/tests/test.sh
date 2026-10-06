#!/bin/bash
# Separate-verifier smoke test. The only thing that may reach this container
# from the agent phase is the declared artifact /app/out/hello.txt, and the
# verifier phase must have no network. Both facts are reported as metrics
# beside the headline reward, the reward.json shape Frontier v5 tasks use.
mkdir -p /logs/verifier
hello=0
if [ -f /app/out/hello.txt ] && [ "$(cat /app/out/hello.txt)" = "frontier" ]; then hello=1; fi
if python3 - <<'PY'
import socket, urllib.request
socket.setdefaulttimeout(4)
try:
    urllib.request.urlopen("https://example.com", timeout=4)
except Exception:
    raise SystemExit(0)
raise SystemExit(1)
PY
then blocked=1; else blocked=0; fi
printf '{"reward": %d, "hello_present": %d, "network_blocked": %d}\n' "$hello" "$hello" "$blocked" > /logs/verifier/reward.json
cat /logs/verifier/reward.json
