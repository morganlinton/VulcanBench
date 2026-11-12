#!/bin/bash
# Oracle solution: Finder, through the same VNC backend the agent's tools use.
set -euo pipefail
python3 "$(dirname "$0")/drive.py"
