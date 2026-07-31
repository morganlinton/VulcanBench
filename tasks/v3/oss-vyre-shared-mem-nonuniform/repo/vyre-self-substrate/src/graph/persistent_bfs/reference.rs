use vyre_primitives::graph::persistent_bfs::try_cpu_ref as try_reference_persistent_bfs;

/// Run up to `max_iters` BFS steps starting from `frontier_in`,
/// returning the saturated frontier and a sticky changed-flag (1 if
/// any iteration added new bits, 0 if the seed was already
/// saturated). Bumps the dataflow-fixpoint substrate counter.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn bfs_expand(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    frontier_in: &[u32],
    allow_mask: u32,
    max_iters: u32,
) -> (Vec<u32>, u32) {
    try_bfs_expand(
        node_count,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        frontier_in,
        allow_mask,
        max_iters,
    )
    .unwrap_or_else(|err| panic!("persistent BFS self-substrate reference rejected input. {err}"))
}

/// Fallible persistent-BFS substrate reference wrapper.
#[cfg(any(test, feature = "cpu-parity"))]
#[allow(clippy::too_many_arguments)]
pub fn try_bfs_expand(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    frontier_in: &[u32],
    allow_mask: u32,
    max_iters: u32,
) -> Result<(Vec<u32>, u32), String> {
    use crate::observability::{bump, graph_dispatch_calls};
    bump(&graph_dispatch_calls);
    try_reference_persistent_bfs(
        node_count,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        frontier_in,
        allow_mask,
        max_iters,
    )
}

/// Convenience: compute the forward-reachable set of `seed` under
/// `allow_mask` with a generous iteration budget. Returns just the
/// frontier; callers wanting the changed-flag should use
/// [`bfs_expand`] directly.
#[must_use]
#[cfg(test)]
pub fn forward_reach(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    seed: &[u32],
    allow_mask: u32,
) -> Vec<u32> {
    let (out, _changed) = bfs_expand(
        node_count,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        seed,
        allow_mask,
        node_count,
    );
    out
}
