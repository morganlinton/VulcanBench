//! Region-tree loop topology via #15 vietoris_rips (#15 self-consumer).
//!
//! Closes the recursion thesis for #15  -  vietoris_rips edge filtering
//! ships to user dialects (cosmology, biological networks, mesh
//! topology) AND extracts vyre's loop-nest topological signatures
//! for fusion-vs-fission scheduling.
//!
//! # The self-use
//!
//! Vyre's optimizer chooses between **loop fusion** (merge two
//! adjacent loops into one) and **loop fission** (split one loop
//! into two), driven by data-locality + register-pressure heuristics.
//! These heuristics are local  -  they don't see the full topology of
//! a loop nest. Persistent homology DOES see it: the H₁ persistence
//! diagram of the Region-tree filtration encodes how nested loops
//! merge as the "scale" parameter ε grows.
//!
//! H₁ persistent features (loops born early, die late) → big nested
//! loops worth fusing across.
//! H₁ transient features (loops born late, die early) → tight
//! locality-coherent loops worth fissioning.
//!
//! Vietoris-Rips edge filtering at scale ε is the first step of the
//! persistent homology computation  -  extract the 1-skeleton of the
//! filtration, then count cycles per ε.
//!
//! # Algorithm
//!
//! ```text
//! 1. compute pairwise Region-distance matrix d(i, j)
//!    (e.g. shared-buffer-set Jaccard distance)
//! 2. for each ε in [ε_min, ε_max]:
//!    - vietoris_rips_edge_filter(d, ε) → edge mask
//!    - count cycles in (V, edge_mask) → β₁(ε)
//! 3. persistent features = pairs (born, died) over the ε sequence
//! ```
//!
//! Per-ε cycle counting consumes
//! [`vyre_primitives::topology::betti_persistence::betti_persistence_cpu`]:
//! the 1-skeleton's union-find pass returns `(b0, b1, edges)` and lets
//! the optimizer track how many independent cycles persist as ε grows.
//!
//! # Why this matters
//!
//! Loop-nest topology is the substrate decision for ANY
//! cache-aware loop optimizer. Vyre is the first GPU substrate to
//! compute it via persistent homology.

use crate::dispatch_buffers::{
    ceil_div_u32, checked_square_cells, decode_u32_output_exact, u32_slice_to_le_bytes,
};
#[cfg(any(test, feature = "cpu-parity"))]
use crate::hardware::scratch::reserve_vec_capacity_or_panic;
use crate::optimizer::dispatcher::{DispatchError, OptimizerDispatcher};
#[cfg(any(test, feature = "cpu-parity"))]
use vyre_primitives::topology::vietoris_rips::extract_edges_cpu;
use vyre_primitives::topology::vietoris_rips::vietoris_rips_edge_filter;

/// Reusable buffers for loop-topology filtration sweeps.
#[derive(Debug, Default)]
#[cfg(any(test, feature = "cpu-parity"))]
pub struct LoopTopologyScratch {
    mask: Vec<u32>,
    parent: Vec<u32>,
    rank: Vec<u32>,
}

/// Compute the Vietoris-Rips 1-skeleton at scale `epsilon` over the
/// Region-distance matrix. Returns the edge mask.
///
/// # Panics
///
/// Panics if `dist_matrix.len() != n*n`.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_region_loop_skeleton(dist_matrix: &[f64], epsilon: f64, n: u32) -> Vec<u32> {
    let mut out = Vec::new();
    reference_region_loop_skeleton_into(dist_matrix, epsilon, n, &mut out);
    out
}

/// Compute the Vietoris-Rips 1-skeleton into caller-owned storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_region_loop_skeleton_into(
    dist_matrix: &[f64],
    epsilon: f64,
    n: u32,
    out: &mut Vec<u32>,
) {
    use crate::observability::{bump, persistent_homology_loop_signature_calls};
    bump(&persistent_homology_loop_signature_calls);
    let n_us = n as usize;
    assert_eq!(dist_matrix.len(), n_us * n_us);
    out.clear();
    out.resize(n_us * n_us, 0);
    for i in 0..n_us {
        for j in (i + 1)..n_us {
            if dist_matrix[i * n_us + j] <= epsilon {
                out[i * n_us + j] = 1;
                out[j * n_us + i] = 1;
            }
        }
    }
}

