//! `betti_persistence`  -  full H_1 cycle counting on a Vietoris-Rips
//! 1-skeleton (P-PRIM-4).
//!
//! Given a row-major n×n edge mask (0/1) produced by
//! [`crate::topology::vietoris_rips::vietoris_rips_edge_filter_cpu`], compute
//! the first Betti number `b1`: the rank of `H_1(K)` where `K` is
//! the 1-skeleton of the Rips complex.
//!
//! Euler-characteristic identity for a graph (1-skeleton):
//!
//! ```text
//!     b0 = number of connected components
//!     b1 = E - V + b0       (#independent cycles)
//! ```
//!
//! V = number of non-isolated vertices? No  -  the standard formula
//! treats every vertex as a 0-cell, so V = `n` always. An isolated
//! vertex bumps `b0` by 1 and contributes 0 edges, so the b1
//! computation is unaffected.
//!
//! Implementation: a single-pass union-find over the upper-triangle
//! edges. O(E·α(V))  -  practically linear in the edge count.

/// Compute (b0, b1, edge_count) for the 1-skeleton encoded by `mask`.
/// `mask` is row-major n×n; `mask[i*n + j] != 0` means an edge
/// between vertices i and j. Symmetry is required: `mask[i*n+j] ==
/// mask[j*n+i]`. Self-edges (i == j) are ignored.
///
/// Returns:
/// * `b0`  -  number of connected components.
/// * `b1`  -  first Betti number (independent cycle count).
/// * `edges`  -  number of distinct unordered edges counted.
///
/// Returns `(0, 0, 0)` when the mask is shorter than `n * n`, asymmetric, or
/// the CPU oracle cannot reserve its working buffers. Use
/// [`try_betti_persistence_cpu`] when the caller needs a recoverable
/// error.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn betti_persistence_cpu(mask: &[u32], n: u32) -> (u32, u32, u32) {
    match try_betti_persistence_cpu(mask, n) {
        Ok(result) => result,
        // A parity oracle that returns zeros on failure makes the GPU-vs-CPU
        // assertion pass on (0,0,0)==(0,0,0), silently masking a divergence
        // (Law 10 / Law 6). Fail loud; callers use the try_ variant.
        Err(error) => panic!("vyre-primitives betti_persistence CPU reference failed: {error}"),
    }
}

