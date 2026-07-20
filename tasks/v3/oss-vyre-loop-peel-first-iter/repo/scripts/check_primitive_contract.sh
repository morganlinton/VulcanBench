#!/usr/bin/env bash
# check_primitive_contract.sh  -  enforce the per-primitive contract
# from skills/SKILL_BUILD_DATAFLOW_PRIMITIVE.md against one or more
# changed primitive files.
#
# Usage:
#   check_primitive_contract.sh <file.rs> [<file.rs> ...]
#   check_primitive_contract.sh           # scan every changed file under
#                                         # vyre-libs/src/{security,dataflow}
#                                         # and vyre-primitives/src/{bitset,graph}
#                                         # since the merge-base
#
# Exit 0 = all files pass. Nonzero = number of files that failed.

set -uo pipefail

FAILURES=0

check_one() {
    local f=$1
    if [[ ! -f "$f" ]]; then
        echo "FAIL: $f does not exist" >&2
        return 1
    fi

    local fail=0
    local why=()

    # 1. Must declare an OP_ID.
    if ! grep -q 'pub(crate) const OP_ID: &str' "$f" \
        && ! grep -q 'pub const OP_ID: &str' "$f"; then
        why+=("missing pub(crate)/pub const OP_ID")
        fail=1
    fi

    # 2. Must have a CPU oracle (cpu_ref function).
    if ! grep -q 'pub fn cpu_ref' "$f"; then
        why+=("missing pub fn cpu_ref")
        fail=1
    fi

    # 3. Must have at least 4 #[test] items in a tests module.
    local n_tests
    n_tests=$(grep -c '^[[:space:]]*#\[test\]' "$f" || true)
    if [[ "$n_tests" -lt 4 ]]; then
        why+=("only $n_tests #[test] items, contract requires >=4")
        fail=1
    fi

    # 4. File must be <=600 LOC (excluding blank lines).
    local n_loc
    n_loc=$(grep -cv '^[[:space:]]*$' "$f" || true)
    if [[ "$n_loc" -gt 600 ]]; then
        why+=("$n_loc LOC > 600 budget")
        fail=1
    fi

    # 5. No forbidden patterns.
    if grep -q 'Program::new(' "$f"; then
        why+=("uses Program::new  -  must use Program::wrapped")
        fail=1
    fi
    if grep -qE '_ => panic!|_ => todo!|_ => unimplemented!' "$f"; then
        why+=("catch-all panic/todo/unimplemented arm")
        fail=1
    fi
    if grep -qE 'expect\("never |expect\("can\\?''t |expect\("infallible' "$f"; then
        why+=("expect with 'never/can't/infallible' message")
        fail=1
    fi

    # 6. Must have a module doc comment (//! at the top).
    if ! head -1 "$f" | grep -q '^//!'; then
        why+=("missing //! module doc comment at top")
        fail=1
    fi

    if [[ "$fail" -ne 0 ]]; then
        echo "FAIL: $f" >&2
        for reason in "${why[@]}"; do
            echo "  - $reason" >&2
        done
        FAILURES=$((FAILURES + 1))
    fi
}

if [[ "$#" -gt 0 ]]; then
    for f in "$@"; do
        check_one "$f"
    done
else
    ROOT="$(cd "$(dirname "$0")/.." && pwd)"
    while IFS= read -r -d '' f; do
        # Skip mod.rs (re-exporters) and known non-primitive files.
        case "$f" in
            */mod.rs|*/lib.rs|*/region/*.rs|*/harness.rs|*/soundness.rs|*/markers.rs)
                continue ;;
        esac
        check_one "$f"
    done < <(find "$ROOT/vyre-libs/src/security" \
                  "$ROOT/vyre-libs/src/dataflow" \
                  "$ROOT/vyre-primitives/src/bitset" \
                  "$ROOT/vyre-primitives/src/graph" \
                  -name '*.rs' -print0 2>/dev/null)
fi

if [[ "$FAILURES" -gt 0 ]]; then
    echo
    echo "FAIL: $FAILURES primitive contract violations" >&2
    exit "$FAILURES"
fi
echo "OK: all primitives satisfy the contract"
exit 0
