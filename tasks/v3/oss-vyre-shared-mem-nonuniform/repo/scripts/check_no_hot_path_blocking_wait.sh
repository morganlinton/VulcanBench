#!/usr/bin/env bash
# P1 inventory #106  -  block-on / busy-waits / polled blocking maintenance on throughput paths.
#
# Scanned patterns: `wgpu::Maintain::Wait`, `pollster::block_on`, `std::thread::sleep`,
# `std::thread::yield_now`, `thread::park`, `park_timeout` (extend as needed).
#
# Default mode: HIGHWATER ratchet on line matches in vyre-driver-wgpu production sources
# (tests excluded). The current tree still blocks in several places (poll + error scopes);
# the ratchet prevents new sites without a deliberate baseline bump.
#
# `--strict`: fail if any match lies outside STRICT_ALLOW_PREFIX.
# Strict mode allows one-shot device initialization waits plus the single
# shared adaptive backoff helper; raw hot-path waits must not be scattered.

set -euo pipefail

STRICT=false
[[ "${1:-}" == "--strict" ]] && STRICT=true

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

RGOPTS=( --no-heading --line-number --glob '*.rs' --glob '!**/tests/**' --glob '!**/benches/**' )

SCAN_ROOT="vyre-driver-wgpu/src"

# One alternation per line (no multiline).
PATTERN='Maintain::Wait|pollster::block_on|std::thread::sleep\(|std::thread::yield_now\(\)|thread::park\(\)|park_timeout'

hits="$(
  rg "${RGOPTS[@]}" -e "$PATTERN" "$SCAN_ROOT" 2>/dev/null \
    | rg -v '^vyre-driver-wgpu/src/wait_backoff\.rs:' || true
)"

if [[ -z "$hits" ]]; then
  echo "blocking-wait scan: 0 matches (HIGHWATER N/A)."
  exit 0
fi

count=$(printf '%s\n' "$hits" | wc -l | tr -d ' ')
# Baseline tightened 2026-04-28 after routing validation through a
# nonblocking error-scope poll and changing flush to an explicit submitted
# fence. The remaining sites are one-shot device acquisition waits.
HIGHWATER=2

echo "blocking-wait scan ($PATTERN): $count occurrences (HIGHWATER=$HIGHWATER, strict=$STRICT)."

if [[ "$STRICT" == true ]]; then
  # Only these paths qualify as deliberate one-shot init/teardown polling today  -  extend carefully.
  STRICT_ALLOW_PREFIX=(
    '^vyre-driver-wgpu/src/lib\.rs:'
    '^vyre-driver-wgpu/src/backend_impl\.rs:'
    '^vyre-driver-wgpu/src/runtime/device/device\.rs:'
    '^vyre-driver-wgpu/src/wait_backoff\.rs:'
  )
  exit_code=0
  while IFS= read -r line; do
    [[ -z "$line" ]] && continue
    allowed=false
    for pref in "${STRICT_ALLOW_PREFIX[@]}"; do
      if grep -qE "$pref" <<< "$line"; then
        allowed=true
        break
      fi
    done
    if [[ "$allowed" != true ]]; then
      exit_code=1
      echo "Potentially-blocking wait on hot-scope path:" >&2
      echo "  $line" >&2
    fi
  done <<< "$(printf '%s\n' "$hits")"

  if [[ "$exit_code" -ne 0 ]]; then
    echo >&2 ""
    echo "Fix: prefer Poll / fence callbacks / Maintain::Poll patterns; consolidate waits; if truly init-only move under allow-prefix." >&2
    exit "$exit_code"
  fi
  exit 0
fi

if [[ "$count" -gt "$HIGHWATER" ]]; then
  echo "(ratchet) Regression: $count occurrences exceed HIGHWATER=$HIGHWATER." >&2
  printf '%s\n' "$hits" >&2
  echo "Fix: remove blocking waits from hot tiers or bump HIGHWATER with explicit rationale." >&2
  exit 1
fi

exit 0