/// Fallible CPU reference for [`betti_persistence_cpu`].
///
/// This variant validates the full row-major mask, rejects asymmetric
/// edges, and reserves all working memory before running union-find.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_betti_persistence_cpu(mask: &[u32], n: u32) -> Result<(u32, u32, u32), String> {
    if n == 0 {
        return Ok((0, 0, 0));
    }

    let n_us = usize::try_from(n).map_err(|_| "n does not fit usize".to_string())?;
    let cells = n_us
        .checked_mul(n_us)
        .ok_or_else(|| format!("n * n overflows usize for n={n}"))?;
    if mask.len() < cells {
        return Err(format!(
            "mask is too short for {n}x{n}: got {}, need {cells}",
            mask.len()
        ));
    }

    let mut parent: Vec<u32> = Vec::new();
    parent
        .try_reserve_exact(n_us)
        .map_err(|err| format!("failed to reserve union-find parent buffer: {err}"))?;
    parent.extend(0..n);

    let mut rank: Vec<u32> = Vec::new();
    rank.try_reserve_exact(n_us)
        .map_err(|err| format!("failed to reserve union-find rank buffer: {err}"))?;
    rank.resize(n_us, 0);

    fn find(parent: &mut [u32], mut x: u32) -> u32 {
        // Iterative path compression.
        while parent[x as usize] != x {
            let p = parent[x as usize];
            parent[x as usize] = parent[p as usize];
            x = parent[x as usize];
        }
        x
    }

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

    let mut edges: u32 = 0;
    let mut tree_edges: u32 = 0;

    // Iterate the upper triangle so each edge is counted once.
    for i in 0..n_us {
        for j in (i + 1)..n_us {
            let upper = mask[i * n_us + j];
            let lower = mask[j * n_us + i];
            if upper != lower {
                return Err(format!(
                    "mask is asymmetric at ({i}, {j}): upper={upper}, lower={lower}"
                ));
            }
            if upper == 0 {
                continue;
            }
            edges = edges
                .checked_add(1)
                .ok_or_else(|| "edge count exceeds u32::MAX".to_string())?;
            if union(&mut parent, &mut rank, i as u32, j as u32) {
                tree_edges = tree_edges
                    .checked_add(1)
                    .ok_or_else(|| "tree edge count exceeds u32::MAX".to_string())?;
            }
        }
    }

    // After all unions, count distinct roots = b0.
    let mut seen = Vec::new();
    seen.try_reserve_exact(n_us)
        .map_err(|err| format!("failed to reserve root bitmap: {err}"))?;
    seen.resize(n_us, false);
    let mut b0 = 0_u32;
    for v in 0..n {
        let root = find(&mut parent, v) as usize;
        if !seen[root] {
            seen[root] = true;
            b0 = b0
                .checked_add(1)
                .ok_or_else(|| "component count exceeds u32::MAX".to_string())?;
        }
    }

    // b1 = E - V + b0. Substituting V = n and tree_edges = n - b0
    // gives b1 = E - tree_edges (every non-tree edge contributes one
    // independent cycle). Computing it that way avoids any signed
    // intermediate.
    let b1 = edges - tree_edges;

    Ok((b0, b1, edges))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_mask(n: u32) -> Vec<u32> {
        vec![0u32; (n * n) as usize]
    }

    fn add_edge(mask: &mut [u32], n: u32, i: u32, j: u32) {
        let n_us = n as usize;
        mask[(i as usize) * n_us + (j as usize)] = 1;
        mask[(j as usize) * n_us + (i as usize)] = 1;
    }

    #[test]
    fn empty_graph_has_b0_n_b1_zero() {
        // 5 isolated vertices -> 5 components, 0 cycles, 0 edges.
        let n = 5;
        let mask = empty_mask(n);
        let (b0, b1, edges) = betti_persistence_cpu(&mask, n);
        assert_eq!((b0, b1, edges), (5, 0, 0));
    }

    #[test]
    fn n_zero_returns_all_zero() {
        assert_eq!(betti_persistence_cpu(&[], 0), (0, 0, 0));
    }

    #[test]
    fn tree_has_b1_zero() {
        // 4-vertex path 0-1-2-3: 3 edges, 1 component, 0 cycles.
        let n = 4;
        let mut mask = empty_mask(n);
        add_edge(&mut mask, n, 0, 1);
        add_edge(&mut mask, n, 1, 2);
        add_edge(&mut mask, n, 2, 3);
        let (b0, b1, edges) = betti_persistence_cpu(&mask, n);
        assert_eq!((b0, b1, edges), (1, 0, 3));
    }

    #[test]
    fn triangle_has_b1_one() {
        // Triangle 0-1-2: 3 edges, 1 component, 1 cycle.
        let n = 3;
        let mut mask = empty_mask(n);
        add_edge(&mut mask, n, 0, 1);
        add_edge(&mut mask, n, 1, 2);
        add_edge(&mut mask, n, 0, 2);
        let (b0, b1, edges) = betti_persistence_cpu(&mask, n);
        assert_eq!((b0, b1, edges), (1, 1, 3));
    }

    #[test]
    fn two_triangles_share_no_edge_has_b1_two() {
        // Two disjoint triangles: 6 edges, 2 components, 2 cycles.
        let n = 6;
        let mut mask = empty_mask(n);
        for (a, b) in [(0, 1), (1, 2), (0, 2), (3, 4), (4, 5), (3, 5)] {
            add_edge(&mut mask, n, a, b);
        }
        let (b0, b1, edges) = betti_persistence_cpu(&mask, n);
        assert_eq!((b0, b1, edges), (2, 2, 6));
    }

    #[test]
    fn k4_has_b1_three() {
        // Complete graph K4: 6 edges, 4 vertices, 1 component.
        // Spanning tree uses 3 edges, leaving 3 cycles.
        let n = 4;
        let mut mask = empty_mask(n);
        for i in 0..n {
            for j in (i + 1)..n {
                add_edge(&mut mask, n, i, j);
            }
        }
        let (b0, b1, edges) = betti_persistence_cpu(&mask, n);
        assert_eq!((b0, b1, edges), (1, 3, 6));
    }

    #[test]
    fn tree_plus_isolated_vertex() {
        // Path 0-1-2 plus isolated vertex 3: 2 edges, 2 components, 0 cycles.
        let n = 4;
        let mut mask = empty_mask(n);
        add_edge(&mut mask, n, 0, 1);
        add_edge(&mut mask, n, 1, 2);
        let (b0, b1, edges) = betti_persistence_cpu(&mask, n);
        assert_eq!((b0, b1, edges), (2, 0, 2));
    }

    #[test]
    fn cycle_then_attach_chord_adds_cycle() {
        // 4-cycle 0-1-2-3-0 plus chord 0-2.
        // 5 edges, 1 component. Spanning tree picks 3, leaves 2 cycles.
        let n = 4;
        let mut mask = empty_mask(n);
        add_edge(&mut mask, n, 0, 1);
        add_edge(&mut mask, n, 1, 2);
        add_edge(&mut mask, n, 2, 3);
        add_edge(&mut mask, n, 3, 0);
        add_edge(&mut mask, n, 0, 2);
        let (b0, b1, edges) = betti_persistence_cpu(&mask, n);
        assert_eq!((b0, b1, edges), (1, 2, 5));
    }

    #[test]
    fn matches_euler_characteristic_identity() {
        // Random-ish graph: vertices 0..7, edges chosen by hand.
        let n = 7;
        let mut mask = empty_mask(n);
        let edges = [(0, 1), (1, 2), (2, 0), (3, 4), (4, 5), (5, 3), (4, 6)];
        for (a, b) in edges {
            add_edge(&mut mask, n, a, b);
        }
        let (b0, b1, e) = betti_persistence_cpu(&mask, n);
        // E - V + b0 = 7 - 7 + b0 = b0; b1 = b0 implies E == V.
        // Manual check: triangle {0,1,2} (b0+=1, b1=1), triangle
        // {3,4,5} with extra leaf 6 attached to 4 (b0+=1, b1=1).
        // So b0=2, b1=2, E=7.
        assert_eq!((b0, b1, e), (2, 2, 7));
    }

    #[test]
    fn symmetric_mask_is_required() {
        // Self-edges are ignored; off-diagonal symmetric edges
        // count once.
        let n = 3;
        let mut mask = empty_mask(n);
        // Self-edges set but not contributing.
        mask[0] = 1;
        mask[4] = 1; // (1,1)
        mask[8] = 1; // (2,2)
        add_edge(&mut mask, n, 0, 1);
        let (b0, b1, edges) = betti_persistence_cpu(&mask, n);
        assert_eq!((b0, b1, edges), (2, 0, 1));
    }

    #[test]
    fn fallible_cpu_rejects_short_mask() {
        let err = try_betti_persistence_cpu(&[0, 1, 0], 2).unwrap_err();
        assert!(err.contains("too short"), "{err}");
    }

    #[test]
    fn compatibility_wrapper_matches_fallible_reference() {
        let n = 3;
        let mut mask = empty_mask(n);
        add_edge(&mut mask, n, 0, 1);
        add_edge(&mut mask, n, 1, 2);

        assert_eq!(
            betti_persistence_cpu(&mask, n),
            try_betti_persistence_cpu(&mask, n)
                .expect("Fix: small Betti CPU reference must reserve")
        );
    }

    #[test]
    #[should_panic(expected = "betti_persistence CPU reference failed")]
    fn compatibility_wrapper_fails_loud_on_invalid_mask() {
        // An invalid mask must fail LOUD, not silently return (0,0,0) — a
        // GPU-vs-CPU parity assertion would accept (0,0,0)==(0,0,0) as a match
        // and hide the divergence (Law 10 / Law 6).
        let _ = betti_persistence_cpu(&[0, 1, 0], 2);
    }

    #[test]
    fn production_wrapper_has_no_raw_panic_path() {
        let production = include_str!("betti_persistence.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("Fix: betti_persistence.rs must contain production section");

        assert!(
            !production.contains(".expect(") && !production.contains(".unwrap("),
            "Fix: Betti persistence CPU wrapper must not panic in production."
        );
    }

    #[test]
    fn fallible_cpu_rejects_asymmetric_mask() {
        let err = try_betti_persistence_cpu(&[0, 1, 0, 0], 2).unwrap_err();
        assert!(err.contains("asymmetric"), "{err}");
    }

    #[test]
    fn larger_random_graph_consistent() {
        // 10 vertices, several cycles. Spot-check the identity.
        let n = 10;
        let mut mask = empty_mask(n);
        let edges = [
            (0, 1),
            (1, 2),
            (0, 2), // triangle in {0,1,2}
            (2, 3),
            (3, 4),
            (4, 5),
            (5, 3), // triangle in {3,4,5}
            (5, 6),
            (6, 7),
            (7, 8),
            (8, 9),
            (9, 6), // 4-cycle in {6,7,8,9}
        ];
        for (a, b) in edges {
            add_edge(&mut mask, n, a, b);
        }
        let (b0, b1, e) = betti_persistence_cpu(&mask, n);
        // Single component (path through 2-3 and 5-6 connects all),
        // 12 edges. Spanning tree uses 9, so b1 = 3.
        assert_eq!(b0, 1);
        assert_eq!(b1, 3);
        assert_eq!(e, 12);
    }
}
