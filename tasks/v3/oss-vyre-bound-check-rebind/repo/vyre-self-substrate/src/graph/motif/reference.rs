use vyre_primitives::graph::motif::{
    cpu_ref_matches as reference_motif_matches, try_cpu_ref_into as try_reference_motif_into,
    try_cpu_ref_participation_count as try_reference_motif_participation_count, MotifEdge,
};

/// Match a motif (small directed pattern) against a CSR-encoded
/// Region-graph and return the per-node participation byte-vector
/// (1 = node participates in a full motif match, 0 otherwise).
///
/// `node_count` is the number of Regions; `edge_offsets`/`edge_targets`
/// are the CSR; `edge_kind_mask` carries per-edge kind bits parallel
/// to `edge_targets`. Bumps the dataflow-fixpoint substrate counter
/// (the closest existing counter for graph-walk primitives) so
/// dispatch dashboards register motif match traffic.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn match_motif(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    motif_edges: &[MotifEdge],
) -> Vec<u32> {
    try_match_motif(
        node_count,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        motif_edges,
    )
    .unwrap_or_else(|err| panic!("motif self-substrate reference rejected input. {err}"))
}

/// Fallible motif reference wrapper.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_match_motif(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    motif_edges: &[MotifEdge],
) -> Result<Vec<u32>, String> {
    use crate::observability::{bump, graph_dispatch_calls};
    bump(&graph_dispatch_calls);
    let mut witness = Vec::new();
    try_reference_motif_into(
        node_count,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        motif_edges,
        &mut witness,
    )?;
    Ok(witness)
}

/// Convenience: returns true iff any node participates in a motif
/// match (i.e. the motif fully matched at least once on the graph).
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn motif_matches(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    motif_edges: &[MotifEdge],
) -> bool {
    try_motif_matches(
        node_count,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        motif_edges,
    )
    .unwrap_or_else(|err| panic!("motif self-substrate match reference rejected input. {err}"))
}

/// Checked motif existence wrapper.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_motif_matches(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    motif_edges: &[MotifEdge],
) -> Result<bool, String> {
    Ok(try_motif_participation_count(
        node_count,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        motif_edges,
    )? != 0
        && reference_motif_matches(edge_offsets, edge_targets, edge_kind_mask, motif_edges))
}

/// Count the number of distinct nodes participating in motif
/// matches over the graph. Useful as a dispatch-time signal: high
/// participation suggests the motif is endemic and worth a
/// dedicated rewrite pass.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn motif_participation_count(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    motif_edges: &[MotifEdge],
) -> u32 {
    try_motif_participation_count(
        node_count,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        motif_edges,
    )
    .unwrap_or_else(|err| {
        panic!("motif self-substrate participation reference rejected input. {err}")
    })
}

/// Checked motif participation-count wrapper.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_motif_participation_count(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    motif_edges: &[MotifEdge],
) -> Result<u32, String> {
    use crate::observability::{bump, graph_dispatch_calls};
    bump(&graph_dispatch_calls);
    try_reference_motif_participation_count(
        node_count,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        motif_edges,
    )
}
