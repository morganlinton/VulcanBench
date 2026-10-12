//! Vector-to-neighbor-graph fusion contract.
//!
//! This module is the shared boundary between ANN-style vector ranking and
//! graph traversal primitives. It constructs a deterministic k-nearest-neighbor
//! CSR graph from vector rows, traverses that graph from the query-nearest seed,
//! and records whether graph-ranked top-k matches direct vector top-k.

use std::collections::VecDeque;

/// Schema version for vector-to-graph fusion evidence.
pub const VECTOR_GRAPH_FUSION_SCHEMA_VERSION: u32 = 1;
/// Comparator identity shared by benchmark and release evidence.
pub const VECTOR_GRAPH_FUSION_COMPARATOR: &str = "vector-graph-fusion:v1";
/// Metric family used by this first fusion contract.
pub const VECTOR_GRAPH_FUSION_METRIC_FAMILY: &str = "l2-squared-top-k";
/// Release leaderboard artifact this contract links to.
pub const VECTOR_GRAPH_FUSION_FRONTIER_LEADERBOARD: &str =
    "release/evidence/benchmarks/frontier-leaderboard.json";

/// One deterministic top-k row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VectorGraphTopKEntry {
    /// Vector row / graph node id.
    pub node_id: u32,
    /// Squared L2 distance encoded with [`f32::to_bits`].
    pub distance_bits: u32,
}

impl VectorGraphTopKEntry {
    #[must_use]
    fn new(node_id: usize, distance: f32) -> Self {
        Self {
            node_id: node_id as u32,
            distance_bits: distance.to_bits(),
        }
    }

    /// Decode the squared L2 distance.
    #[must_use]
    pub fn distance(&self) -> f32 {
        f32::from_bits(self.distance_bits)
    }
}

/// Evidence bundle for one vector-to-graph fusion proof.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VectorGraphFusionEvidence {
    /// Schema version.
    pub schema_version: u32,
    /// Comparator identity.
    pub comparator: String,
    /// Dataset identity supplied by the caller.
    pub dataset_id: String,
    /// Metric family.
    pub metric_family: String,
    /// Release floor identity supplied by the caller.
    pub release_floor: String,
    /// Failure mode identity supplied by the caller.
    pub failure_mode: String,
    /// Frontier leaderboard artifact linkage.
    pub frontier_leaderboard_artifact: String,
    /// Number of vector rows / graph nodes.
    pub node_count: u32,
    /// Vector dimension.
    pub dimension: u32,
    /// Neighbor count per CSR row.
    pub neighbor_k: u32,
    /// Query ranking size.
    pub rank_k: u32,
    /// CSR row offsets for the generated neighbor graph.
    pub csr_offsets: Vec<u32>,
    /// CSR target nodes for the generated neighbor graph.
    pub csr_targets: Vec<u32>,
    /// Direct vector top-k by squared L2 distance.
    pub direct_top_k: Vec<VectorGraphTopKEntry>,
    /// Top-k after traversing the neighbor graph from the query-nearest seed.
    pub graph_traversal_top_k: Vec<VectorGraphTopKEntry>,
    /// Number of graph nodes reached from the query-nearest seed.
    pub graph_reached_count: u32,
    /// Whether graph traversal reached every node.
    pub traversal_parity: bool,
    /// Whether graph traversal top-k exactly matches direct vector top-k.
    pub top_k_stable: bool,
    /// Blocking reasons for release evidence.
    pub blockers: Vec<String>,
}

