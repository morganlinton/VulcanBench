#!/usr/bin/env bash
# P1 inventory #103  -  no unbounded `HashMap::new()` / `VecDeque::new()` growth footguns
# in dispatcher + tenant runtime wiring.
#
# Maps/deques reachable from compilation or megakernel hot paths SHOULD start with explicit
# capacity budgets, LRU/tier eviction, pooled maps, etc. Bare `new()` is a lint signal.
#
# Default mode ratchets a HIGHWATER on allowed bare `new()` occurrences in scoped trees so
# the situation cannot regress silently. Raise HIGHWATER only with review + comment.
#
# `--strict`: every hit must satisfy the allow-prefix list (narrow reviewed exceptions).

set -euo pipefail

STRICT=false
[[ "${1:-}" == "--strict" ]] && STRICT=true

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

RGOPTS=( --no-heading --line-number --glob '*.rs' --glob '!**/tests/**' --glob '!**/benches/**' --glob '!**/fuzz/**' )

SCAN_PATHS=(
  "vyre-driver-wgpu/src"
  "vyre-runtime/src"
)

PATTERN='\b(HashMap|VecDeque)::new\(\)'
HIGHWATER=2

hits=""
hits="$(rg "${RGOPTS[@]}" -e "$PATTERN" "${SCAN_PATHS[@]}" 2>/dev/null || true)"

if [[ -z "$hits" ]]; then
  count=0
else
  count=$(printf '%s\n' "$hits" | wc -l | tr -d ' ')
fi

echo "unbounded-cache scan ($PATTERN): $count occurrences (HIGHWATER=$HIGHWATER, strict=$STRICT)."

if [[ "$STRICT" == true ]]; then
  exit_code=0
  while IFS= read -r line; do
    [[ -z "$line" ]] && continue
    if ! grep -qE '^(vyre-driver-wgpu/src/buffer/handle\.rs|vyre-runtime/src/uring/io_loop\.rs):' <<< "$line"; then
      echo "Forbidden unbounded associative container construction:" >&2
      echo "$line" >&2
      exit_code=1
    fi
  done <<< "$(printf '%s\n' "$hits")"

  if [[ "$exit_code" -ne 0 ]]; then
    echo >&2 ""
    echo "Fix: replace with bounded construction (capacity / LRU / pool) or move off the hot tier." >&2
    echo "Fix: tighten allow-prefix lists in scripts/check_no_unbounded_cache.sh only after invariant review." >&2
    exit "$exit_code"
  fi
  exit 0
fi

if [[ "$count" -gt "$HIGHWATER" ]]; then
  echo "(ratchet failure) Regression: $count occurrences exceed HIGHWATER=$HIGHWATER." >&2
  echo >&2 ""
  printf '%s\n' "$hits" >&2
  echo "Fix: remove new bare ::new sites or bump HIGHWATER with explicit rationale in this script." >&2
  exit 1
fi

exit 0
