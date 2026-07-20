#!/usr/bin/env bash
# P1 inventory #94 - deep benchmark suite covers every dimension.
#
# The canonical architecture is the vyre-bench meta-harness, not scattered
# Criterion targets. This gate validates stable registry case IDs for the
# dimensions that are owned by vyre-bench and keeps the compile-cache dimension
# tied to the executable driver cache contract until it is promoted into a
# first-class vyre-bench case.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
source scripts/lib/cargo_runner.sh
vyre_select_cargo_runner

registry_json="$(mktemp)"
trap 'rm -f "$registry_json"' EXIT
"$CARGO_RUNNER" run -q -p vyre-bench -- list --format json > "$registry_json"

declare -A CASE_REPRESENTATIVE
CASE_REPRESENTATIVE[throughput]="foundation.dfa_match.256k"
CASE_REPRESENTATIVE[latency]="runtime.megakernel.dispatch.256"
CASE_REPRESENTATIVE[memory]="primitives.graph.frontier_step.1m"
CASE_REPRESENTATIVE[optimizer]="foundation.optimizer.impact"
CASE_REPRESENTATIVE[lowering]="lower.rewrites.impact.corpus"
CASE_REPRESENTATIVE[runtime_queueing]="runtime.megakernel.condition.64k"

errors=()
for dim in "${!CASE_REPRESENTATIVE[@]}"; do
    case_id="${CASE_REPRESENTATIVE[$dim]}"
    if ! grep -q "\"$case_id\"" "$registry_json"; then
        errors+=("$dim: representative vyre-bench case '$case_id' not registered")
    fi
done
compile_cache_contract="vyre-driver-cuda/tests/module_cache_contracts.rs"
if [[ ! -f "$compile_cache_contract" ]]; then
    errors+=("compile_cache: executable cache contract '$compile_cache_contract' not found")
elif ! grep -q "repeated_dispatch_reuses_loaded_cuda_module" "$compile_cache_contract"; then
    errors+=("compile_cache: '$compile_cache_contract' does not pin CUDA module-cache reuse")
fi

if (( ${#errors[@]} > 0 )); then
    echo "deep-bench-coverage gate: ${#errors[@]} dimensions uncovered." >&2
    for e in "${errors[@]}"; do
        echo "  $e" >&2
    done
    echo >&2
    echo "Fix: every dimension (throughput / latency / memory / compile_cache /" >&2
    echo "optimizer / lowering / runtime_queueing) must have executable" >&2
    echo "meta-harness evidence or a named driver contract." >&2
    exit 1
fi

echo "deep-bench-coverage gate: all 7 dimensions covered by vyre-bench registry evidence."
exit 0
