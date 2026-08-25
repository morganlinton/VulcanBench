//! Polyhedral / affine fusion queries over pass dependency graphs.

use super::dataflow_fixpoint::reachability_closure_into;

/// Return an `n x n` mask of independently fusable pass pairs.
///
/// `adj[i*n + j] != 0` means pass `i` must precede pass `j`. Two
/// passes are fusable when neither reaches the other in the transitive
/// dependency closure.
#[must_use]
pub fn fusable_pairs(adj: &[u32], n: u32, max_iters: u32) -> Vec<u32> {
    let mut closure = Vec::new();
    let mut next = Vec::new();
    let mut out = Vec::new();
    fusable_pairs_with_scratch_into(adj, n, max_iters, &mut closure, &mut next, &mut out);
    out
}

/// Return an `n x n` mask of independently fusable pass pairs into
/// caller-owned output storage.
pub fn fusable_pairs_into(adj: &[u32], n: u32, max_iters: u32, out: &mut Vec<u32>) {
    let mut closure = Vec::new();
    let mut next = Vec::new();
    fusable_pairs_with_scratch_into(adj, n, max_iters, &mut closure, &mut next, out);
}

/// Return an `n x n` mask of independently fusable pass pairs using
/// caller-owned closure, next, and output buffers.
pub fn fusable_pairs_with_scratch_into(
    adj: &[u32],
    n: u32,
    max_iters: u32,
    closure: &mut Vec<u32>,
    next: &mut Vec<u32>,
    out: &mut Vec<u32>,
) {
    if n == 0 {
        closure.clear();
        next.clear();
        out.clear();
        return;
    }
    let Some(cells) = n.checked_mul(n).map(|v| v as usize) else {
        closure.clear();
        next.clear();
        out.clear();
        return;
    };
    if adj.len() != cells {
        closure.clear();
        next.clear();
        out.clear();
        return;
    }
    reachability_closure_into(adj, n, max_iters.max(1), closure, next);
    let n_usize = n as usize;
    out.clear();
    out.resize(n_usize * n_usize, 0);
    for i in 0..n_usize {
        for j in 0..n_usize {
            if i != j && closure[i * n_usize + j] == 0 && closure[j * n_usize + i] == 0 {
                out[i * n_usize + j] = 1;
            }
        }
    }
}

/// Count independently fusable ordered pass pairs.
#[must_use]
pub fn fusion_score(adj: &[u32], n: u32, max_iters: u32) -> u32 {
    let mask = fusable_pairs(adj, n, max_iters);
    mask.iter().sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn independent_passes_are_fusable() {
        // 3 passes, no dependencies → all pairs fusable.
        #[rustfmt::skip]
        let adj = vec![
            0, 0, 0,
            0, 0, 0,
            0, 0, 0,
        ];
        let fused = fusable_pairs(&adj, 3, 3);
        // (0,1), (1,0), (0,2), (2,0), (1,2), (2,1) should all be 1.
        assert_eq!(fused[0 * 3 + 1], 1);
        assert_eq!(fused[1 * 3 + 0], 1);
        assert_eq!(fused[0 * 3 + 2], 1);
        assert_eq!(fused[2 * 3 + 0], 1);
        // Diagonal is always 0 (can't fuse with self).
        assert_eq!(fused[0], 0);
    }

    #[test]
    fn dependent_passes_are_not_fusable() {
        // Chain: 0 → 1 → 2.
        #[rustfmt::skip]
        let adj = vec![
            0, 1, 0,
            0, 0, 1,
            0, 0, 0,
        ];
        let fused = fusable_pairs(&adj, 3, 3);
        // 0 reaches 1 and 2 transitively, 1 reaches 2. No independent pairs.
        assert_eq!(fused[0 * 3 + 1], 0);
        assert_eq!(fused[0 * 3 + 2], 0);
        assert_eq!(fused[1 * 3 + 2], 0);
    }

    #[test]
    fn diamond_top_and_bottom_not_fusable() {
        // Diamond: 0 → 1, 0 → 2, 1 → 3, 2 → 3.
        #[rustfmt::skip]
        let adj = vec![
            0, 1, 1, 0,
            0, 0, 0, 1,
            0, 0, 0, 1,
            0, 0, 0, 0,
        ];
        let fused = fusable_pairs(&adj, 4, 4);
        // 1 and 2 are independent (neither reaches the other).
        assert_eq!(fused[1 * 4 + 2], 1);
        assert_eq!(fused[2 * 4 + 1], 1);
        // 0 and 3 are NOT fusable (0 reaches 3 transitively).
        assert_eq!(fused[0 * 4 + 3], 0);
    }

    #[test]
    fn empty_graph_returns_empty() {
        let fused = fusable_pairs(&[], 0, 0);
        assert!(fused.is_empty());
    }

    #[test]
    fn fusable_pairs_into_reuses_output_and_clears_invalid_shape() {
        let adj = vec![0u32; 9];
        let mut out = Vec::with_capacity(16);
        let ptr = out.as_ptr();
        fusable_pairs_into(&adj, 3, 3, &mut out);
        assert_eq!(out, fusable_pairs(&adj, 3, 3));
        assert_eq!(out.as_ptr(), ptr);

        fusable_pairs_into(&[0, 1, 0], 2, 3, &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn fusable_pairs_with_scratch_reuses_all_buffers() {
        let adj = vec![0u32; 9];
        let mut closure = Vec::with_capacity(16);
        let mut next = Vec::with_capacity(16);
        let mut out = Vec::with_capacity(16);
        let closure_ptr = closure.as_ptr();
        let next_ptr = next.as_ptr();
        let out_ptr = out.as_ptr();
        fusable_pairs_with_scratch_into(&adj, 3, 3, &mut closure, &mut next, &mut out);
        assert_eq!(out, fusable_pairs(&adj, 3, 3));
        assert_eq!(closure.as_ptr(), closure_ptr);
        assert_eq!(next.as_ptr(), next_ptr);
        assert_eq!(out.as_ptr(), out_ptr);
        assert_eq!(fusion_score(&adj, 3, 3), 6);
    }
}
