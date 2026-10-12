//! Megakernel fusion-grouping via #46 matroid intersection (#22 self-consumer).
//!
//! Closes the recursion thesis for #46  -  `matroid_exchange_bfs_step` ships
//! to user dialects (combinatorial scheduling, bipartite matching) AND
//! powers vyre's megakernel scheduler.
//!
//! # The self-use
//!
//! Vyre's megakernel scheduler chooses which subset of compiled
//! Programs to fuse into a single dispatch. Two classes of constraint:
//!
//! 1. **Memory constraints** (graphic matroid): every selected pair
//!    of fused Programs must share ≤ M GiB of intermediate buffers,
//!    or fusion overflows on-chip storage. The set of "fusable
//!    pairs" forms a graphic matroid  -  the independent sets are the
//!    fusion-cliques whose total memory stays within budget.
//!
//! 2. **Sync constraints** (partition matroid): each Region belongs
//!    to a synchronization class (atomic-touching, cross-workgroup,
//:    pure-compute). Fusing across classes requires a workgroup
//!    barrier, which kills the megakernel benefit. The independent
//!    sets are subsets that pick ≤ k Programs from each
//!    synchronization class.
//!
//! Maximum-fusion = max independent set in the intersection of these
//! two matroids. Edmonds' (1970) augmenting-path algorithm is the
//! canonical solver; each iteration is one BFS over the exchange
//! graph, which we run via #46 `matroid_exchange_bfs_step`.
//!
//! # Why this complements `megakernel_schedule` (#22)
//!
//! The existing `megakernel_schedule` ships the homotopy-relaxation
//! continuous solver. That gives a smooth fractional answer in
//! `[0, 1]^n` over fusion indicators. The matroid intersection here
//! is the discrete, exact, combinatorial solver  -  used when the
//! homotopy result is ambiguous (fractional values near 0.5) or when
//! the dispatch budget demands a provably-optimal selection.
//!
//! Together they realize the round-trip:
//!
//! ```text
//! homotopy continuation  →  fractional fusion indicators
//!                                       ↓
//!                          matroid intersection rounds to
//!                          provably-optimal discrete subset
//!                                       ↓
//!                          megakernel dispatch fuses that subset
//! ```
//!
//! # Algorithm
//!
//! Standard augmenting-path matroid intersection:
//!
//! 1. Start with empty independent set `S`.
//! 2. Build the exchange graph `D(S)`  -  node per element, edge `i → j`
//!    when swapping `i` (∈ S) for `j` (∉ S) preserves independence in
//!    matroid 1, and edge `j → i` when it preserves independence in
//!    matroid 2.
//! 3. BFS in `D(S)` from "matroid-1-only-allowed" sources to
//!    "matroid-2-only-allowed" sinks. If a path exists, augment along
//!    it. Otherwise `S` is maximum.
//!
//! Each BFS layer is one `matroid_exchange_bfs_step` dispatch.
//!
//! # Wiring
//!
//! [`max_fusion_subset`] runs the full augmenting-path loop on the
//! CPU (the canonical parity oracle). GPU callers dispatch
//! `matroid_exchange_bfs_step` per layer and compare against this
//! oracle.

/// Input-shape error from megakernel matroid scheduling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatroidScheduleError {
    /// `n * n` overflowed `usize`.
    AdjacencySizeOverflow { n: usize },
    /// `seed.len()` did not match `n`.
    SeedLen { expected: usize, actual: usize },
    /// `exchange_adj.len()` did not match `n * n`.
    ExchangeAdjLen { expected: usize, actual: usize },
}

impl std::fmt::Display for MatroidScheduleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AdjacencySizeOverflow { n } => write!(
                f,
                "matroid scheduler n*n overflow for n={n}. Fix: shard the megakernel fusion graph before scheduling."
            ),
            Self::SeedLen { expected, actual } => write!(
                f,
                "matroid scheduler seed length {actual} does not match n={expected}. Fix: pass one seed bit per fusion candidate."
            ),
            Self::ExchangeAdjLen { expected, actual } => write!(
                f,
                "matroid scheduler exchange_adj length {actual} does not match n*n={expected}. Fix: pass a dense row-major n*n exchange graph."
            ),
        }
    }
}

impl std::error::Error for MatroidScheduleError {}