/// Compute the Vietoris-Rips 1-skeleton through the dispatcher using
/// fixed-point 16.16 distances. This is the production path for callers
/// that already keep topology features in primitive-native storage.
///
/// # Errors
///
/// Returns [`DispatchError`] when the shape is invalid, the backend rejects
/// the primitive, or the backend returns a malformed edge-mask buffer.
pub fn region_loop_skeleton_fixed_via(
    dispatcher: &impl OptimizerDispatcher,
    dist_matrix_fixed: &[u32],
    epsilon_fixed: u32,
    n: u32,
) -> Result<Vec<u32>, DispatchError> {
    let mut out = Vec::new();
    region_loop_skeleton_fixed_via_into(dispatcher, dist_matrix_fixed, epsilon_fixed, n, &mut out)?;
    Ok(out)
}

/// Compute the fixed-point Vietoris-Rips 1-skeleton into caller-owned
/// storage without materializing an intermediate host-side mask.
///
/// # Errors
///
/// Returns [`DispatchError`] when input or backend output violates the
/// primitive contract.
pub fn region_loop_skeleton_fixed_via_into(
    dispatcher: &impl OptimizerDispatcher,
    dist_matrix_fixed: &[u32],
    epsilon_fixed: u32,
    n: u32,
    out: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    use crate::observability::{bump, persistent_homology_loop_signature_calls};
    bump(&persistent_homology_loop_signature_calls);

    let cells = checked_square_cells(n, "region_loop_skeleton_fixed_via")?;
    if dist_matrix_fixed.len() != cells {
        return Err(DispatchError::BadInputs(format!(
            "Fix: region_loop_skeleton_fixed_via requires dist_matrix_fixed.len() == n*n, got len={}, n={}, n*n={cells}.",
            dist_matrix_fixed.len(),
            n
        )));
    }

    let program = vietoris_rips_edge_filter("dist_matrix", "epsilon", "edge_mask", n);
    let cells_u32 = u32::try_from(cells).map_err(|_| {
        DispatchError::BadInputs(format!(
            "Fix: region_loop_skeleton_fixed_via n*n exceeds the primitive u32 lane limit for n={n}."
        ))
    })?;
    let grid = Some([ceil_div_u32(cells_u32, 256), 1, 1]);
    let outputs = dispatcher.dispatch(
        &program,
        &[
            u32_slice_to_le_bytes(dist_matrix_fixed),
            epsilon_fixed.to_le_bytes().to_vec(),
        ],
        grid,
    )?;

    if outputs.is_empty() {
        return Err(DispatchError::BackendError(format!(
            "Fix: region_loop_skeleton_fixed_via expected exactly one edge-mask output, got {}.",
            outputs.len()
        )));
    }
    decode_u32_output_exact(&outputs[0], cells, "region_loop_skeleton_fixed_via", out)
}

/// Convenience: extract the edge list of the 1-skeleton.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_region_loop_edges(dist_matrix: &[f64], epsilon: f64, n: u32) -> Vec<(u32, u32)> {
    let mask = reference_region_loop_skeleton(dist_matrix, epsilon, n);
    extract_edges_cpu(&mask, n)
}

/// Sweep over a range of ε scales and return the edge count at
/// each scale.
///
/// `epsilons` is a sorted-ascending sequence of scale parameters.
/// Returns one edge-count per ε.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_loop_filtration_edge_counts(
    dist_matrix: &[f64],
    epsilons: &[f64],
    n: u32,
) -> Vec<u32> {
    let mut scratch = LoopTopologyScratch::default();
    let mut out = Vec::with_capacity(epsilons.len());
    reference_loop_filtration_edge_counts_into(dist_matrix, epsilons, n, &mut scratch, &mut out);
    out
}

/// Sweep over ε scales into caller-owned output.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_loop_filtration_edge_counts_into(
    dist_matrix: &[f64],
    epsilons: &[f64],
    n: u32,
    scratch: &mut LoopTopologyScratch,
    out: &mut Vec<u32>,
) {
    out.clear();
    reserve_vec_capacity_or_panic(out, epsilons.len(), "loop filtration edge-count output");
    for &eps in epsilons {
        reference_region_loop_skeleton_into(dist_matrix, eps, n, &mut scratch.mask);
        out.push(count_upper_triangle_edges(&scratch.mask, n));
    }
}

