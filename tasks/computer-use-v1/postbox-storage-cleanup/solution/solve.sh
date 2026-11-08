#!/bin/bash
# Oracle solution: drive the real UI. `harbor run -a oracle` starts no display,
# so bring up the same stack computer-1 would (no-op if it is already up), then
# operate the page through Chromium's CDP port with Playwright.
set -euo pipefail
bash /opt/vbdesk/ensure-display.sh
python3 "$(dirname "$0")/drive.py"
