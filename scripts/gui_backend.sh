#!/usr/bin/env bash
# Start the local run GUI backend: runner enabled, loopback only.
# usage: scripts/gui_backend.sh [port]   (default 8000)
set -euo pipefail
cd "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
port="${1:-8000}"
if ! .venv/bin/python -c "import uvicorn" 2>/dev/null; then
  echo "uvicorn missing in .venv; installing the backend extra" >&2
  uv pip install -q --python .venv/bin/python "uvicorn[standard]>=0.30.0" 2>/dev/null \
    || .venv/bin/python -m pip install -q "uvicorn[standard]>=0.30.0"
fi
exec env VULCANBENCH_RUNNER=1 .venv/bin/uvicorn backend.app:app --host 127.0.0.1 --port "$port"
