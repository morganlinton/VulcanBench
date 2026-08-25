#!/usr/bin/env bash
# Run every vyre `sweep_*` oracle-matrix integration test with crate-correct --features.
#
# Cargo --test requires exact integration-test binary names (no globs). Feature-gated
# sweeps must enable the same flags as their [[test]] required-features in Cargo.toml.
#
# Usage:
#   scripts/run_sweep_oracle_matrix.sh

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
source scripts/lib/cargo_runner.sh
vyre_select_cargo_runner

step() {
    echo
    echo "▶ $*"
}

PRIMITIVES_FEATURES='cpu-parity,bitset,graph,reduce,hash,predicate'
PRIMITIVES_SWEEPS=(
    sweep_bitset_binary_oracle_matrix
    sweep_graph_csr_bidirectional_oracle_matrix
    sweep_graph_motif_oracle_matrix
    sweep_graph_path_reconstruct_oracle_matrix
    sweep_hash_crc_oracle_matrix
    sweep_hash_fnv1a_oracle_matrix
    sweep_predicate_node_kind_oracle_matrix
    sweep_radix_sort_oracle_matrix
    sweep_reduce_scalar_oracle_matrix
    sweep_segment_reduce_oracle_matrix
    sweep_toposort_oracle_matrix
)

step "vyre-primitives sweep oracle matrices (${#PRIMITIVES_SWEEPS[@]} targets)"
primitives_test_args=()
for target in "${PRIMITIVES_SWEEPS[@]}"; do
    primitives_test_args+=(--test "$target")
done
"$CARGO_RUNNER" test -p vyre-primitives --features "$PRIMITIVES_FEATURES" \
    "${primitives_test_args[@]}"

step "vyre-foundation sweep_validation_rejection_oracle_matrix"
"$CARGO_RUNNER" test -p vyre-foundation --test sweep_validation_rejection_oracle_matrix

step "vyre-spec sweep_wire_roundtrip_oracle_matrix"
"$CARGO_RUNNER" test -p vyre-spec --test sweep_wire_roundtrip_oracle_matrix

step "vyre-reference sweep_dual_arith_oracle_matrix"
"$CARGO_RUNNER" test -p vyre-reference --test sweep_dual_arith_oracle_matrix

step "vyre-libs sweep oracle matrices (logical, hash, decode, text)"
"$CARGO_RUNNER" test -p vyre-libs --features logical,hash,decode,text \
    --test sweep_logical_reference_matrix \
    --test sweep_hash_crc32_reference_matrix \
    --test sweep_decode_hex_oracle_matrix \
    --test sweep_text_utf8_oracle_matrix

step "vyre-self-substrate sweep_graph_cpu_oracle_matrix"
"$CARGO_RUNNER" test -p vyre-self-substrate --features cpu-parity \
    --test sweep_graph_cpu_oracle_matrix

step "vyre-driver sweep oracle matrices"
"$CARGO_RUNNER" test -p vyre-driver \
    --test sweep_dispatch_shape_oracle_matrix \
    --test sweep_numeric_oracle_matrix

step "vyre-runtime sweep oracle matrices"
"$CARGO_RUNNER" test -p vyre-runtime \
    --test sweep_tenant_policy_oracle_matrix \
    --test sweep_ring_buffer_oracle_matrix

echo
echo "All sweep_* oracle matrix integration tests passed."
