#!/usr/bin/env bash
# check_cuda_parity_perf_gate.sh
# CUDA parity/performance gate — contract + full *gpu_parity* integration suite.
# VYRE-TASK-000006: gate must exercise documented gpu_parity tests, not a narrow subset.
# VYRE-TASK-000005: packed INT4 extension ops are covered by int4_quantized_gpu_parity
# (all six quant.int4.* harness ids: dot i32/scaled, matvec, batched matvec/matmul/top1).

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if ! nvidia-smi >/dev/null 2>&1; then
    echo "CUDA gate requires a live NVIDIA GPU, but nvidia-smi failed." >&2
    echo "Fix: repair CUDA/NVIDIA driver visibility; do not skip CUDA parity on this GPU fleet." >&2
    exit 1
fi

source scripts/lib/cargo_runner.sh
vyre_select_cargo_runner

CONTRACT_TESTS=(
    capability_contracts
    cuda_device_contract
    cuda_release_surface_contracts
    gpu_elementwise_conformance
    megakernel_scale_scheduler_contracts
    module_cache_contracts
)

mapfile -t PARITY_TESTS < <(
    find vyre-driver-cuda/tests -maxdepth 1 -name '*gpu_parity*.rs' -printf '%f\n' \
        | sed 's/\.rs$//' | sort -u
)

echo "CUDA parity gate: ${#CONTRACT_TESTS[@]} contract tests, ${#PARITY_TESTS[@]} gpu_parity integration tests"

for test in "${CONTRACT_TESTS[@]}"; do
    echo "==> contract: $test"
    "$CARGO_RUNNER" test -p vyre-driver-cuda --test "$test" -- --nocapture
done

for test in "${PARITY_TESTS[@]}"; do
    echo "==> gpu_parity: $test"
    "$CARGO_RUNNER" test -p vyre-driver-cuda --test "$test" -- --nocapture
done

echo "CUDA parity gate: all ${#PARITY_TESTS[@]} gpu_parity tests and ${#CONTRACT_TESTS[@]} contract tests passed"
