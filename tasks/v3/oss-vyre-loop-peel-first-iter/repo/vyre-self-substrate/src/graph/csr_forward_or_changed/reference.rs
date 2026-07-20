use vyre_primitives::graph::csr_forward_or_changed::{
    cpu_ref as csr_foc_cpu,
    cpu_ref_closure_into_with_step_hook as csr_foc_closure_into_with_step_hook,
};

/// Run one in-place forward-expand step over the CSR graph and
/// return both the new frontier and a 0/1 changed flag. The
/// primitive's contract: bits added to the frontier flip the flag;
/// no new bits → flag stays 0 → caller's fixpoint loop terminates.
///
/// Bumps the dataflow-fixpoint substrate counter so observability
/// logs every change-detection step.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_forward_step_with_change_flag(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    frontier: &[u32],
    allow_mask: u32,
) -> (Vec<u32>, u32) {
    use crate::observability::{bump, graph_dispatch_calls};
    bump(&graph_dispatch_calls);
    csr_foc_cpu(
        node_count,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        frontier,
        allow_mask,
    )
}

/// Iterate `forward_step_with_change_flag` until the change flag
/// reads 0 or `max_iters` is reached. Returns the saturated
/// frontier.
///
/// This is the substrate path for "expand a Region set to its
/// forward-reachable closure"  -  the same fixpoint loop the
/// optimizer used to write by hand, now driven by the primitive's
/// own change flag.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_forward_closure_via_change_flag(
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
    reference_forward_closure_via_change_flag_into(
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

/// Iterate `forward_step_with_change_flag` using caller-owned scratch.
#[allow(clippy::too_many_arguments)]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_forward_closure_via_change_flag_into(
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
    csr_foc_closure_into_with_step_hook(
        node_count,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        seed,
        allow_mask,
        max_iters,
        current,
        next,
        |_| bump(&graph_dispatch_calls),
    );
}