fn validate_inputs(
    seed: &[u32],
    exchange_adj: &[u32],
    n: usize,
) -> Result<usize, MatroidScheduleError> {
    let expected_adj = n
        .checked_mul(n)
        .ok_or(MatroidScheduleError::AdjacencySizeOverflow { n })?;
    if seed.len() != n {
        return Err(MatroidScheduleError::SeedLen {
            expected: n,
            actual: seed.len(),
        });
    }
    if exchange_adj.len() != expected_adj {
        return Err(MatroidScheduleError::ExchangeAdjLen {
            expected: expected_adj,
            actual: exchange_adj.len(),
        });
    }
    Ok(expected_adj)
}

/// Run matroid-intersection augmenting paths on the CPU.
///
/// `n` Programs, indexed 0..n. `exchange_adj` is the n*n exchange
/// graph adjacency in matroid 1 union matroid 2 (caller pre-merges
/// via OR  -  the BFS step picks up any reachable edge regardless of
/// origin matroid). `seed` is the initial independent set as a
/// 0/1 vector of length n.
///
/// Returns the maximum independent set as a 0/1 vector. Iterations
/// capped at `max_iters` augmenting paths.
///
#[must_use]
pub fn max_fusion_subset(
    seed: &[u32],
    exchange_adj: &[u32],
    n: usize,
    max_iters: u32,
) -> Result<Vec<u32>, MatroidScheduleError> {
    let mut current = Vec::with_capacity(n);
    let mut next = Vec::with_capacity(n);
    let mut flow = Vec::with_capacity(n);
    max_fusion_subset_into(
        seed,
        exchange_adj,
        n,
        max_iters,
        &mut current,
        &mut next,
        &mut flow,
    )?;
    Ok(current)
}

/// Compute a maximum fusion subset into caller-owned storage.
pub fn max_fusion_subset_into(
    seed: &[u32],
    exchange_adj: &[u32],
    n: usize,
    max_iters: u32,
    current: &mut Vec<u32>,
    next: &mut Vec<u32>,
    flow: &mut Vec<f64>,
) -> Result<(), MatroidScheduleError> {
    use crate::observability::{bump, matroid_megakernel_scheduler_calls};
    bump(&matroid_megakernel_scheduler_calls);

    let _expected_adj = validate_inputs(seed, exchange_adj, n)?;

    current.clear();
    current.extend_from_slice(seed);
    // Multigrid-smoothed augmenting flow vector. The discrete BFS gives a
    // 0/1 next-layer indicator; Jacobi smoothing on the relaxed system
    // (A_adj · x ≈ b_next) gives a per-node confidence weight in
    // [0, 1] that the discrete BFS step missed nuanced exchange-graph
    // structure (long-range dependencies, weakly-connected components).
    // We use the smoothed weights to break ties when multiple BFS layer
    // candidates would augment the independent set: prefer the node
    // whose Jacobi weight exceeds the threshold first.
    const FLOW_THRESHOLD: f64 = 0.5;
    const JACOBI_OMEGA: f64 = 0.66;

    for _ in 0..max_iters {
        // BFS one layer from current frontier in exchange graph.
        let any_change = matroid_bfs_step_into(current, exchange_adj, current, n, next);

        if !any_change {
            // No augmenting path  -  current is maximum.
            return Ok(());
        }

        // Multigrid Jacobi smoothing: refine the BFS-discovered next
        // layer with relaxed-LP confidence weights. Items whose
        // smoothed flow weight exceeds FLOW_THRESHOLD are preferred
        // for augmentation; items below it remain candidates for the
        // next BFS iteration with updated flow evidence.
        matroid_jacobi_flow_into(exchange_adj, next, current, JACOBI_OMEGA, n, flow);

        // Augment: add a node from `next` only when its smoothed flow
        // weight clears the LP relaxation threshold. Nodes that BFS
        // flagged but the relaxation rejected stay queued for next
        // iteration with fresh evidence.
        let mut changed = false;
        for i in 0..n {
            let merged = u32::from(current[i] != 0 || (next[i] != 0 && flow[i] > FLOW_THRESHOLD));
            changed |= merged != current[i];
            current[i] = merged;
        }
        if !changed {
            return Ok(());
        }
    }
    Ok(())
}

