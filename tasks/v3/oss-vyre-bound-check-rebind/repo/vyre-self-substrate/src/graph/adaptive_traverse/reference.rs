use vyre_primitives::graph::adaptive_traverse::{
    adaptive_frontier_popcount_in_domain,
    cpu_sparse_dense_step as reference_adaptive_sparse_dense_step,
    validate_adaptive_frontier as primitive_validate_adaptive_frontier,
};

/// CPU reference for one adaptive sparse/dense graph step.
///
/// # Errors
///
/// Returns primitive frontier-shape or popcount diagnostics instead of
/// panicking; self-substrate is only the dispatch/scratch consumer here, so the
/// primitive remains the authority for traversal validity.
#[allow(clippy::too_many_arguments)]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn adaptive_traverse_step(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    adj_rows_dense: &[u32],
    frontier_in: &[u32],
    allow_mask: u32,
    dense_threshold_pct: u32,
) -> Result<Vec<u32>, String> {
    primitive_validate_adaptive_frontier(node_count, frontier_in)?;
    let frontier_popcount =
        adaptive_frontier_popcount_in_domain(node_count, frontier_in, "adaptive_traverse_step")?;
    Ok(reference_adaptive_sparse_dense_step(
        frontier_in,
        frontier_popcount,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        adj_rows_dense,
        node_count,
        allow_mask,
        dense_threshold_pct,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adaptive_traverse_step_rejects_frontier_shape_without_panicking() {
        let err = adaptive_traverse_step(2, &[0, 0], &[], &[], &[0, 0], &[], 1, 100)
            .expect_err("Fix: malformed frontier shape must be rejected.");

        assert!(
            err.contains("frontier expected 1 word"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn adaptive_traverse_step_delegates_sparse_reference_to_primitive() {
        let out = adaptive_traverse_step(2, &[0, 1, 1], &[1], &[1], &[0, 1], &[1], 1, 100)
            .expect("Fix: valid two-node sparse traversal must succeed.");

        assert_eq!(out, vec![0b10]);
    }
}
