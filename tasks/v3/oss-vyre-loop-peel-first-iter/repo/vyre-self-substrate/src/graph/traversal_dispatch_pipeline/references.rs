#[cfg(any(test, feature = "cpu-parity"))]
use vyre_primitives::graph::{
    adaptive_traverse::cpu_dense_step,
    csr_forward_or_changed::cpu_ref_closure_into,
    csr_frontier_degree_sum::{csr_frontier_degree_sum_cpu, try_csr_frontier_degree_sum_cpu},
};

/// CPU dense traversal reference used by self-substrate parity tests.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_dense_step(
    frontier_in: &[u32],
    adj_rows_dense: &[u32],
    node_count: u32,
) -> Vec<u32> {
    cpu_dense_step(frontier_in, adj_rows_dense, node_count)
}

/// CPU frontier-degree reference used by self-substrate parity tests.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_frontier_degree_sum(
    frontier_in: &[u32],
    edge_offsets: &[u32],
    node_count: u32,
) -> u32 {
    csr_frontier_degree_sum_cpu(frontier_in, edge_offsets, node_count)
}

/// Checked CPU frontier-degree reference used by self-substrate parity tests.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_reference_frontier_degree_sum(
    frontier_in: &[u32],
    edge_offsets: &[u32],
    node_count: u32,
) -> Result<u32, String> {
    try_csr_frontier_degree_sum_cpu(frontier_in, edge_offsets, node_count)
}

/// CPU closure reference used by self-substrate fixed-point tests.
#[cfg(any(test, feature = "cpu-parity"))]
#[allow(clippy::too_many_arguments)]
pub fn reference_csr_closure_into(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    seed: &[u32],
    allow_mask: u32,
    max_iters: u32,
    current: &mut Vec<u32>,
    next: &mut Vec<u32>,
) {
    cpu_ref_closure_into(
        node_count,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        seed,
        allow_mask,
        max_iters,
        current,
        next,
    );
}