#[cfg(any(test, feature = "cpu-parity"))]
fn count_upper_triangle_edges(mask: &[u32], n: u32) -> u32 {
    let n_us = n as usize;
    let mut edges = 0u32;
    for i in 0..n_us {
        for j in (i + 1)..n_us {
            if mask[i * n_us + j] != 0 {
                edges = edges.saturating_add(1);
            }
        }
    }
    edges
}

/// Sweep over `epsilons` and return `(b0, b1)`  -  connected components
/// and independent-cycle count  -  at each scale.
///
/// `b1` rises every time a new loop closes in the 1-skeleton; that
/// jump is exactly an H₁ persistent feature being born. The optimizer
/// uses these jumps to detect loop-nest topology that the local
/// fusion/fission heuristic doesn't see.
///
/// Composes [`reference_region_loop_skeleton`] (Vietoris-Rips edge filter) with
/// `betti_persistence_cpu` (union-find cycle count).
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_loop_filtration_betti(
    dist_matrix: &[f64],
    epsilons: &[f64],
    n: u32,
) -> Vec<(u32, u32)> {
    let mut scratch = LoopTopologyScratch::default();
    let mut out = Vec::with_capacity(epsilons.len());
    reference_loop_filtration_betti_into(dist_matrix, epsilons, n, &mut scratch, &mut out);
    out
}

/// Sweep over ε scales and write `(b0, b1)` into caller-owned output.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_loop_filtration_betti_into(
    dist_matrix: &[f64],
    epsilons: &[f64],
    n: u32,
    scratch: &mut LoopTopologyScratch,
    out: &mut Vec<(u32, u32)>,
) {
    out.clear();
    reserve_vec_capacity_or_panic(out, epsilons.len(), "loop filtration Betti output");
    for &eps in epsilons {
        reference_region_loop_skeleton_into(dist_matrix, eps, n, &mut scratch.mask);
        let (b0, b1, _edges) =
            betti_persistence_into(&scratch.mask, n, &mut scratch.parent, &mut scratch.rank);
        out.push((b0, b1));
    }
}

/// Find every ε at which a new H₁ feature is born  -  i.e. an ε where
/// the cycle count `b1` strictly increases over the previous ε.
/// Returns the sequence of `(epsilon, b1_after)` pairs.
///
/// These are the loop-nest "scale signatures" the optimizer fuses on:
/// a small ε with sudden b1 jump = tightly coupled loops worth fusing;
/// a large ε with no b1 change = independent loops worth fissioning.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_h1_birth_scales(dist_matrix: &[f64], epsilons: &[f64], n: u32) -> Vec<(f64, u32)> {
    let mut scratch = LoopTopologyScratch::default();
    let mut births = Vec::new();
    reference_h1_birth_scales_into(dist_matrix, epsilons, n, &mut scratch, &mut births);
    births
}

/// Find H1 birth scales into caller-owned output without materializing
/// the full Betti series.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_h1_birth_scales_into(
    dist_matrix: &[f64],
    epsilons: &[f64],
    n: u32,
    scratch: &mut LoopTopologyScratch,
    births: &mut Vec<(f64, u32)>,
) {
    let mut prev_b1 = 0u32;
    births.clear();
    for &eps in epsilons {
        reference_region_loop_skeleton_into(dist_matrix, eps, n, &mut scratch.mask);
        let (_b0, b1, _edges) =
            betti_persistence_into(&scratch.mask, n, &mut scratch.parent, &mut scratch.rank);
        if b1 > prev_b1 {
            births.push((eps, b1));
        }
        prev_b1 = b1;
    }
}

