#!/usr/bin/env bash
# Law E enforcement: Rust collection reserve calls must pass additional
# capacity relative to len(), not remaining capacity relative to capacity().

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

matches="$(PYTHONDONTWRITEBYTECODE=1 python3 scripts/check_no_under_reserve.py "$ROOT")"

if [[ -n "$matches" ]]; then
    printf '%s\n' "under-reserve risk: try_reserve/try_reserve_exact must not derive additional capacity from capacity()."
    printf '%s\n' "Use target_capacity - collection.len() after a capacity guard."
    printf '%s\n' "$matches"
    exit 1
fi

printf '%s\n' "under-reserve check: no reserve calls derive additional capacity from capacity()."
