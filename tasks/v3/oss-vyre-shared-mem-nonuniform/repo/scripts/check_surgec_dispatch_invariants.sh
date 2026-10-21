#!/usr/bin/env bash
# V1 / V2 / V3 / V4  -  surgec dispatch invariants.
#
# These four audit rows describe surgec's eventual route through the
# vyre-runtime megakernel:
#
#   V1  -  surgec scan dispatch goes through MegakernelDispatch.
#   V2  -  program-graph CSR buffers stay GPU-resident across calls.
#   V3  -  host-driven fixpoint loops replaced by persistent_fixpoint.
#   V4  -  two-tier pipeline cache hit path (in-memory + disk).
#
# The runtime side of each (megakernel protocol, persistent buffer
# pool, persistent_fixpoint primitive, two-tier cache) already ships
# in this workspace. The remaining wiring lives in surgec, which is
# OUTSIDE this workspace. This gate verifies the vyre-side
# preconditions so surgec can rely on them.
#
# Usage:
#   scripts/check_surgec_dispatch_invariants.sh

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

errors=()

# V1: MegakernelDispatch surface exposed.
if ! grep -q 'pub struct MegakernelDispatch\|pub trait MegakernelDispatch' \
        vyre-runtime/src/megakernel/*.rs vyre-driver-wgpu/src/megakernel*.rs 2>/dev/null; then
    errors+=("V1: MegakernelDispatch surface not found  -  surgec cannot route through it")
fi

# V2: Persistent buffer pool exposed (residency comes through tiered acquire/release).
if ! grep -q 'pub struct BufferPool\|pub fn acquire\|pub fn release' \
        vyre-driver-wgpu/src/buffer/pool.rs 2>/dev/null; then
    errors+=("V2: BufferPool surface not found in vyre-driver-wgpu/src/buffer/pool.rs")
fi

# V3: persistent_fixpoint primitive ships.
if [[ ! -f vyre-primitives/src/fixpoint/persistent_fixpoint.rs ]]; then
    errors+=("V3: persistent_fixpoint primitive missing")
fi

# V4: Two-tier cache (in-memory + disk via LayeredPipelineCache).
if ! grep -q 'pub struct LayeredPipelineCache\|pub struct InMemoryPipelineCache\|pub struct DiskCache' \
        vyre-runtime/src/pipeline_cache/*.rs 2>/dev/null; then
    errors+=("V4: two-tier cache types not found in vyre-runtime/src/pipeline_cache/")
fi

if (( ${#errors[@]} > 0 )); then
    echo "surgec-dispatch-invariants gate: ${#errors[@]} preconditions broken." >&2
    for e in "${errors[@]}"; do
        echo "  $e" >&2
    done
    echo >&2
    echo "Fix: a vyre-side surface that surgec depends on has regressed." >&2
    echo "Restore the type/file before merging." >&2
    exit 1
fi

echo "surgec-dispatch-invariants gate: V1-V4 preconditions in place; surgec wiring tracked downstream."
exit 0
