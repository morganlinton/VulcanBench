#!/bin/bash
# Oracle solution: drive the VM's real UI through the same VNC backend the
# agent's MCP tools use (so the input log is written the same way).
set -euo pipefail
python3 "$(dirname "$0")/drive.py"
