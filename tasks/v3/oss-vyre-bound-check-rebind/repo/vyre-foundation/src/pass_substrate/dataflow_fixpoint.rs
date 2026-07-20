//! Region-graph dataflow fixpoint via #1 `semiring_gemm` (#26 substrate).
//!
//! Treats vyre's Region tree adjacency as a sparse boolean matrix
//! and computes reachability / liveness / dominance / constant-prop
//! via `semiring_gemm` iterations under different semirings:
//!
//! | Analysis | Semiring | Combine | Accumulate |
//! |---|---|---|---|
//! | Reachability | `BoolOr` | AND | OR |
//! | Liveness | `BoolOr` (reverse direction) | AND | OR |
//! | Reaching defs | `Lineage` | OR (zero-absorbing) | OR |
//! | Constant prop | `Lineage` | OR | OR |
//! | Min-cost path | `MinPlus` | + (sat) | min |
//!
//! Same primitive (#1), same Program, four different IR analyses.
//! Demonstrates the recursion thesis directly.

#![allow(deprecated)]

pub use vyre_spec::Semiring;

/// Multiply matrices over the selected semiring on the CPU.
///
/// AUDIT_2026-05-23: Deprecated - CPU reference. Use GPU semiring GEMM primitive.
#[deprecated(note = "CPU reference. Use GPU semiring GEMM primitive.")]
#[must_use]
pub fn semiring_gemm_cpu(
    left: &[u32],
    right: &[u32],
    rows: u32,
    cols: u32,
    inner: u32,
    semiring: Semiring,
) -> Vec<u32> {
    let mut output = Vec::new();
    semiring_gemm_cpu_into(left, right, rows, cols, inner, semiring, &mut output);
    output
}

/// Multiply matrices over the selected semiring into caller-owned storage.
///
/// Invalid shapes clear `output`, matching [`semiring_gemm_cpu`]'s empty-vector
/// rejection contract without forcing wrappers to duplicate overflow and input
/// sizing checks.
pub fn semiring_gemm_cpu_into(
    left: &[u32],
    right: &[u32],
    rows: u32,
    cols: u32,
    inner: u32,
    semiring: Semiring,
    output: &mut Vec<u32>,
) {
    let Some(out_len) = rows.checked_mul(cols).map(|v| v as usize) else {
        output.clear();
        return;
    };
    let Some(left_len) = rows.checked_mul(inner).map(|v| v as usize) else {
        output.clear();
        return;
    };
    let Some(right_len) = inner.checked_mul(cols).map(|v| v as usize) else {
        output.clear();
        return;
    };
    if left.len() < left_len || right.len() < right_len {
        output.clear();
        return;
    }
    output.clear();
    output.resize(out_len, semiring.identity());
    let rows = rows as usize;
    let cols = cols as usize;
    let inner = inner as usize;
    for row in 0..rows {
        for col in 0..cols {
            let mut acc = semiring.identity();
            for scan in 0..inner {
                let left_value = left[row * inner + scan];
                let right_value = right[scan * cols + col];

                let combined = match semiring {
                    Semiring::Real | Semiring::MaxTimes => left_value.wrapping_mul(right_value),
                    Semiring::MinPlus => {
                        if left_value == u32::MAX || right_value == u32::MAX {
                            u32::MAX
                        } else {
                            left_value.saturating_add(right_value)
                        }
                    }
                    Semiring::MaxPlus => left_value.saturating_add(right_value),
                    Semiring::BoolOr | Semiring::Gf2 => left_value & right_value,
                    Semiring::BoolAnd => left_value | right_value,
                    Semiring::Lineage => {
                        if left_value == 0 || right_value == 0 {
                            0
                        } else {
                            left_value | right_value
                        }
                    }
                };

                acc = match semiring {
                    Semiring::Real | Semiring::MaxPlus => acc.wrapping_add(combined),
                    Semiring::MinPlus => acc.min(combined),
                    Semiring::MaxTimes => acc.max(combined),
                    Semiring::BoolOr | Semiring::Lineage => acc | combined,
                    Semiring::BoolAnd => acc & combined,
                    Semiring::Gf2 => acc ^ combined,
                };
            }
            output[row * cols + col] = acc;
        }
    }
}

fn square_cells(n: u32) -> Option<usize> {
    n.checked_mul(n).map(|cells| cells as usize)
}

/// Compute boolean reachability closure on a Region adjacency matrix
/// via repeated `semiring_gemm` iterations under `Semiring::BoolOr`.
/// Iterates until fixpoint (max `max_iters` steps).
#[must_use]
pub fn reachability_closure(adj: &[u32], n: u32, max_iters: u32) -> Vec<u32> {
    let mut current = Vec::new();
    let mut next = Vec::new();
    reachability_closure_into(adj, n, max_iters, &mut current, &mut next);
    current
}

