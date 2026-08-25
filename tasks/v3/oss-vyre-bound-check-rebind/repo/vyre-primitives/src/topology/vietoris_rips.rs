//! Vietoris-Rips filtration boundary primitive (#15).
//!
//! Persistent homology computes topological features (connected
//! components, loops, voids) of point clouds across multiple scales.
//! The Vietoris-Rips (V-R) filtration builds simplicial complexes by
//! adding all simplices whose pairwise distances are below a
//! threshold ε. Recent work (Bauer 2021 Ripser++, Lewis 2024 chunked
//! GPU reduction) makes V-R practical at billions of simplices.
//!
//! This file ships the **edge filtration step** primitive  -  given
//! a pairwise-distance matrix and the current threshold ε, output a
//! sorted list of edges (pairs of vertices) whose distance ≤ ε. Edges
//! are encoded as `u32` packed `(u_vertex << 16) | v_vertex`.
//!
//! Composes with #1 semiring_gemm (boundary matrix products = MinPlus
//! semiring on the edge incidence matrix) for the chunk-reduction
//! step that extracts persistence pairs.
//!
//! The primitive stays domain-neutral: higher-level crates can compose
//! it into shape analysis, anomaly detection, persistent-landscape
//! features, or graph-signature pipelines without changing the
//! topology authority layer.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Op id.
pub const OP_ID: &str = "vyre-primitives::topology::vietoris_rips_edge_filter";

