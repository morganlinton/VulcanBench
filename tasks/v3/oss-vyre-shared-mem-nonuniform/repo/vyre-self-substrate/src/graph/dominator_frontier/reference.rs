use vyre_primitives::graph::dominator_frontier::try_cpu_ref as try_reference_dominator_frontier;

/// Compute the dominance frontier for `seed` over the Region graph
/// described by the CSR dominance closure (`dom_offsets`/`dom_targets`,
/// row `n` = every Region dominated by `n` including `n`) and the CSR
/// predecessor list (`pred_offsets`/`pred_targets`, row `m` = Regions
/// with an edge into `m`). `seed` is the packed-u32 bitset of selected
/// nodes; `node_count` matches the bitset width.
///
/// Returns the frontier bitset: the set of Regions where seed
/// influence must be reconciled. Bumps the substrate-call counter so
/// observability dashboards can see the dispatch is exercising the
/// primitive.
#[must_use]
pub fn compute_dominance_frontier(
    node_count: u32,
    dom_offsets: &[u32],
    dom_targets: &[u32],
    pred_offsets: &[u32],
    pred_targets: &[u32],
    seed: &[u32],
) -> Vec<u32> {
    try_compute_dominance_frontier(
        node_count,
        dom_offsets,
        dom_targets,
        pred_offsets,
        pred_targets,
        seed,
    )
    .unwrap_or_else(|_| vec![0; seed.len()])
}

/// Fallible dominance-frontier substrate reference wrapper.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_compute_dominance_frontier(
    node_count: u32,
    dom_offsets: &[u32],
    dom_targets: &[u32],
    pred_offsets: &[u32],
    pred_targets: &[u32],
    seed: &[u32],
) -> Result<Vec<u32>, String> {
    use crate::observability::{bump, graph_dispatch_calls};
    bump(&graph_dispatch_calls);
    try_reference_dominator_frontier(
        node_count,
        dom_offsets,
        dom_targets,
        pred_offsets,
        pred_targets,
        seed,
    )
}
