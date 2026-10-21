#!/usr/bin/env bash
# Inventory #2  -  direct wgpu dispatch must stage ordinary outputs through the
# size-classed readback ring, not fresh per-output MAP_READ buffers.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

record="vyre-driver-wgpu/src/engine/record_and_readback.rs"
record_modules="vyre-driver-wgpu/src/engine/record_and_readback"
arena="vyre-driver-wgpu/src/lib.rs"

required_patterns=(
    "readback_rings:"
    "SubmittedReadback::Ring"
    ".record_copy("
    ".arm_ticket("
    ".with_mapped_ticket("
)

for pattern in "${required_patterns[@]}"; do
    if ! rg --fixed-strings --quiet "$pattern" "$record" "$record_modules"; then
        echo "direct readback ring gate failed: missing '$pattern' in record/readback modules" >&2
        echo "Fix: route ordinary output readbacks through ReadbackRing slots before falling back to pooled staging." >&2
        exit 1
    fi
done

if ! rg --fixed-strings --quiet "ReadbackRingSet::new()" "$arena"; then
    echo "direct readback ring gate failed: DispatchArena does not own a ReadbackRingSet" >&2
    echo "Fix: keep direct readback rings in the backend dispatch arena so hot dispatches reuse staging slots." >&2
    exit 1
fi

if rg --quiet 'for output in request\.output_bindings[\s\S]*pool\s*\.\s*acquire' "$record"; then
    echo "direct readback ring gate failed: ordinary output loop still acquires pooled MAP_READ buffers directly" >&2
    echo "Fix: only trap/timestamp sidecars should use pooled MAP_READ staging in record_and_readback." >&2
    exit 1
fi

echo "direct readback ring default gate: OK"
