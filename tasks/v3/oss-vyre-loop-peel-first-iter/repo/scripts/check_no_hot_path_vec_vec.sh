#!/usr/bin/env bash
# P1 inventory #107  -  `Vec<Vec<u8>>` output / IO handles on hot dispatch surfaces.
#
# Byte rows should eventually migrate to contiguous buffers, scratch arenas, borrowed slices
# (`&[&[u8]]`), or pools. This gate prevents backsliding: new `Vec<Vec<u8>>` occurrences in
# `vyre-driver-wgpu` prod sources cannot increase without auditing.
#
# Default: HIGHWATER ratchet on substring `Vec<Vec<u8>>`. Baseline aligns with migrations in
# VYRE_PERFORMANCE_ARCHITECTURE_INVENTORY_2026-04-28 (P0 items 3–5).
#
# `--strict`: every match outside STRICT_ALLOW_REGEX must disappear (narrow doc-only exclusions).

set -euo pipefail

STRICT=false
[[ "${1:-}" == "--strict" ]] && STRICT=true

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

RGOPTS=( --no-heading --line-number --glob '*.rs' --glob '!**/tests/**' )

SCAN_ROOT="vyre-driver-wgpu/src"
PATTERN='Vec<Vec<u8>>'

hits="$(rg "${RGOPTS[@]}" -F "$PATTERN" "$SCAN_ROOT" 2>/dev/null || true)"

if [[ -z "$hits" ]]; then
  echo "Vec<Vec<u8>> scan: 0 occurrences."
  exit 0
fi

count=$(printf '%s\n' "$hits" | wc -l | tr -d ' ')
HIGHWATER=35

echo "Vec<Vec<u8>> scan: $count occurrences (HIGHWATER=$HIGHWATER, strict=$STRICT)."

if [[ "$STRICT" == true ]]; then
  # Doc-comments mentioning the type alias are OK; everything else is a release blocker.
  exit_code=0
  while IFS= read -r line; do
    [[ -z "$line" ]] && continue
    path="${line%%:*}"
    rest="${line#*:}"
    content="${rest#*:}"
    if grep -qE '^\s*//' <<< "$content"; then
      continue
    fi
    if grep -qE '^\s*/\*' <<< "$content"; then
      continue
    fi
    exit_code=1
    echo "Vec<Vec<u8>> outside doc-only allowance:" >&2
    echo "  $line" >&2
  done <<< "$(printf '%s\n' "$hits")"

  if [[ "$exit_code" -ne 0 ]]; then
    echo >&2 ""
    echo "Fix: migrate to borrowed row handles, single flat buffer + offsets, or arena-backed rows." >&2
    exit "$exit_code"
  fi
  exit 0
fi

if [[ "$count" -gt "$HIGHWATER" ]]; then
  echo "(ratchet) Regression: $count occurrences exceed HIGHWATER=$HIGHWATER." >&2
  printf '%s\n' "$hits" >&2
  echo "Fix: remove new nested-Vec output paths or bump HIGHWATER with inventory review." >&2
  exit 1
fi

exit 0
