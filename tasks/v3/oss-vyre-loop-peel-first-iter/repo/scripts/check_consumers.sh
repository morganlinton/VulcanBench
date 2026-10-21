#!/usr/bin/env bash
# Dry-check downstream consumers that are present in the local Santh tree.

set -euo pipefail
cd "$(dirname "$0")/.."
source scripts/lib/cargo_runner.sh
vyre_select_cargo_runner

FAIL=0
FOUND=0

check_consumer() {
    local rel="$1"
    local manifest="$rel/Cargo.toml"
    if [ ! -f "$manifest" ]; then
        echo "skip missing consumer: $rel"
        return
    fi
    FOUND=1
    echo "checking consumer: $rel"
    if ! "$CARGO_RUNNER" check --manifest-path "$manifest"; then
        echo "consumer check failed: $rel" >&2
        FAIL=1
    fi
}

check_consumer "../../../../software/surgec"
check_consumer "../../../../software/pyrograph"
check_consumer "../../../../software/warpscan"

if [ "$FOUND" -eq 0 ]; then
    echo "no downstream consumers found; skipped."
fi

exit "$FAIL"
