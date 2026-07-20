#!/usr/bin/env bash
# P1 inventory #104  -  unbounded synchronous external reads (`read_to_end` on arbitrary files)
# must not appear on dispatch-critical paths outside approved cache/asset modules.
#
# Network / disk artifact / tiered caches must expose byte caps, truncation, checksum length
# proofs, etc. Plain `std::fs::File::open` → `read_to_end` pairs are DoS amplifiers on those
# paths if accidentally wired into synchronous dispatch loops.
#
# Implemented as an allow-prefix gate: occurrences are allowed ONLY under explicit cache/disk/io
# modules listed below (documented exclusions). Extend the allowlist sparingly  -  each entry
# should describe its cap/deny policy in-module.
#
# Remaining FYI violations (wire decode corpus / tools) are intentionally out of THIS scan’s
# tree scope (`vyre-driver-wgpu/src` prod); expand SCAN_PATH when wire gates land.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

RGOPTS=( --no-heading --line-number --glob '*.rs' --glob '!**/tests/**' )

# Single-tree focus: driver synchronous surface adjacent to backends.
SCAN_ROOT="vyre-driver-wgpu/src"

# Blocking read-all pattern (extend if new read APIs appear).
PATTERN='read_to_end'

hits="$(rg "${RGOPTS[@]}" -e "$PATTERN" "$SCAN_ROOT" 2>/dev/null || true)"

if [[ -z "$hits" ]]; then
  echo "unbounded-external-read scan: 0 occurrences (PASS)."
  exit 0
fi

# Allow-listed production modules where disk ingestion is deliberate (compile cache, tiers).
ALLOW_PREFIX=(
  '^vyre-driver-wgpu/src/pipeline/disk_cache\.rs:'
  '^vyre-driver-wgpu/src/runtime/cache/disk\.rs:'
)

exit_code=0
while IFS= read -r line; do
  [[ -z "$line" ]] && continue
  allowed=false
  for pref in "${ALLOW_PREFIX[@]}"; do
    if grep -qE "$pref" <<< "$line"; then
      allowed=true
      break
    fi
  done
  if [[ "$allowed" != true ]]; then
    exit_code=1
    echo "Disallowed unbounded synchronous read-all:" >&2
    echo "  $line" >&2
  fi
done <<< "$(printf '%s\n' "$hits")"

if [[ "$exit_code" -ne 0 ]]; then
  echo >&2 ""
  echo "Fix: move IO behind bounded readers (explicit max bytes, chunked read, mmap with cap) or add to ALLOW_PREFIX with rationale." >&2
  exit 1
fi

echo "unbounded-external-read scan: all read_to_end uses are under approved cache modules (PASS)."
exit 0
