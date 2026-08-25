#[cfg(any(test, feature = "cpu-parity"))]
pub(crate) use vyre_primitives::graph::csr_bidirectional::cpu_ref as reference_csr_bidir;
#[cfg(test)]
pub(crate) use vyre_primitives::graph::csr_bidirectional::cpu_ref_closure as reference_csr_bidir_closure;
#[cfg(any(test, feature = "cpu-parity"))]
use vyre_primitives::graph::csr_bidirectional::cpu_ref_closure_into_with_step_hook as reference_csr_bidir_closure_into_with_step_hook;

/// Compute one bidirectional BFS step over a CSR-encoded Region
/// graph: returns the bitset that includes every node reachable
/// in <=1 forward edge OR <=1 backward edge from `frontier_in`,
/// filtered by `allow_mask` over edge kinds.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_bidirectional_step(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    frontier_in: &[u32],
    allow_mask: u32,
) -> Vec<u32> {
    use crate::observability::{bump, graph_dispatch_calls};
    bump(&graph_dispatch_calls);
    reference_csr_bidir(
        node_count,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        frontier_in,
        allow_mask,
    )
}

/// Iterate `bidirectional_step` to fixpoint or `max_iters`.
#[must_use]
#[allow(clippy::too_many_arguments)]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_bidirectional_closure(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    seed: &[u32],
    allow_mask: u32,
    max_iters: u32,
) -> Vec<u32> {
    let mut current = Vec::new();
    let mut next = Vec::new();
    reference_bidirectional_closure_into(
        node_count,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        seed,
        allow_mask,
        max_iters,
        &mut current,
        &mut next,
    );
    current
}

/// Iterate `bidirectional_step` to fixpoint using caller-owned buffers.
#[allow(clippy::too_many_arguments)]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_bidirectional_closure_into(
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
    use crate::observability::{bump, graph_dispatch_calls};
    reference_csr_bidir_closure_into_with_step_hook(
        node_count,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        seed,
        allow_mask,
        max_iters,
        current,
        next,
        || bump(&graph_dispatch_calls),
    );
}
