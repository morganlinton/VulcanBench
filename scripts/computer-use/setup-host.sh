#!/bin/bash
# Host setup for the ComputerUse v1 suite: harbor with the computer-1 extra
# (vendor SDKs for its native providers) and the computer-use sandbox image.
# Idempotent; run it again after a harbor or image change.
#
#   scripts/computer-use/setup-host.sh
set -euo pipefail
here="$(cd "$(dirname "$0")/../.." && pwd)"
export PATH="$HOME/.local/bin:/opt/homebrew/bin:$PATH"

HARBOR_VERSION="${HARBOR_VERSION:-0.24.0}"

echo "== harbor[computer-1]==$HARBOR_VERSION"
uv tool install "harbor[computer-1]==$HARBOR_VERSION"
"$HOME/.local/share/uv/tools/harbor/bin/python" -c 'import anthropic, openai; print("sdks ok:", anthropic.__version__, openai.__version__)'
harbor --version | grep -q "$HARBOR_VERSION" || { echo "harbor --version is not $HARBOR_VERSION" >&2; exit 1; }
harbor agent schema computer-1 >/dev/null

echo "== vulcanbench/sandbox:cu-base"
make -C "$here" sandbox-image-computer-use
docker run --rm vulcanbench/sandbox:cu-base bash -c \
  'chromium --version && for b in Xvfb startxfce4 xdotool wmctrl import cwebp scrot curl pgrep sqlite3; do command -v "$b" >/dev/null || { echo "missing $b" >&2; exit 1; }; done && python3 -c "import playwright" && echo "image ok"'
echo "== done"
