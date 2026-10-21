#!/usr/bin/env bash
# Every wire-format version has a round-trip or migration test.

set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# Current version: scan for VIR0_VERSION or equivalent constant.
current_version="$(
    grep -rEh 'VIR0_VERSION|WIRE_VERSION' --include='*.rs' vyre-core/src/ir/serial 2>/dev/null \
    | grep -oE '=[[:space:]]*[0-9]+' | grep -oE '[0-9]+' | head -1
)"

if [[ -z "$current_version" ]]; then
    current_version="$(
        grep -rEh 'put_u8.*[,[:space:]]+[0-9]+.*version|version.*put_u8' \
            --include='*.rs' vyre-core/src/ir/serial/wire/ 2>/dev/null \
        | grep -oE '[0-9]+' | head -1
    )"
fi

[[ -z "$current_version" ]] && current_version=1

echo "Wire format version: $current_version"

for v in $(seq 1 "$current_version"); do
    found=0
    for pattern in "wire_migration_v$((v-1))_to_v${v}" "wire_v${v}_round_trip" "wire_round_trip" "wire_opaque_round_trip"; do
        if find vyre-core/tests -name "${pattern}.rs" 2>/dev/null | head -1 | grep -q .; then
            found=1; break
        fi
    done
    if [[ "$found" -eq 0 ]]; then
        echo "FAIL: wire v$v has no round-trip test. Fix: add vyre-core/tests/wire_v${v}_round_trip.rs." >&2
        exit 1
    fi
done

echo "Wire migrations: all versions 1..${current_version} have tests."