/// Compute boolean reachability closure into caller-owned buffers.
///
/// Invalid shapes clear both buffers and return, matching the owned helper's
/// empty-vector rejection contract.
pub fn reachability_closure_into(
    adj: &[u32],
    n: u32,
    max_iters: u32,
    current: &mut Vec<u32>,
    next: &mut Vec<u32>,
) {
    let Some(cells) = square_cells(n) else {
        current.clear();
        next.clear();
        return;
    };
    if n == 0 || adj.len() != cells {
        current.clear();
        next.clear();
        return;
    }
    current.clear();
    current.extend_from_slice(adj);
    next.clear();
    for _ in 0..max_iters {
        semiring_gemm_cpu_into(current, current, n, n, n, Semiring::BoolOr, next);
        if !merge_or_changed(current, next) {
            return;
        }
    }
}

/// Compute lineage (which-clauses-used) closure under `Semiring::Lineage`.
/// Each entry of `adj` is a bitset of clause/source IDs.
#[must_use]
pub fn lineage_closure(adj: &[u32], n: u32, max_iters: u32) -> Vec<u32> {
    let mut current = Vec::new();
    let mut next = Vec::new();
    lineage_closure_into(adj, n, max_iters, &mut current, &mut next);
    current
}

/// Compute lineage closure into caller-owned buffers.
pub fn lineage_closure_into(
    adj: &[u32],
    n: u32,
    max_iters: u32,
    current: &mut Vec<u32>,
    next: &mut Vec<u32>,
) {
    let Some(cells) = square_cells(n) else {
        current.clear();
        next.clear();
        return;
    };
    if n == 0 || adj.len() != cells {
        current.clear();
        next.clear();
        return;
    }
    current.clear();
    current.extend_from_slice(adj);
    next.clear();
    for _ in 0..max_iters {
        semiring_gemm_cpu_into(current, current, n, n, n, Semiring::Lineage, next);
        if !merge_or_changed(current, next) {
            return;
        }
    }
}

/// Compute min-cost shortest-path distance matrix under `Semiring::MinPlus`.
/// Use `u32::MAX` for "no edge".
#[must_use]
pub fn shortest_path_closure(adj: &[u32], n: u32, max_iters: u32) -> Vec<u32> {
    let mut current = Vec::new();
    let mut next = Vec::new();
    shortest_path_closure_into(adj, n, max_iters, &mut current, &mut next);
    current
}

/// Compute min-cost shortest-path closure into caller-owned buffers.
pub fn shortest_path_closure_into(
    adj: &[u32],
    n: u32,
    max_iters: u32,
    current: &mut Vec<u32>,
    next: &mut Vec<u32>,
) {
    let Some(cells) = square_cells(n) else {
        current.clear();
        next.clear();
        return;
    };
    if n == 0 || adj.len() != cells {
        current.clear();
        next.clear();
        return;
    }
    current.clear();
    current.extend_from_slice(adj);
    next.clear();
    for _ in 0..max_iters {
        semiring_gemm_cpu_into(current, current, n, n, n, Semiring::MinPlus, next);
        if !merge_min_changed(current, next) {
            return;
        }
    }
}

/// Merge `next` into `current` with bitwise OR and return whether any cell changed.
///
/// `current` and `next` must have the same length. This helper is public so
/// higher-tier GPU dispatch wrappers can reuse the same convergence semantics
/// instead of forking the fixpoint merge kernel.
#[must_use]
pub fn merge_or_changed(current: &mut [u32], next: &[u32]) -> bool {
    debug_assert_eq!(current.len(), next.len());
    let mut changed = false;
    for (dst, src) in current.iter_mut().zip(next.iter()) {
        let merged = *dst | *src;
        changed |= merged != *dst;
        *dst = merged;
    }
    changed
}

