#!/usr/bin/env bash
# Run a shard of volume-wave oracle matrices (16k cases each) for CI/runtime validation.
#
# Usage:
#   scripts/run_volume_sweep_shard.sh [shard_index] [shard_count]
#   VYRE_VOLUME_SHARD=0 VYRE_VOLUME_SHARDS=8 scripts/run_volume_sweep_shard.sh
#
# Default: shard 0 of 4 (quarter of all sweep_*_volume_oracle_matrix targets).

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
source scripts/lib/cargo_runner.sh
vyre_select_cargo_runner

SHARD="${1:-${VYRE_VOLUME_SHARD:-0}}"
SHARDS="${2:-${VYRE_VOLUME_SHARDS:-4}}"

mapfile -t ALL_TARGETS < <(
    rg -l 'volume_oracle_matrix' vyre-primitives/tests vyre-reference/tests vyre-foundation/tests 2>/dev/null \
        | sed -E 's#.*/([^/]+)\.rs$#\1#' \
        | sort -u
)

if ((${#ALL_TARGETS[@]} == 0)); then
    echo "no volume oracle matrix targets found" >&2
    exit 1
fi

SELECTED=()
for i in "${!ALL_TARGETS[@]}"; do
    if (( i % SHARDS == SHARD )); then
        SELECTED+=("${ALL_TARGETS[$i]}")
    fi
done

echo "volume shard ${SHARD}/${SHARDS}: ${#SELECTED[@]} of ${#ALL_TARGETS[@]} targets"

FEATURES='cpu-parity,bitset,graph,reduce,hash,predicate,text'
args=()
for t in "${SELECTED[@]}"; do
    args+=(--test "$t")
done

"$CARGO_RUNNER" test -p vyre-primitives --features "$FEATURES" "${args[@]}" -q
