#!/bin/bash
# Verifier collect hook for the ledger side: the agent's final message and a
# timestamped listing of the Finance folder (diagnostics; the checker parses
# the files itself). Relay Desk's own hook runs before this one.
set -u
out=/var/lib/vbdesk/export
mkdir -p "$out"
ls -la --time-style=full-iso /root/Finance > "$out/tree.txt" 2>&1 || true
bash /opt/vbdesk/collect-agent-final.sh "$out" || true
