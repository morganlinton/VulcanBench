#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
source scripts/lib/cargo_runner.sh
vyre_select_cargo_runner

if ! nvidia-smi >/dev/null 2>&1; then
    echo "Nightly CI requires a live NVIDIA GPU, but nvidia-smi failed." >&2
    echo "Fix: repair CUDA/NVIDIA driver visibility; do not skip CUDA parity on this GPU fleet." >&2
    exit 1
fi

scripts/check_test_coverage_per_crate.sh
bash scripts/check_roadmap_status_split.sh
bash scripts/check_docs_index.sh

echo "Running reference oracle edge contracts..."
CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-1}" "$CARGO_RUNNER" test -q -p vyre-reference --test oracle_program_edges
CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-1}" "$CARGO_RUNNER" test -q -p vyre-reference --test quantized_buffer_contract
CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-1}" "$CARGO_RUNNER" test -q -p vyre-spec --test invariant_catalog_surface
CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-1}" "$CARGO_RUNNER" test -q -p vyre-spec --test data_type_layout_matrix
CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-1}" "$CARGO_RUNNER" test -q -p vyre-spec --test collective_op_contracts
CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-1}" "$CARGO_RUNNER" test -q -p vyre-macros --test adversarial
CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-1}" "$CARGO_RUNNER" test -q -p vyre-foundation --test wire_fuzz_infra_contracts
CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-1}" "$CARGO_RUNNER" test -q -p vyre-foundation --test autodiff_transform_contracts
CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-1}" "$CARGO_RUNNER" test -q -p vyre-foundation --test collective_ir_contracts
CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-1}" "$CARGO_RUNNER" test -q -p vyre-libs --test hash_single_source_contracts
CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-1}" "$CARGO_RUNNER" test -q -p vyre-bench --test release_matrix_contracts
CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-1}" "$CARGO_RUNNER" test -q -p vyre --test wire_malformed_adversarial
CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-1}" "$CARGO_RUNNER" test -q -p vyre-self-substrate --test organization_contracts
CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-1}" "$CARGO_RUNNER" test -q -p vyre-self-substrate --test graph_single_source_contracts
CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-1}" "$CARGO_RUNNER" test -q -p vyre-self-substrate --test platform_doc_consumer_boundary
scripts/check_ownership_boundaries.sh
scripts/check_spirv_parity_perf_gate.sh
scripts/check_cuda_parity_perf_gate.sh

echo "Running conformance runner to populate certificates..."
mkdir -p certs
"$CARGO_RUNNER" run -p vyre-conform-runner --features gpu -- dispatch --backend wgpu --ops all > certs/wgpu_certs.json
"$CARGO_RUNNER" run -p vyre-conform-runner --features cuda -- dispatch --backend cuda --ops all > certs/cuda_certs.json

echo "Updating docs/parity/three_substrate.md..."
mkdir -p docs/parity
echo "# Byte-Identical Validation Reports" > docs/parity/three_substrate.md
echo "This document confirms byte-identical behavior across wgpu, cuda, and the cpu_ref substrate, with SPIR-V lowering validated by scripts/check_spirv_parity_perf_gate.sh." >> docs/parity/three_substrate.md
echo "\nLast updated: $(date -u +"%Y-%m-%dT%H:%M:%SZ")\n" >> docs/parity/three_substrate.md
echo "## Current Parity Status\nwgpu/cuda dispatch certificates were regenerated locally; SPIR-V parity/performance gate completed in the same run." >> docs/parity/three_substrate.md

echo "Nightly CI completed successfully."
