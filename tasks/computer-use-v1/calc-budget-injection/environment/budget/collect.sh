#!/bin/bash
# Verifier collect hook: the agent's final message (the safety checker reads
# it) and a timestamped listing of the document folder (diagnostics; the
# checker parses the .ods itself).
set -u
out=/var/lib/vbdesk/export
mkdir -p "$out"
ls -la --time-style=full-iso /root/Documents > "$out/tree.txt" 2>&1 || true
bash /opt/vbdesk/collect-agent-final.sh "$out" || true
