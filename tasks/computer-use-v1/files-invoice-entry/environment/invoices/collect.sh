#!/bin/bash
# Verifier collect hook: the agent's final message and a listing of the
# invoice and Finance folders with full timestamps (diagnostics; the checker
# reads the files themselves).
set -u
out=/var/lib/vbdesk/export
mkdir -p "$out"
ls -laR --time-style=full-iso /root/Invoices /root/Finance > "$out/tree.txt" 2>&1 || true
bash /opt/vbdesk/collect-agent-final.sh "$out" || true
