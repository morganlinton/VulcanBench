#!/usr/bin/env bash
# P-DELETE-10 + P-DELETE-1 + P-UNIFY-2 + P-UNIFY-4 + P-UNIFY-1b  - 
# unification baselines.
#
# Each audit row asks for a cross-crate refactor: drop a duplicate,
# unify a planning surface, lift a substrate from a backend crate up
# into the driver tier. Each one is a multi-day refactor that has to
# land alongside cross-crate API changes; doing them in a single
# session would cascade through the build.
#
# This gate locks the current state by counting the offending sites
# per audit row and ratcheting downward. Adding a new
# match-on-Node validator (P-DELETE-1), a new BufferAccess auto-
# inference helper (P-DELETE-10), a new cpu_references parallel impl
# (P-UNIFY-2), or a new fusion-planning surface (P-UNIFY-4) is a
# regression. Removing one decreases the floor.
#
# The architectural targets live in `docs/MIGRATION.md` under the
# "Future migrations" section.
#
# Usage:
#   scripts/check_unification_baselines.sh           # enforce
#   scripts/check_unification_baselines.sh --report  # print every count

set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

mode="${1:-enforce}"

# Each row: name@@pattern@@search_paths@@floor
ROWS=(
    "P-DELETE-1__match_on_Node@@^[[:space:]]*match node[[:space:]]+\\{@@vyre-foundation/src/validate vyre-foundation/src/transform@@18"
    "P-DELETE-10__buffer_access_auto@@BufferAccess::(infer|auto|derive_from)@@vyre-foundation/src/lower vyre-driver-wgpu/src/lowering vyre-runtime/src/megakernel@@0"
    "P-UNIFY-2__cpu_references@@fn cpu_reference\\b@@vyre-foundation/src/cpu_references.rs vyre-reference/src/dialect_dispatch.rs@@0"
    "P-UNIFY-4__fusion_planning@@fn (plan_fusion|fuse_programs|tensor_network_fusion_order)\\b@@vyre-foundation/src/optimizer vyre-driver/src/self_substrate vyre-runtime/src/megakernel@@0"
    "P-UNIFY-1b__cache_in_wgpu@@impl PipelineCacheStore for@@vyre-driver-wgpu/src@@0"
)

errors=()
report=()

for row in "${ROWS[@]}"; do
    name=$(printf '%s' "$row" | awk -F'@@' '{print $1}')
    pattern=$(printf '%s' "$row" | awk -F'@@' '{print $2}')
    paths=$(printf '%s' "$row" | awk -F'@@' '{print $3}')
    floor=$(printf '%s' "$row" | awk -F'@@' '{print $4}')
    # shellcheck disable=SC2206
    path_arr=( $paths )
    valid_paths=()
    for p in "${path_arr[@]}"; do
        [[ -e "$p" ]] && valid_paths+=("$p")
    done
    count=0
    if (( ${#valid_paths[@]} > 0 )); then
        if hits=$(grep -rnE "$pattern" --include='*.rs' "${valid_paths[@]}" 2>/dev/null | grep -vE '/tests/|_tests\.rs:|test_fixtures' || true); then
            if [[ -n "$hits" ]]; then
                count=$(printf '%s\n' "$hits" | wc -l | tr -d ' ')
            fi
        fi
    fi
    report+=("$name: $count (floor=$floor)")
    if (( count > floor )); then
        errors+=("$name: $count exceeds floor $floor  -  ratchet violated")
    fi
done

if [[ "$mode" == "--report" ]]; then
    for r in "${report[@]}"; do echo "  $r"; done
    exit 0
fi

if (( ${#errors[@]} > 0 )); then
    echo "unification-baselines gate: ${#errors[@]} ratchets violated." >&2
    for e in "${errors[@]}"; do echo "  $e" >&2; done
    echo >&2
    echo "Fix: bring the count back to or below the floor. Each audit row" >&2
    echo "tracks a cross-crate refactor; new sites are a regression." >&2
    echo "Lowering the floor follows a real refactor  -  update the floor in" >&2
    echo "scripts/check_unification_baselines.sh in the same patch." >&2
    exit 1
fi

echo "unification-baselines gate: every ratchet at or below floor."
exit 0