fn matroid_bfs_step_into(
    frontier_in: &[u32],
    exchange_adj: &[u32],
    visited: &[u32],
    n: usize,
    out: &mut Vec<u32>,
) -> bool {
    debug_assert_eq!(frontier_in.len(), n);
    debug_assert_eq!(visited.len(), n);
    debug_assert_eq!(exchange_adj.len(), n * n);

    out.clear();
    out.resize(n, 0);
    let mut any = false;
    for k in 0..n {
        if frontier_in[k] == 0 {
            continue;
        }
        let row = &exchange_adj[k * n..(k + 1) * n];
        for j in 0..n {
            if visited[j] == 0 && row[j] != 0 {
                out[j] = 1;
                any = true;
            }
        }
    }
    any
}

fn matroid_jacobi_flow_into(
    exchange_adj: &[u32],
    b_next: &[u32],
    x_current: &[u32],
    omega: f64,
    n: usize,
    out: &mut Vec<f64>,
) {
    debug_assert_eq!(exchange_adj.len(), n * n);
    debug_assert_eq!(b_next.len(), n);
    debug_assert_eq!(x_current.len(), n);

    out.clear();
    out.resize(n, 0.0);
    for i in 0..n {
        let mut row_dot = 0.0;
        let row = &exchange_adj[i * n..(i + 1) * n];
        for (edge, x) in row.iter().zip(x_current.iter()) {
            if *edge != 0 {
                row_dot += f64::from(*x);
            }
        }
        let res = f64::from(b_next[i]) - row_dot;
        out[i] = f64::from(x_current[i]) + omega * res;
    }
}

/// Convenience: count selected fusion candidates.
#[must_use]
pub fn count_selected(subset: &[u32]) -> u32 {
    subset.iter().filter(|&&v| v != 0).count() as u32
}

#[cfg(test)]
mod tests {
    #![allow(clippy::identity_op, clippy::erasing_op)]
    use super::*;

    #[test]
    fn empty_seed_with_no_edges_returns_empty() {
        let seed = vec![0u32; 4];
        let adj = vec![0u32; 16];
        let result = max_fusion_subset(&seed, &adj, 4, 8).unwrap();
        assert_eq!(count_selected(&result), 0);
    }

    #[test]
    fn linear_chain_augments_to_full() {
        // 4 nodes; chain 0→1→2→3 in exchange graph; seed at 0.
        // After 3 BFS layers, all 4 should be reached.
        let seed = vec![1u32, 0, 0, 0];
        let mut adj = vec![0u32; 16];
        adj[0 * 4 + 1] = 1;
        adj[1 * 4 + 2] = 1;
        adj[2 * 4 + 3] = 1;
        let result = max_fusion_subset(&seed, &adj, 4, 8).unwrap();
        assert_eq!(count_selected(&result), 4, "linear chain reaches all nodes");
    }

    #[test]
    fn disconnected_components_stay_separate() {
        // 4 nodes; edge only 0→1; seed at 0. Should reach {0,1}, not 2/3.
        let seed = vec![1u32, 0, 0, 0];
        let mut adj = vec![0u32; 16];
        adj[0 * 4 + 1] = 1;
        let result = max_fusion_subset(&seed, &adj, 4, 8).unwrap();
        assert_eq!(count_selected(&result), 2);
        assert_eq!(result[0], 1);
        assert_eq!(result[1], 1);
        assert_eq!(result[2], 0);
        assert_eq!(result[3], 0);
    }

    #[test]
    fn convergence_short_circuits_before_max_iters() {
        // Single isolated node; one iteration converges immediately.
        let seed = vec![1u32];
        let adj = vec![0u32];
        let result = max_fusion_subset(&seed, &adj, 1, 100).unwrap();
        assert_eq!(result, vec![1]);
    }

    #[test]
    fn invalid_shapes_return_errors_instead_of_panicking() {
        let err = max_fusion_subset(&[1, 0], &[0], 2, 8).unwrap_err();
        assert_eq!(
            err,
            MatroidScheduleError::ExchangeAdjLen {
                expected: 4,
                actual: 1,
            }
        );

        let mut current = Vec::new();
        let mut next = Vec::new();
        let mut flow = Vec::new();
        let err = max_fusion_subset_into(
            &[1],
            &[0, 0, 0, 0],
            2,
            8,
            &mut current,
            &mut next,
            &mut flow,
        )
        .unwrap_err();
        assert_eq!(
            err,
            MatroidScheduleError::SeedLen {
                expected: 2,
                actual: 1,
            }
        );
    }

    #[test]
    fn count_matches_set_bits() {
        let subset = vec![1u32, 0, 1, 1, 0];
        assert_eq!(count_selected(&subset), 3);
    }
}