/// Build vector-to-graph fusion evidence for one query.
///
/// # Errors
/// Returns an actionable error when input shape or metadata is invalid. Parity
/// failures are recorded in [`VectorGraphFusionEvidence::blockers`] instead of
/// failing construction so release artifacts can expose exact failure modes.
pub fn try_vector_graph_fusion_evidence(
    vectors: &[f32],
    dimension: usize,
    neighbor_k: usize,
    query: &[f32],
    rank_k: usize,
    dataset_id: impl Into<String>,
    release_floor: impl Into<String>,
    failure_mode: impl Into<String>,
) -> Result<VectorGraphFusionEvidence, String> {
    let dataset_id = dataset_id.into();
    let release_floor = release_floor.into();
    let failure_mode = failure_mode.into();
    validate_vector_graph_inputs(
        vectors,
        dimension,
        neighbor_k,
        query,
        rank_k,
        &dataset_id,
        &release_floor,
        &failure_mode,
    )?;
    let node_count = vectors.len() / dimension;
    let (csr_offsets, csr_targets) = build_knn_csr(vectors, dimension, neighbor_k);
    let direct_top_k = top_k_for_nodes(vectors, dimension, query, 0..node_count, rank_k);
    let reached = traverse_from_seed(
        direct_top_k[0].node_id as usize,
        node_count,
        &csr_offsets,
        &csr_targets,
    );
    let graph_nodes = reached
        .iter()
        .enumerate()
        .filter_map(|(node, reached)| reached.then_some(node));
    let graph_traversal_top_k = top_k_for_nodes(vectors, dimension, query, graph_nodes, rank_k);
    let graph_reached_count = reached.iter().filter(|seen| **seen).count();
    let traversal_parity = graph_reached_count == node_count;
    let top_k_stable = direct_top_k == graph_traversal_top_k;
    let mut blockers = Vec::new();
    if !traversal_parity {
        blockers.push(format!(
            "graph traversal reached {graph_reached_count}/{node_count} node(s); Fix: increase neighbor_k, add reciprocal edges, or shard the dataset before claiming graph-ranking parity."
        ));
    }
    if !top_k_stable {
        blockers.push(
            "graph traversal top-k differs from direct vector top-k; Fix: preserve candidate recall before using graph ranking evidence."
                .to_string(),
        );
    }
    Ok(VectorGraphFusionEvidence {
        schema_version: VECTOR_GRAPH_FUSION_SCHEMA_VERSION,
        comparator: VECTOR_GRAPH_FUSION_COMPARATOR.to_string(),
        dataset_id,
        metric_family: VECTOR_GRAPH_FUSION_METRIC_FAMILY.to_string(),
        release_floor,
        failure_mode,
        frontier_leaderboard_artifact: VECTOR_GRAPH_FUSION_FRONTIER_LEADERBOARD.to_string(),
        node_count: node_count as u32,
        dimension: dimension as u32,
        neighbor_k: neighbor_k as u32,
        rank_k: rank_k as u32,
        csr_offsets,
        csr_targets,
        direct_top_k,
        graph_traversal_top_k,
        graph_reached_count: graph_reached_count as u32,
        traversal_parity,
        top_k_stable,
        blockers,
    })
}

fn validate_vector_graph_inputs(
    vectors: &[f32],
    dimension: usize,
    neighbor_k: usize,
    query: &[f32],
    rank_k: usize,
    dataset_id: &str,
    release_floor: &str,
    failure_mode: &str,
) -> Result<(), String> {
    if dimension == 0 {
        return Err("Fix: vector graph fusion requires dimension > 0.".to_string());
    }
    if vectors.is_empty() {
        return Err("Fix: vector graph fusion requires at least two vector rows.".to_string());
    }
    if vectors.len() % dimension != 0 {
        return Err(format!(
            "Fix: vector graph fusion received {} scalar value(s), not divisible by dimension={dimension}.",
            vectors.len()
        ));
    }
    let node_count = vectors.len() / dimension;
    if node_count < 2 {
        return Err("Fix: vector graph fusion requires at least two vector rows.".to_string());
    }
    if node_count > u32::MAX as usize {
        return Err(format!(
            "Fix: vector graph fusion node_count={node_count} exceeds u32 graph ids; shard the dataset."
        ));
    }
    if dimension > u32::MAX as usize {
        return Err(format!(
            "Fix: vector graph fusion dimension={dimension} exceeds u32 evidence fields; shard the vectors."
        ));
    }
    if neighbor_k == 0 || neighbor_k >= node_count {
        return Err(format!(
            "Fix: vector graph fusion neighbor_k={neighbor_k} must be in 1..node_count for node_count={node_count}."
        ));
    }
    if rank_k == 0 || rank_k > node_count {
        return Err(format!(
            "Fix: vector graph fusion rank_k={rank_k} must be in 1..=node_count for node_count={node_count}."
        ));
    }
    if query.len() != dimension {
        return Err(format!(
            "Fix: vector graph fusion query has {} value(s), expected dimension={dimension}.",
            query.len()
        ));
    }
    if vectors.iter().chain(query.iter()).any(|value| !value.is_finite()) {
        return Err("Fix: vector graph fusion requires finite vector and query values.".to_string());
    }
    if dataset_id.trim().is_empty() {
        return Err("Fix: vector graph fusion dataset_id cannot be blank.".to_string());
    }
    if release_floor.trim().is_empty() {
        return Err("Fix: vector graph fusion release_floor cannot be blank.".to_string());
    }
    if failure_mode.trim().is_empty() {
        return Err("Fix: vector graph fusion failure_mode cannot be blank.".to_string());
    }
    Ok(())
}

