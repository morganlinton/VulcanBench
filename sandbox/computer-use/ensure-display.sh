#!/bin/bash
# Bring up the display stack harbor's computer-1 agent would start in the task
# container, for runs where no agent starts it: `harbor run -a oracle` (the gold
# script), `-a nop`, the validator's cheat runs, and the MCP computer tools.
# Idempotent: every step is skipped when it is already up, using the same
# checks computer-1 uses (harbor/agents/computer_1/runtime.py), so calling it
# after computer-1 is a no-op. Paths and flags mirror that runtime on purpose.
#
# Environment:
#   VB_DESKTOP_WIDTH / VB_DESKTOP_HEIGHT   framebuffer size (default 1024x900)
#   VB_START_URL      the page Chromium opens when started here (about:blank)
#   VB_NO_BROWSER=1   do not start Chromium (family B native-app tasks; the
#                     window manager and the input recorder still start)
#   VB_RAISE_WINDOW   a window-title pattern (grep -Ei) to raise and maximize
#                     once it exists, e.g. "LibreOffice Calc" or "Thunar"
set -euo pipefail

W="${VB_DESKTOP_WIDTH:-1024}"
H="${VB_DESKTOP_HEIGHT:-900}"
RT=/tmp/computer_1_runtime
PROFILE="$RT/profile"
export DISPLAY=:1

mkdir -p "$RT" /tmp/computer_1-screenshots "$PROFILE" /var/lib/vbdesk

if ! test -S /tmp/.X11-unix/X1; then
  setsid nohup Xvfb :1 -screen 0 "${W}x${H}x24" -fbdir /var/tmp >>"$RT/xvfb.log" 2>&1 </dev/null &
  for _ in $(seq 1 480); do
    test -S /tmp/.X11-unix/X1 && break
    sleep 0.25
  done
  test -S /tmp/.X11-unix/X1 || { echo "ensure-display: X11 display :1 never appeared" >&2; exit 1; }
fi

if ! pgrep -x xfwm4 >/dev/null 2>&1; then
  setsid nohup startxfce4 >>"$RT/xfce4.log" 2>&1 </dev/null &
  sleep 1
fi

# Raw input recorder (UI-path evidence for apps without an audit log). One per
# container; the log is an artifact for tasks that declare /var/lib/vbdesk.
if ! pgrep -f '/opt/vbdesk/xinput_record.py' >/dev/null 2>&1; then
  setsid nohup python3 /opt/vbdesk/xinput_record.py >>"$RT/xinput-record.log" 2>&1 </dev/null &
fi

cdp_up() { curl -fsS -o /dev/null --max-time 3 http://127.0.0.1:9222/json/version 2>/dev/null; }

if [ "${VB_NO_BROWSER:-0}" != "1" ]; then
  if ! cdp_up; then
    setsid nohup /usr/bin/chromium \
      --ignore-certificate-errors --disable-dev-shm-usage --no-sandbox --disable-gpu \
      --display=:1 "--user-data-dir=$PROFILE" --window-position=0,0 "--window-size=${W},${H}" \
      --no-first-run --no-default-browser-check --disable-default-apps --disable-dev-tools \
      --disable-extensions \
      --disable-features=IsolateOrigins,site-per-process,AutomationControlled,HttpsUpgrades \
      --disable-infobars --disable-blink-features=AutomationControlled \
      --js-flags=--max-old-space-size=4096 --renderer-process-limit=4 --test-type --lang=en-US \
      --remote-debugging-port=9222 "${VB_START_URL:-about:blank}" >>"$RT/chromium.log" 2>&1 </dev/null &
    for _ in $(seq 1 240); do
      cdp_up && break
      sleep 0.5
    done
    cdp_up || { echo "ensure-display: Chromium did not become ready" >&2; tail -20 "$RT/chromium.log" >&2 || true; exit 1; }
  fi

  # Pin and maximize the browser window so it covers the framebuffer, as
  # computer-1's _position_computer_window does. Best effort: the window may
  # take a moment to map.
  for _ in $(seq 1 40); do
    wid="$(wmctrl -l 2>/dev/null | grep -Ei 'chromium|chrome' | head -1 | awk '{print $1}')"
    [ -n "${wid:-}" ] && break
    sleep 0.25
  done
  if [ -n "${wid:-}" ]; then
    wmctrl -i -r "$wid" -e "0,0,0,${W},${H}" || true
    wmctrl -i -r "$wid" -b add,maximized_vert,maximized_horz || true
  fi
fi

if [ -n "${VB_RAISE_WINDOW:-}" ]; then
  bash /opt/vbdesk/raise-window.sh "$VB_RAISE_WINDOW" 20 || true
fi

if [ "${VB_NO_BROWSER:-0}" != "1" ]; then
  echo "ensure-display: ready (display :1 ${W}x${H}, cdp 127.0.0.1:9222)"
else
  echo "ensure-display: ready (display :1 ${W}x${H}, no browser)"
fi
