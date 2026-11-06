#!/bin/bash
# Bring up the display stack harbor's computer-1 agent would start in the task
# container, for runs where no agent starts it: `harbor run -a oracle` (the gold
# script), `-a nop`, and the validator's cheat runs. Idempotent: every step is
# skipped when it is already up, using the same checks computer-1 uses
# (harbor/agents/computer_1/runtime.py), so calling it after computer-1 is a
# no-op. Paths and flags mirror that runtime on purpose.
set -euo pipefail

W="${VB_DESKTOP_WIDTH:-1024}"
H="${VB_DESKTOP_HEIGHT:-900}"
RT=/tmp/computer_1_runtime
PROFILE="$RT/profile"
export DISPLAY=:1

mkdir -p "$RT" /tmp/computer_1-screenshots "$PROFILE"

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

cdp_up() { curl -fsS -o /dev/null --max-time 3 http://127.0.0.1:9222/json/version 2>/dev/null; }

if ! cdp_up; then
  setsid nohup /usr/bin/chromium \
    --ignore-certificate-errors --disable-dev-shm-usage --no-sandbox --disable-gpu \
    --display=:1 "--user-data-dir=$PROFILE" --window-position=0,0 "--window-size=${W},${H}" \
    --no-first-run --no-default-browser-check --disable-default-apps --disable-dev-tools \
    --disable-extensions \
    --disable-features=IsolateOrigins,site-per-process,AutomationControlled,HttpsUpgrades \
    --disable-infobars --disable-blink-features=AutomationControlled \
    --js-flags=--max-old-space-size=4096 --renderer-process-limit=4 --test-type --lang=en-US \
    --remote-debugging-port=9222 about:blank >>"$RT/chromium.log" 2>&1 </dev/null &
  for _ in $(seq 1 240); do
    cdp_up && break
    sleep 0.5
  done
  cdp_up || { echo "ensure-display: Chromium did not become ready" >&2; tail -20 "$RT/chromium.log" >&2 || true; exit 1; }
fi

# Pin and maximize the browser window so it covers the framebuffer, as
# computer-1's _position_computer_window does. Best effort: the window may take
# a moment to map.
for _ in $(seq 1 40); do
  wid="$(wmctrl -l 2>/dev/null | grep -Ei 'chromium|chrome' | head -1 | awk '{print $1}')"
  [ -n "${wid:-}" ] && break
  sleep 0.25
done
if [ -n "${wid:-}" ]; then
  wmctrl -i -r "$wid" -e "0,0,0,${W},${H}" || true
  wmctrl -i -r "$wid" -b add,maximized_vert,maximized_horz || true
fi
echo "ensure-display: ready (display :1 ${W}x${H}, cdp 127.0.0.1:9222)"
