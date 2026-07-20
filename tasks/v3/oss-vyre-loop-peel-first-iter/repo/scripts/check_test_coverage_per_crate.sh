#!/usr/bin/env bash
# Audit gap  -  file-level test coverage ratchet per crate.
#
# Counts the number of `*.rs` files under each crate's `src/` and
# `tests/` that contain at least one `#[test]` attribute, expressed
# as a percentage of total `*.rs` files. The audit dated 2026-04-28
# observed that 449/999 source files (45%) had a test; some crates scored
# 0% at the time while others scored well. CUDA is hardware-required in
# this GPU fleet, so CUDA tests count directly in this crate-level floor.
#
# This gate ratchets the ratio per crate. It does NOT audit test
# quality  -  that is the proptest / adversarial / SQLite-bar
# discipline elsewhere. It does prevent the file-coverage ratio
# from silently shrinking.
#
# Usage:
#   scripts/check_test_coverage_per_crate.sh           # enforce
#   scripts/check_test_coverage_per_crate.sh --report  # print current state

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# Per-crate floors (% of `.rs` files that contain `#[test]`).
# Floors begin at the audit-date measurement; bump upward as new
# tests land. Crates with documented rationale for low coverage
# (proc-macro tests live in callers) get a documented lower floor.
declare -A FLOOR
FLOOR["vyre-core"]=0          # 1 file, no tests
FLOOR["vyre-foundation"]=15
FLOOR["vyre-driver"]=10
FLOOR["vyre-driver-wgpu"]=10
FLOOR["vyre-driver-metal"]=50
FLOOR["vyre-driver-spirv"]=30
FLOOR["vyre-driver-cuda"]=50
FLOOR["vyre-driver-reference"]=10
FLOOR["vyre-reference"]=32
FLOOR["vyre-spec"]=33
FLOOR["vyre-macros"]=18       # proc-macro; includes trybuild and release-surface tests
FLOOR["vyre-primitives"]=20
FLOOR["vyre-runtime"]=15
FLOOR["vyre-libs"]=15
FLOOR["vyre-intrinsics"]=10
FLOOR["vyre-cc"]=5
FLOOR["vyre-aot"]=10
FLOOR["vyre-harness"]=10

mode="${1:-enforce}"

errors=()

for crate in "${!FLOOR[@]}"; do
    src="$crate/src"
    tdir="$crate/tests"
    [[ ! -d "$src" ]] && continue
    total=0
    tested=0
    while IFS= read -r f; do
        total=$((total + 1))
        if grep -qE '#\[test\]' "$f" 2>/dev/null; then
            tested=$((tested + 1))
        fi
    done < <(find "$src" "$tdir" -type f -name '*.rs' 2>/dev/null)
    if (( total == 0 )); then
        continue
    fi
    pct=$(( (tested * 100) / total ))
    floor=${FLOOR[$crate]}
    if [[ "$mode" == "--report" ]]; then
        echo "$(printf '%-25s' "$crate") $tested/$total = ${pct}% (floor=${floor}%)"
    elif (( pct < floor )); then
        errors+=("$crate: $tested/$total = ${pct}%  -  below floor ${floor}%")
    fi
done

[[ "$mode" == "--report" ]] && exit 0

if (( ${#errors[@]} > 0 )); then
    echo "test-coverage-per-crate gate: ${#errors[@]} crates below floor." >&2
    for e in "${errors[@]}"; do
        echo "  $e" >&2
    done
    echo >&2
    echo "Fix: add a #[test] to a file in the offending crate, or document" >&2
    echo "why the floor should drop in scripts/check_test_coverage_per_crate.sh." >&2
    exit 1
fi

echo "test-coverage-per-crate gate: every crate clears its floor."
exit 0