#[cfg(any(test, feature = "cpu-parity"))]
fn betti_persistence_into(
    mask: &[u32],
    n: u32,
    parent: &mut Vec<u32>,
    rank: &mut Vec<u32>,
) -> (u32, u32, u32) {
    let n_us = n as usize;
    assert_eq!(
        mask.len(),
        n_us * n_us,
        "Fix: betti_persistence requires mask of length n*n."
    );
    if n == 0 {
        parent.clear();
        rank.clear();
        return (0, 0, 0);
    }

    parent.clear();
    parent.extend(0..n);
    rank.clear();
    rank.resize(n_us, 0);

    let mut edges: u32 = 0;
    let mut tree_edges: u32 = 0;
    for i in 0..n_us {
        for j in (i + 1)..n_us {
            if mask[i * n_us + j] == 0 {
                continue;
            }
            edges = edges.saturating_add(1);
            if union(parent, rank, i as u32, j as u32) {
                tree_edges = tree_edges.saturating_add(1);
            }
        }
    }

    let mut b0 = 0u32;
    for v in 0..n {
        if find(parent, v) == v {
            b0 = b0.saturating_add(1);
        }
    }
    (b0, edges - tree_edges, edges)
}

#[cfg(any(test, feature = "cpu-parity"))]
fn find(parent: &mut [u32], mut x: u32) -> u32 {
    while parent[x as usize] != x {
        let p = parent[x as usize];
        parent[x as usize] = parent[p as usize];
        x = parent[x as usize];
    }
    x
}

#[cfg(any(test, feature = "cpu-parity"))]
fn union(parent: &mut [u32], rank: &mut [u32], a: u32, b: u32) -> bool {
    let ra = find(parent, a);
    let rb = find(parent, b);
    if ra == rb {
        return false;
    }
    let (ra_rank, rb_rank) = (rank[ra as usize], rank[rb as usize]);
    match ra_rank.cmp(&rb_rank) {
        std::cmp::Ordering::Less => parent[ra as usize] = rb,
        std::cmp::Ordering::Greater => parent[rb as usize] = ra,
        std::cmp::Ordering::Equal => {
            parent[rb as usize] = ra;
            rank[ra as usize] = ra_rank + 1;
        }
    }
    true
}

#[cfg(test)]
mod fixed_via_tests {
    use super::*;
    use vyre_foundation::ir::Program;

    struct SkeletonDispatcher;

    impl OptimizerDispatcher for SkeletonDispatcher {
        fn dispatch(
            &self,
            _program: &Program,
            inputs: &[Vec<u8>],
            grid_override: Option<[u32; 3]>,
        ) -> Result<Vec<Vec<u8>>, DispatchError> {
            assert_eq!(grid_override, Some([1, 1, 1]));
            assert_eq!(inputs.len(), 2);
            let dist = crate::hardware::dispatch_buffers::read_u32s(&inputs[0]);
            let epsilon = crate::hardware::dispatch_buffers::read_u32s(&inputs[1])[0];
            let n = integer_sqrt(dist.len());
            let mut mask = vec![0u32; dist.len()];
            for i in 0..n {
                for j in (i + 1)..n {
                    let idx = i * n + j;
                    if dist[idx] <= epsilon {
                        mask[idx] = 1;
                    }
                }
            }
            Ok(vec![u32_slice_to_le_bytes(&mask)])
        }
    }

    #[test]
    fn fixed_via_dispatches_vietoris_rips_mask() {
        let dist = vec![0, 10, 30, 10, 0, 20, 30, 20, 0];
        let mask = region_loop_skeleton_fixed_via(&SkeletonDispatcher, &dist, 20, 3).unwrap();
        assert_eq!(mask, vec![0, 1, 0, 0, 0, 1, 0, 0, 0]);
    }

    #[test]
    fn fixed_via_rejects_bad_matrix_shape() {
        let err =
            region_loop_skeleton_fixed_via(&SkeletonDispatcher, &[0, 1, 2], 1, 2).unwrap_err();
        assert!(matches!(err, DispatchError::BadInputs(_)));
    }

