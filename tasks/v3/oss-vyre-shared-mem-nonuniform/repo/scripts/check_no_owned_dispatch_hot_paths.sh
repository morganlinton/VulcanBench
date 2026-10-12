#!/usr/bin/env bash
# P1 inventory #5  -  production callers must prefer borrowed dispatch.
#
# `VyreBackend::dispatch(&[Vec<u8>])` remains as compatibility surface area,
# but hot production/conformance paths should call `dispatch_borrowed` so
# backends with clone-free staging do not get forced through owned row APIs.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

scan_roots=(
    "vyre-libs/src"
    "vyre-runtime/src"
    "conform/vyre-conform-runner/src"
    "conform/vyre-test-harness/src"
)

hits="$(rg --no-heading --line-number --glob '*.rs' --glob '!**/tests/**' '\.dispatch\(' "${scan_roots[@]}" 2>/dev/null || true)"

if [[ -z "$hits" ]]; then
    echo "owned dispatch hot-path scan: 0 occurrences."
    exit 0
fi

echo "owned dispatch hot-path calls detected:" >&2
printf '%s\n' "$hits" >&2
echo >&2 ""
echo "Fix: build borrowed rows with inputs.iter().map(Vec::as_slice) and call dispatch_borrowed." >&2
exit 1
