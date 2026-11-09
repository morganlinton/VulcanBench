#!/bin/bash
# Oracle solution: drive the real UI. `harbor run -a oracle` starts no display,
# so bring up the same stack computer-1 would (no-op if it is already up) with
# the tracker as the start page, then operate both apps through Chromium's
# CDP port with Playwright.
set -euo pipefail
VB_START_URL=http://127.0.0.1:8000/ bash /opt/vbdesk/ensure-display.sh
python3 "$(dirname "$0")/drive.py"