    fn integer_sqrt(n: usize) -> usize {
        let mut root = 0usize;
        while root * root < n {
            root += 1;
        }
        root
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_primitives::topology::betti_persistence::betti_persistence_cpu;

    #[test]

    fn empty_skeleton_below_threshold() {
        // 2 nodes at distance 1.0; ε = 0.5 yields no edges.
        let dist = vec![0.0, 1.0, 1.0, 0.0];
        let mask = reference_region_loop_skeleton(&dist, 0.5, 2);
        assert!(mask.iter().all(|&v| v == 0));
    }

    #[test]
    fn full_skeleton_above_threshold() {
        // 3 nodes, all at distance 0.5 from each other.
        let dist = vec![0.0, 0.5, 0.5, 0.5, 0.0, 0.5, 0.5, 0.5, 0.0];
        let mask = reference_region_loop_skeleton(&dist, 0.6, 3);
        // 3 edges in upper triangle: (0,1), (0,2), (1,2).
        let count = count_upper_triangle_edges(&mask, 3);
        assert_eq!(count, 3);
    }

    #[test]
    fn edges_extracted_in_canonical_order() {
        let dist = vec![0.0, 0.3, 0.7, 0.3, 0.0, 0.4, 0.7, 0.4, 0.0];
        let edges = reference_region_loop_edges(&dist, 0.5, 3);
        // Distances ≤ 0.5: (0,1) at 0.3; (1,2) at 0.4. (0,2) excluded.
        assert!(edges.contains(&(0, 1)));
        assert!(edges.contains(&(1, 2)));
        assert!(!edges.contains(&(0, 2)));
    }

    #[test]
    fn filtration_edge_counts_monotone_increasing() {
        // As ε grows, edge count should be non-decreasing.
        let dist = vec![0.0, 0.1, 0.5, 0.1, 0.0, 0.2, 0.5, 0.2, 0.0];
        let epsilons = vec![0.05, 0.15, 0.25, 0.6];
        let counts = reference_loop_filtration_edge_counts(&dist, &epsilons, 3);
        for w in counts.windows(2) {
            assert!(
                w[0] <= w[1],
                "edge counts must be monotone over ε filtration"
            );
        }
        // Final ε should reach 3 edges.
        assert_eq!(counts[3], 3);
    }

    #[test]
    fn singleton_dist_yields_no_edges() {
        let dist = vec![0.0];
        let mask = reference_region_loop_skeleton(&dist, 1.0, 1);
        assert!(mask.iter().all(|&v| v == 0));
    }

    // ---- betti consumer ----

    #[test]
    fn betti_filtration_below_threshold_no_cycles() {
        // 3 nodes far apart; ε small → no edges, b0=3, b1=0.
        let dist = vec![0.0, 1.0, 1.0, 1.0, 0.0, 1.0, 1.0, 1.0, 0.0];
        let series = reference_loop_filtration_betti(&dist, &[0.5], 3);
        assert_eq!(series, vec![(3, 0)]);
    }

    #[test]
    fn betti_filtration_triangle_has_b1_one() {
        // 3 nodes in equilateral triangle; ε large → triangle 1-skeleton.
        let dist = vec![0.0, 0.5, 0.5, 0.5, 0.0, 0.5, 0.5, 0.5, 0.0];
        let series = reference_loop_filtration_betti(&dist, &[0.6], 3);
        // 3 edges, 1 component, 1 cycle.
        assert_eq!(series, vec![(1, 1)]);
    }

    #[test]
    fn betti_filtration_b1_monotone_non_decreasing_on_growing_filtration() {
        // 4 nodes; distances chosen so adding edges never breaks cycles.
        // (Edges only added; an existing cycle persists in a 1-skeleton.)
        let dist = vec![
            0.0, 0.1, 0.2, 0.3, // 0 -> {1,2,3}
            0.1, 0.0, 0.4, 0.5, // 1 -> {0,2,3}
            0.2, 0.4, 0.0, 0.6, // 2 -> {0,1,3}
            0.3, 0.5, 0.6, 0.0, // 3 -> {0,1,2}
        ];
        let epsilons = vec![0.05, 0.15, 0.25, 0.35, 0.45, 0.55, 0.65];
        let series = reference_loop_filtration_betti(&dist, &epsilons, 4);
        for w in series.windows(2) {
            assert!(
                w[0].1 <= w[1].1,
                "b1 must be non-decreasing across a growing filtration; got {:?}",
                series
            );
        }
        // Final ε engulfs every pair: K4 has b1 = 3.
        assert_eq!(series.last().unwrap().1, 3);
    }

    #[test]
    fn betti_h1_birth_scales_pinpoints_first_cycle() {
        // 3 nodes; distances 0.1 (0,1), 0.2 (0,2), 0.3 (1,2).
        // ε=0.15 → only (0,1) edge → b1=0.
        // ε=0.25 → (0,1)+(0,2) edges → b1=0 (tree).
        // ε=0.35 → all three edges → b1=1 (triangle).
        let dist = vec![0.0, 0.1, 0.2, 0.1, 0.0, 0.3, 0.2, 0.3, 0.0];
        let epsilons = vec![0.15, 0.25, 0.35];
        let births = reference_h1_birth_scales(&dist, &epsilons, 3);
        assert_eq!(births, vec![(0.35, 1)]);
    }

    #[test]
    fn filtration_into_paths_match_owned_helpers() {
        let dist = vec![0.0, 0.1, 0.2, 0.1, 0.0, 0.3, 0.2, 0.3, 0.0];
        let epsilons = vec![0.15, 0.25, 0.35];
        let mut scratch = LoopTopologyScratch::default();

        let owned_counts = reference_loop_filtration_edge_counts(&dist, &epsilons, 3);
        let mut counts = Vec::new();
        reference_loop_filtration_edge_counts_into(&dist, &epsilons, 3, &mut scratch, &mut counts);
        assert_eq!(counts, owned_counts);

        let owned_betti = reference_loop_filtration_betti(&dist, &epsilons, 3);
        let mut betti = Vec::new();
        reference_loop_filtration_betti_into(&dist, &epsilons, 3, &mut scratch, &mut betti);
        assert_eq!(betti, owned_betti);

        let owned_births = reference_h1_birth_scales(&dist, &epsilons, 3);
        let mut births = Vec::new();
        reference_h1_birth_scales_into(&dist, &epsilons, 3, &mut scratch, &mut births);
        assert_eq!(births, owned_births);
    }

    /// Closure-bar: `reference_loop_filtration_betti` must produce identical
    /// (b0, b1) tuples to the underlying primitive when called on the
    /// same edge mask. If the consumer ever drifts (e.g. computes b1
    /// from edge count alone) this test fails.
    #[test]
    fn betti_filtration_matches_primitive_on_each_epsilon() {
        let dist = vec![
            0.0, 0.2, 0.4, 0.2, 0.0, 0.3, 0.4, 0.3, 0.0, // K3 with mixed dists
        ];
        let epsilons = vec![0.1, 0.25, 0.35, 0.5];
        let series = reference_loop_filtration_betti(&dist, &epsilons, 3);
        for (idx, &eps) in epsilons.iter().enumerate() {
            let mask = reference_region_loop_skeleton(&dist, eps, 3);
            let (b0_p, b1_p, _) = betti_persistence_cpu(&mask, 3);
            assert_eq!(series[idx], (b0_p, b1_p));
        }
    }

    /// Adversarial: a disjoint pair of triangles must have b1 = 2 at a
    /// scale that includes both triangles' edges. Naive code that only
    /// counts cycles within one component would fail.
    #[test]
    fn betti_adversarial_two_disjoint_triangles_has_b1_two() {
        // 6 nodes split into two disjoint triangles.
        // Within each triangle: pairwise distance 0.4.
        // Across triangles: pairwise distance 5.0 (never connect).
        let mut dist = vec![5.0; 36];
        for i in 0..6 {
            dist[i * 6 + i] = 0.0;
        }
        for &(i, j) in &[(0, 1), (0, 2), (1, 2), (3, 4), (3, 5), (4, 5)] {
            dist[i * 6 + j] = 0.4;
            dist[j * 6 + i] = 0.4;
        }
        let series = reference_loop_filtration_betti(&dist, &[0.5], 6);
        let (b0, b1) = series[0];
        assert_eq!((b0, b1), (2, 2));
    }

    /// Adversarial: an empty epsilons slice must yield an empty
    /// series, not panic and not allocate phantom entries.
    #[test]
    fn betti_filtration_empty_epsilons_returns_empty() {
        let dist = vec![0.0, 0.1, 0.1, 0.0];
        let series = reference_loop_filtration_betti(&dist, &[], 2);
        assert!(series.is_empty());
        let births = reference_h1_birth_scales(&dist, &[], 2);
        assert!(births.is_empty());
    }
}
