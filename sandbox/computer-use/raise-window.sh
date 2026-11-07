#!/bin/bash
# Raise and maximize the first window whose title matches a pattern, waiting
# for it to appear. Family B entrypoints use it so the native app sits above
# the Chromium window computer-1 opens unconditionally.
#
#     raise-window.sh <grep -Ei pattern> [timeout_sec=20] [--after-chromium]
#
# With --after-chromium the script first waits (up to the timeout) for a
# Chromium window to exist, then raises the app; without a Chromium window in
# that time it raises the app anyway. Exit 0 when raised, 1 when never seen.
set -u
pat="${1:?pattern}"
timeout="${2:-20}"
mode="${3:-}"
export DISPLAY="${DISPLAY:-:1}"
W="${VB_DESKTOP_WIDTH:-1024}"
H="${VB_DESKTOP_HEIGHT:-900}"
ticks=$(( timeout * 4 ))

find_wid() { wmctrl -l 2>/dev/null | grep -Ei -- "$1" | head -1 | awk '{print $1}'; }

if [ "$mode" = "--after-chromium" ]; then
  for _ in $(seq 1 "$ticks"); do
    [ -n "$(find_wid 'chromium|chrome')" ] && break
    sleep 0.25
  done
  sleep 1
fi

wid=""
for _ in $(seq 1 "$ticks"); do
  wid="$(find_wid "$pat")"
  [ -n "$wid" ] && break
  sleep 0.25
done
[ -n "$wid" ] || { echo "raise-window: no window matching '$pat' after ${timeout}s" >&2; exit 1; }
wmctrl -i -r "$wid" -e "0,0,0,${W},${H}" 2>/dev/null || true
wmctrl -i -r "$wid" -b add,maximized_vert,maximized_horz 2>/dev/null || true
wmctrl -i -a "$wid" 2>/dev/null || true
echo "raise-window: raised $wid ($(wmctrl -l | awk -v w="$wid" '$1==w {$1=$2=$3=""; print substr($0,4)}'))"
