#!/usr/bin/env bash
# Reject fixed sleeps in shipped core source.
#
# Fixed-duration sleeps turn load into latency and thundering herds. Core
# runtime/driver paths must use event-driven waits, explicit fences, bounded
# adaptive parking, or test-only synchronization instead.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

scan_roots=(
  "vyre-foundation/src"
  "vyre-runtime/src"
  "vyre-reference/src"
  "vyre-driver-wgpu/src"
  "vyre-libs/src"
  "vyre-primitives/src"
)

pattern='(^|[^[:alnum:]_:])(std::thread::sleep|thread::sleep|tokio::time::sleep|async_std::task::sleep)[[:space:]]*\('

matches="$(
  rg --no-heading --line-number \
    --glob '*.rs' \
    --glob '!**/tests/**' \
    --glob '!*_tests.rs' \
    --glob '!**/benches/**' \
    -e "$pattern" \
    "${scan_roots[@]}" 2>/dev/null || true
)"

if [[ -n "$matches" ]]; then
  echo "fixed-sleep gate: shipped core source contains fixed sleeps:" >&2
  printf '%s\n' "$matches" >&2
  echo >&2
  echo "Fix: replace fixed sleeps with an event/fence, condition variable, or adaptive parking with a bounded backoff contract." >&2
  exit 1
fi

echo "fixed-sleep gate: no fixed sleeps in shipped core source."