/// Merge `next` into `current` with element-wise minimum and return whether any cell changed.
///
/// `current` and `next` must have the same length. This is the shared
/// convergence merge for MinPlus shortest-path closure.
#[must_use]
pub fn merge_min_changed(current: &mut [u32], next: &[u32]) -> bool {
    debug_assert_eq!(current.len(), next.len());
    let mut changed = false;
    for (dst, src) in current.iter_mut().zip(next.iter()) {
        let merged = (*dst).min(*src);
        changed |= merged != *dst;
        *dst = merged;
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reachability_chain_graph() {
        // 0 → 1 → 2 → 3
        let adj = vec![0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0];
        let closure = reachability_closure(&adj, 4, 5);
        // After closure: 0 reaches {1, 2, 3}; 1 reaches {2, 3}; 2 reaches {3}.
        assert_eq!(closure[0 * 4 + 1], 1);
        assert_eq!(closure[0 * 4 + 2], 1);
        assert_eq!(closure[0 * 4 + 3], 1);
        assert_eq!(closure[1 * 4 + 3], 1);
        // No reverse edges
        assert_eq!(closure[3 * 4 + 0], 0);
    }

    #[test]
    fn semiring_gemm_into_reuses_output_and_clears_on_invalid_shape() {
        let left = vec![1, 2, 3, 4, 5, 6];
        let right = vec![7, 8, 9, 10, 11, 12];
        let mut out = Vec::with_capacity(8);
        let ptr = out.as_ptr();
        semiring_gemm_cpu_into(&left, &right, 2, 2, 3, Semiring::Real, &mut out);
        assert_eq!(
            out,
            semiring_gemm_cpu(&left, &right, 2, 2, 3, Semiring::Real)
        );
        assert_eq!(out.as_ptr(), ptr);

        semiring_gemm_cpu_into(&left, &right, 2, 2, u32::MAX, Semiring::Real, &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn reachability_into_matches_owned_and_clears_invalid_shape() {
        let adj = vec![0, 1, 0, 0, 0, 1, 0, 0, 0];
        let mut current = Vec::with_capacity(16);
        let mut next = Vec::with_capacity(16);
        let current_ptr = current.as_ptr();
        reachability_closure_into(&adj, 3, 3, &mut current, &mut next);
        assert_eq!(current, reachability_closure(&adj, 3, 3));
        assert_eq!(current.as_ptr(), current_ptr);

        reachability_closure_into(&[0, 1, 0], 2, 3, &mut current, &mut next);
        assert!(current.is_empty());
        assert!(next.is_empty());
    }

    #[test]
    fn closure_into_helpers_respect_zero_max_iters() {
        let reach = vec![0, 1, 0, 0, 0, 1, 0, 0, 0];
        let lineage = vec![0, 1, 0, 0, 0, 2, 0, 0, 0];
        let inf = u32::MAX;
        let shortest = vec![inf, 5, 100, inf, inf, 3, inf, inf, inf];
        let mut current = Vec::new();
        let mut next = Vec::new();

        reachability_closure_into(&reach, 3, 0, &mut current, &mut next);
        assert_eq!(current, reach);
        lineage_closure_into(&lineage, 3, 0, &mut current, &mut next);
        assert_eq!(current, lineage);
        shortest_path_closure_into(&shortest, 3, 0, &mut current, &mut next);
        assert_eq!(current, shortest);
    }

    #[test]
    fn reachability_disjoint_components_stay_disjoint() {
        // 0 → 1, 2 → 3, no cross.
        let adj = vec![0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0];
        let closure = reachability_closure(&adj, 4, 5);
        assert_eq!(closure[0 * 4 + 2], 0);
        assert_eq!(closure[2 * 4 + 0], 0);
    }

    #[test]
    fn lineage_closure_unions_clauses_along_paths() {
        // Edge 0→1 used clause f1 = 0b01; edge 1→2 used clause f2 = 0b10.
        // Path 0→2 uses both: 0b11.
        let f1 = 0b01;
        let f2 = 0b10;
        let adj = vec![0, f1, 0, 0, 0, f2, 0, 0, 0];
        let closure = lineage_closure(&adj, 3, 5);
        assert_eq!(closure[0 * 3 + 2], f1 | f2);
    }

    #[test]
    fn shortest_path_closure_finds_two_hop_minimum() {
        let inf = u32::MAX;
        // 0→1 cost 5, 1→2 cost 3, 0→2 cost 100 (slower direct).
        let adj = vec![inf, 5, 100, inf, inf, 3, inf, inf, inf];
        let closure = shortest_path_closure(&adj, 3, 5);
        // Best 0→2 = min(100, 5+3) = 8.
        assert_eq!(closure[0 * 3 + 2], 8);
    }

    #[test]
    fn reachability_self_loop_detected() {
        // 0 → 1, 1 → 0. Closure should mark both.
        let adj = vec![0, 1, 1, 0];
        let closure = reachability_closure(&adj, 2, 5);
        // After 1 iteration: 0 reaches 0 via 0→1→0; 1 reaches 1.
        assert_eq!(closure[0 * 2 + 0], 1);
        assert_eq!(closure[1 * 2 + 1], 1);
    }
}
