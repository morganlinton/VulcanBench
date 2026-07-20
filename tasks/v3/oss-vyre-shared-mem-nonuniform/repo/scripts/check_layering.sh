#!/usr/bin/env bash
# Ω.3  -  Machine-checkable enforcement of the v0.4.1 layer DAG
# (COMPUTE_2_0.md §3, R1–R6).
#
# Cross-layer imports go DOWN only. Violations fail CI, not review.

set -euo pipefail
cd "$(dirname "$0")/.."

FAIL=0
source scripts/lib/cargo_runner.sh
vyre_select_cargo_runner

check_deps() {
    local crate="$1"
    local forbidden="$2"
    local tree
    if ! tree=$("$CARGO_RUNNER" tree -p "$crate" --edges=normal --prefix=none 2>/dev/null | sort -u); then
        echo "layer check: skipping missing workspace crate $crate"
        return
    fi
    if echo "$tree" | grep -qE "^($forbidden) "; then
        echo "LAYER VIOLATION: $crate depends on $forbidden" >&2
        echo "$tree" | grep -E "^($forbidden) " >&2
        FAIL=1
    fi
}

# R1: vyre-foundation depends only on vyre-spec, vyre-macros, and lightweight
# data crates. No driver, ops, conform, wgpu, naga, or toml.
check_deps vyre-foundation "vyre-driver|vyre-driver-wgpu|vyre-driver-spirv|vyre-ops|vyre-conform|wgpu|naga"

# R2: vyre-driver is substrate-agnostic. No backend-specific or stdlib deps.
check_deps vyre-driver "vyre-ops|vyre-driver-wgpu|vyre-driver-spirv|wgpu"

# R3: vyre-ops is the stdlib tier. No backend-specific deps.
check_deps vyre-ops "vyre-driver-wgpu|vyre-driver-spirv|wgpu"

# R5 strict: vyre-reference is foundation-only. It must not depend on the
# driver tier, backend crates, or the `vyre` meta shim.
check_deps vyre-reference "vyre|vyre-driver|vyre-driver-wgpu|vyre-driver-spirv|wgpu"
if grep -Eq '^[[:space:]]*(vyre|vyre-driver)[[:space:]]*=' vyre-reference/Cargo.toml; then
    echo "LAYER VIOLATION: vyre-reference/Cargo.toml must not depend on vyre or vyre-driver" >&2
    grep -En '^[[:space:]]*(vyre|vyre-driver)[[:space:]]*=' vyre-reference/Cargo.toml >&2
    FAIL=1
fi

if [ "$FAIL" -eq 1 ]; then
    echo "" >&2
    echo "One or more layer violations detected. See COMPUTE_2_0.md §3." >&2
    exit 1
fi
echo "All layer invariants green."
