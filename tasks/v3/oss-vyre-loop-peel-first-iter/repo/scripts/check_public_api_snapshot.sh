#!/usr/bin/env bash
# Public-API stability gate.
#
# Extracts every `pub` item from each published crate's src/ tree and
# diffs against docs/public-api/<crate>.txt. Any drift requires
# --refresh + a matching CHANGELOG entry.
#
# Usage:
#   scripts/check_public_api_snapshot.sh                  # verify
#   scripts/check_public_api_snapshot.sh --refresh        # regenerate

set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

SNAPSHOT_DIR="docs/public-api"
mkdir -p "$SNAPSHOT_DIR"

PUBLISHED_CRATES=(
    "vyre-core:vyre"
    "vyre-driver:vyre-driver"
    "vyre-driver-wgpu:vyre-driver-wgpu"
    "vyre-foundation:vyre-foundation"
    "vyre-primitives:vyre-primitives"
    "vyre-spec:vyre-spec"
)

extract_api() {
    local src_dir="$1"
    grep -rhE '^[[:space:]]*pub[[:space:]]+(fn|struct|enum|trait|const|static|type|mod|use)[[:space:]]' \
        "$src_dir" 2>/dev/null \
        | grep -vE '^[[:space:]]*pub[[:space:]]+use[[:space:]]' \
        | sed -E 's/[[:space:]]+/ /g; s/^ //; s/ $//' \
        | sort -u
}

refresh=0
if [[ "${1:-}" == "--refresh" ]]; then refresh=1; fi

failed=0
for entry in "${PUBLISHED_CRATES[@]}"; do
    crate_dir="${entry%:*}"
    crate_name="${entry#*:}"
    src="$crate_dir/src"
    snap="$SNAPSHOT_DIR/${crate_name}.txt"

    [[ ! -d "$src" ]] && continue

    current="$(extract_api "$src")"
    [[ -z "$current" ]] && continue

    if [[ "$refresh" -eq 1 ]]; then
        printf '%s\n' "$current" > "$snap"
        echo "refreshed: $snap"
        continue
    fi

    if [[ ! -f "$snap" ]]; then
        echo "MISSING SNAPSHOT: $snap. Fix: run --refresh AND bump the crate version." >&2
        failed=1
        continue
    fi

    expected="$(cat "$snap")"
    if [[ "$current" != "$expected" ]]; then
        echo "PUBLIC-API DRIFT: $crate_name" >&2
        diff <(echo "$expected") <(echo "$current") | head -20 >&2
        echo "Fix: refresh snapshot AND add CHANGELOG entry in the same commit." >&2
        failed=1
    fi
done

if [[ "$failed" -ne 0 ]]; then
    exit 1
fi
if [[ "$refresh" -eq 0 ]]; then
    echo "Public API: all crates byte-stable."
fi
exit 0