fn build_knn_csr(vectors: &[f32], dimension: usize, neighbor_k: usize) -> (Vec<u32>, Vec<u32>) {
    let node_count = vectors.len() / dimension;
    let mut offsets = Vec::with_capacity(node_count + 1);
    let mut targets = Vec::with_capacity(node_count * neighbor_k);
    offsets.push(0);
    for src in 0..node_count {
        let mut candidates = (0..node_count)
            .filter(|dst| *dst != src)
            .map(|dst| (squared_l2(row(vectors, dimension, src), row(vectors, dimension, dst)), dst))
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| {
            left.0
                .total_cmp(&right.0)
                .then_with(|| left.1.cmp(&right.1))
        });
        targets.extend(candidates.into_iter().take(neighbor_k).map(|(_, dst)| dst as u32));
        offsets.push(targets.len() as u32);
    }
    (offsets, targets)
}

fn top_k_for_nodes<I>(
    vectors: &[f32],
    dimension: usize,
    query: &[f32],
    nodes: I,
    rank_k: usize,
) -> Vec<VectorGraphTopKEntry>
where
    I: IntoIterator<Item = usize>,
{
    let mut scored = nodes
        .into_iter()
        .map(|node| (squared_l2(row(vectors, dimension, node), query), node))
        .collect::<Vec<_>>();
    scored.sort_by(|left, right| {
        left.0
            .total_cmp(&right.0)
            .then_with(|| left.1.cmp(&right.1))
    });
    scored
        .into_iter()
        .take(rank_k)
        .map(|(distance, node)| VectorGraphTopKEntry::new(node, distance))
        .collect()
}

fn traverse_from_seed(
    seed: usize,
    node_count: usize,
    csr_offsets: &[u32],
    csr_targets: &[u32],
) -> Vec<bool> {
    let mut reached = vec![false; node_count];
    let mut queue = VecDeque::new();
    reached[seed] = true;
    queue.push_back(seed);
    while let Some(node) = queue.pop_front() {
        let start = csr_offsets[node] as usize;
        let end = csr_offsets[node + 1] as usize;
        for target in &csr_targets[start..end] {
            let target = *target as usize;
            if !reached[target] {
                reached[target] = true;
                queue.push_back(target);
            }
        }
    }
    reached
}

fn row(vectors: &[f32], dimension: usize, row: usize) -> &[f32] {
    let start = row * dimension;
    &vectors[start..start + dimension]
}

fn squared_l2(left: &[f32], right: &[f32]) -> f32 {
    left.iter()
        .zip(right)
        .map(|(left, right)| {
            let delta = *left - *right;
            delta * delta
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connected_neighbor_graph_preserves_direct_top_k() {
        let vectors = [0.0, 1.0, 2.0, 3.0, 4.0];
        let evidence = try_vector_graph_fusion_evidence(
            &vectors,
            1,
            2,
            &[0.1],
            3,
            "ann.line.connected",
            "release-floor:ann-vector",
            "graph-recall-regression",
        )
        .expect("Fix: connected vector graph fusion fixture should build.");

        assert!(evidence.blockers.is_empty(), "{evidence:#?}");
        assert!(evidence.traversal_parity);
        assert!(evidence.top_k_stable);
        assert_eq!(evidence.direct_top_k, evidence.graph_traversal_top_k);
        assert_eq!(evidence.direct_top_k[0].node_id, 0);
        assert_eq!(evidence.csr_offsets, vec![0, 2, 4, 6, 8, 10]);
        assert_eq!(
            evidence.frontier_leaderboard_artifact,
            VECTOR_GRAPH_FUSION_FRONTIER_LEADERBOARD
        );
    }

    #[test]
    fn disconnected_neighbor_graph_records_recall_blocker() {
        let vectors = [0.0, 1.0, 100.0, 101.0];
        let evidence = try_vector_graph_fusion_evidence(
            &vectors,
            1,
            1,
            &[0.0],
            2,
            "ann.line.disconnected",
            "release-floor:ann-vector",
            "graph-recall-regression",
        )
        .expect("Fix: disconnected vector graph fusion fixture should still build evidence.");

        assert!(!evidence.traversal_parity);
        assert!(evidence.top_k_stable);
        assert!(evidence
            .blockers
            .iter()
            .any(|blocker| blocker.contains("graph traversal reached 2/4")));
    }

    #[test]
    fn invalid_vector_shape_is_actionable() {
        let error = try_vector_graph_fusion_evidence(
            &[0.0, 1.0, 2.0],
            2,
            1,
            &[0.0, 1.0],
            1,
            "ann.bad",
            "release-floor:ann-vector",
            "shape-regression",
        )
        .expect_err("Fix: malformed vector shapes must be rejected.");

        assert!(error.contains("not divisible by dimension=2"));
    }
}