/// Emit the edge filter Program.
///
/// Inputs:
/// - `dist_matrix`: row-major `n × n` u32 (pairwise distances, 16.16
///   fp). Symmetric with zero diagonal.
/// - `epsilon`: 1-element u32  -  current scale.
///
/// Output:
/// - `edge_mask`: row-major `n × n` u32  -  `1` if (i, j) is an edge
///   at scale ε (i < j AND dist[i, j] ≤ ε), else `0`. Half of the
///   matrix (lower triangular) is zero by construction (i ≥ j).
///
/// Lane `t` = flattened (i, j) cell. Caller composes with stream-
/// compaction to extract the actual edge list.
#[must_use]
pub fn vietoris_rips_edge_filter(
    dist_matrix: &str,
    epsilon: &str,
    edge_mask: &str,
    n: u32,
) -> Program {
    if n == 0 {
        return crate::invalid_output_program(
            OP_ID,
            edge_mask,
            DataType::U32,
            format!("Fix: vietoris_rips_edge_filter requires n > 0, got {n}."),
        );
    }

    let cells = n * n;
    let t = Expr::InvocationId { axis: 0 };
    let i_expr = Expr::div(t.clone(), Expr::u32(n));
    let j_expr = Expr::rem(t.clone(), Expr::u32(n));

    // Edge mask: 1 iff (i < j) AND (dist[i, j] ≤ ε)
    let in_upper = Expr::lt(i_expr, j_expr);
    let in_eps = Expr::le(
        Expr::load(dist_matrix, t.clone()),
        Expr::load(epsilon, Expr::u32(0)),
    );
    let value = Expr::select(Expr::and(in_upper, in_eps), Expr::u32(1), Expr::u32(0));

    let body = vec![Node::if_then(
        Expr::lt(t.clone(), Expr::u32(cells)),
        vec![Node::store(edge_mask, t, value)],
    )];

    Program::wrapped(
        vec![
            BufferDecl::storage(dist_matrix, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(cells),
            BufferDecl::storage(epsilon, 1, BufferAccess::ReadOnly, DataType::U32).with_count(1),
            BufferDecl::storage(edge_mask, 2, BufferAccess::ReadWrite, DataType::U32)
                .with_count(cells),
        ],
        [256, 1, 1],
        vec![Node::Region {
            generator: Ident::from(OP_ID),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}

/// CPU reference: emit edge mask as a flat row-major `n × n` array.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn vietoris_rips_edge_filter_cpu(dist_matrix: &[f64], epsilon: f64, n: u32) -> Vec<u32> {
    let mut out = Vec::new();
    match try_vietoris_rips_edge_filter_cpu_into(dist_matrix, epsilon, n, &mut out) {
        Ok(()) => out,
        // A parity oracle that returns empty on failure makes the GPU-vs-CPU
        // assertion pass on empty==empty, silently masking a divergence
        // (Law 10 / Law 6). Fail loud; callers use the try_ variant.
        Err(error) => {
            panic!("vyre-primitives Vietoris-Rips edge filter CPU reference failed: {error}")
        }
    }
}

/// Fallible CPU reference using a caller-owned output buffer.
///
/// Missing distance entries are treated as `INFINITY`, matching the
/// owned CPU helper. `out` is not mutated until all required storage is
/// reserved.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_vietoris_rips_edge_filter_cpu_into(
    dist_matrix: &[f64],
    epsilon: f64,
    n: u32,
    out: &mut Vec<u32>,
) -> Result<(), String> {
    let n = usize::try_from(n).map_err(|_| "n does not fit usize".to_string())?;
    let cells = n
        .checked_mul(n)
        .ok_or_else(|| format!("n * n overflows usize for n={n}"))?;
    let additional = cells.saturating_sub(out.capacity());
    out.try_reserve_exact(additional)
        .map_err(|err| format!("failed to reserve edge mask: {err}"))?;
    out.clear();
    out.resize(cells, 0);
    for i in 0..n {
        for j in 0..n {
            let addr = i * n + j;
            if i < j && dist_matrix.get(addr).copied().unwrap_or(f64::INFINITY) <= epsilon {
                out[addr] = 1;
            }
        }
    }
    Ok(())
}

/// CPU helper: extract the edge list from a mask. Returns
/// `Vec<(u_vertex, v_vertex)>`.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn extract_edges_cpu(edge_mask: &[u32], n: u32) -> Vec<(u32, u32)> {
    let mut edges = Vec::new();
    match try_extract_edges_cpu_into(edge_mask, n, &mut edges) {
        Ok(()) => edges,
        // A parity oracle that returns empty on failure makes the GPU-vs-CPU
        // assertion pass on empty==empty, silently masking a divergence
        // (Law 10 / Law 6). Fail loud; callers use the try_ variant.
        Err(error) => {
            panic!("vyre-primitives Vietoris-Rips edge extraction CPU reference failed: {error}")
        }
    }
}

/// Fallible CPU helper that extracts edges into a caller-owned vector.
///
/// The function pre-counts edges so `edges` is not mutated if reserve
/// fails.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_extract_edges_cpu_into(
    edge_mask: &[u32],
    n: u32,
    edges: &mut Vec<(u32, u32)>,
) -> Result<(), String> {
    let n = usize::try_from(n).map_err(|_| "n does not fit usize".to_string())?;
    let mut edge_count = 0_usize;
    for i in 0..n {
        for j in (i + 1)..n {
            if edge_mask.get(i * n + j).copied().unwrap_or(0) != 0 {
                edge_count = edge_count
                    .checked_add(1)
                    .ok_or_else(|| "edge count overflows usize".to_string())?;
            }
        }
    }
    let additional = edge_count.saturating_sub(edges.capacity());
    edges
        .try_reserve_exact(additional)
        .map_err(|err| format!("failed to reserve edge list: {err}"))?;
    edges.clear();
    for i in 0..n {
        for j in (i + 1)..n {
            if edge_mask.get(i * n + j).copied().unwrap_or(0) != 0 {
                edges.push((i as u32, j as u32));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_edge_filter_full_threshold_includes_all() {
        // 3 points at unit triangle: dist 1.0 between all pairs.
        let d = vec![0.0, 1.0, 1.0, 1.0, 0.0, 1.0, 1.0, 1.0, 0.0];
        let mask = vietoris_rips_edge_filter_cpu(&d, 1.0, 3);
        // Upper triangular pairs: (0,1), (0,2), (1,2) all included.
        assert_eq!(mask[0 * 3 + 1], 1);
        assert_eq!(mask[0 * 3 + 2], 1);
        assert_eq!(mask[1 * 3 + 2], 1);
        // Diagonal + lower triangular zeros.
        for i in 0..3 {
            assert_eq!(mask[i * 3 + i], 0);
        }
        assert_eq!(mask[1 * 3 + 0], 0);
        assert_eq!(mask[2 * 3 + 0], 0);
        assert_eq!(mask[2 * 3 + 1], 0);
    }

    #[test]
    fn cpu_edge_filter_low_threshold_excludes() {
        let d = vec![0.0, 1.0, 5.0, 1.0, 0.0, 5.0, 5.0, 5.0, 0.0];
        let mask = vietoris_rips_edge_filter_cpu(&d, 2.0, 3);
        assert_eq!(mask[0 * 3 + 1], 1); // dist 1, included
        assert_eq!(mask[0 * 3 + 2], 0); // dist 5, excluded
        assert_eq!(mask[1 * 3 + 2], 0);
    }

    #[test]
    fn cpu_extract_edges_returns_correct_pairs() {
        let mask = vec![0u32, 1, 0, 0, 0, 1, 0, 0, 0];
        let edges = extract_edges_cpu(&mask, 3);
        assert_eq!(edges, vec![(0, 1), (1, 2)]);
    }

    #[test]
    fn cpu_edge_filter_into_reuses_output_and_clears_tail() {
        let d = vec![0.0, 1.0, 5.0, 1.0, 0.0, 5.0, 5.0, 5.0, 0.0];
        let mut out = Vec::with_capacity(16);
        out.extend([99; 16]);
        let ptr = out.as_ptr();
        try_vietoris_rips_edge_filter_cpu_into(&d, 2.0, 3, &mut out).unwrap();
        assert_eq!(out, vec![0, 1, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(out.as_ptr(), ptr);
    }

    #[test]
    fn cpu_extract_edges_into_reuses_output_and_clears_tail() {
        let mask = vec![0u32, 1, 0, 0, 0, 1, 0, 0, 0];
        let mut edges = Vec::with_capacity(8);
        edges.extend([(9, 9); 8]);
        let ptr = edges.as_ptr();
        try_extract_edges_cpu_into(&mask, 3, &mut edges).unwrap();
        assert_eq!(edges, vec![(0, 1), (1, 2)]);
        assert_eq!(edges.as_ptr(), ptr);
    }

    #[test]
    fn compatibility_wrappers_match_fallible_references() {
        let d = vec![0.0, 1.0, 5.0, 1.0, 0.0, 5.0, 5.0, 5.0, 0.0];
        let mut mask = Vec::with_capacity(16);
        try_vietoris_rips_edge_filter_cpu_into(&d, 2.0, 3, &mut mask)
            .expect("Fix: small edge mask CPU reference must reserve");
        assert_eq!(vietoris_rips_edge_filter_cpu(&d, 2.0, 3), mask);

        let mut edges = Vec::with_capacity(8);
        try_extract_edges_cpu_into(&mask, 3, &mut edges)
            .expect("Fix: small edge extraction CPU reference must reserve");
        assert_eq!(extract_edges_cpu(&mask, 3), edges);
    }

    #[test]
    fn production_wrappers_have_no_raw_panic_path() {
        let production = include_str!("vietoris_rips.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("Fix: vietoris_rips.rs must contain production section");

        assert!(
            !production.contains(".expect(") && !production.contains(".unwrap("),
            "Fix: Vietoris-Rips CPU reference wrappers must not panic in production."
        );
    }

    #[test]
    fn cpu_short_buffers_treat_missing_entries_as_absent() {
        let mask = vietoris_rips_edge_filter_cpu(&[0.0, 0.5], 1.0, 2);
        assert_eq!(mask, vec![0, 1, 0, 0]);

        let edges = extract_edges_cpu(&[0, 1], 2);
        assert_eq!(edges, vec![(0, 1)]);
        assert!(extract_edges_cpu(&[0], 2).is_empty());
    }

    #[test]
    fn cpu_zero_threshold_no_edges() {
        let d = vec![0.0, 0.5, 0.5, 0.5, 0.0, 0.5, 0.5, 0.5, 0.0];
        let mask = vietoris_rips_edge_filter_cpu(&d, 0.0, 3);
        for v in mask {
            assert_eq!(v, 0);
        }
    }

    #[test]
    fn cpu_filtration_grows_monotonically() {
        // As ε increases, the number of edges only grows.
        let d = vec![0.0, 1.0, 3.0, 1.0, 0.0, 2.0, 3.0, 2.0, 0.0];
        let edges_eps1 = extract_edges_cpu(&vietoris_rips_edge_filter_cpu(&d, 1.0, 3), 3);
        let edges_eps2 = extract_edges_cpu(&vietoris_rips_edge_filter_cpu(&d, 2.0, 3), 3);
        let edges_eps3 = extract_edges_cpu(&vietoris_rips_edge_filter_cpu(&d, 3.0, 3), 3);
        assert!(edges_eps1.len() <= edges_eps2.len());
        assert!(edges_eps2.len() <= edges_eps3.len());
        assert_eq!(edges_eps3.len(), 3); // all 3 upper-tri edges
    }

    #[test]
    fn ir_program_buffer_layout() {
        let p = vietoris_rips_edge_filter("d", "e", "m", 4);
        assert_eq!(p.workgroup_size, [256, 1, 1]);
        let names: Vec<&str> = p.buffers.iter().map(|b| b.name()).collect();
        assert_eq!(names, vec!["d", "e", "m"]);
        assert_eq!(p.buffers[0].count(), 16);
        assert_eq!(p.buffers[1].count(), 1);
        assert_eq!(p.buffers[2].count(), 16);
    }

    #[test]
    fn zero_n_traps() {
        let p = vietoris_rips_edge_filter("d", "e", "m", 0);
        assert!(p.stats().trap());
    }
}
