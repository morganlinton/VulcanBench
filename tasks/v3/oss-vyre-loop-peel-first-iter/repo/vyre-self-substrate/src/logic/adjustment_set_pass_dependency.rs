//! Optimizer pass-ordering validity via causal adjustment-set analysis.
//!
//! The pass scheduler models rewrite preconditions as a directed graph:
//! `a[i, j] != 0` means pass `i` can influence pass `j`. A candidate
//! ordering is safe for treatment pass `t` and outcome pass `o` when
//! the ordering does not place an unblocked dependency from `o` back to
//! `t`; such a path would make the proposed order cyclic under the
//! causal intervention "run `t` before `o`".

use vyre_foundation::pass_substrate::adjustment_set_pass_dependency as foundation_pass_dependency;

/// Return whether ordering pass `t` before pass `o` is acyclic.
///
/// `adj` is a row-major `n x n` pass-dependency adjacency matrix. The
/// check computes the transitive dependency closure and rejects any
/// ordering where `o` can already reach `t`.
///
#[must_use]
pub fn ordering_is_safe(adj: &[u32], treatment: u32, outcome: u32, n: u32) -> bool {
    use crate::observability::{adjustment_set_pass_dependency_calls, bump};
    bump(&adjustment_set_pass_dependency_calls);
    foundation_pass_dependency::ordering_is_safe(adj, treatment, outcome, n)
}

/// For each pass index, return strict descendants reachable in the pass
/// influence graph.
#[must_use]
pub fn pass_descendants(adj: &[u32], n: u32) -> Vec<Vec<u32>> {
    use crate::observability::{adjustment_set_pass_dependency_calls, bump};
    bump(&adjustment_set_pass_dependency_calls);
    foundation_pass_dependency::pass_descendants(adj, n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_reverse_dependency_cycle() {
        let adj = vec![0, 0, 1, 0];
        assert!(!ordering_is_safe(&adj, 0, 1, 2));
    }

    #[test]
    fn accepts_forward_dependency_order() {
        let adj = vec![0, 1, 0, 0];
        assert!(ordering_is_safe(&adj, 0, 1, 2));
    }

    #[test]
    fn invalid_shapes_are_rejected_without_panicking() {
        assert!(!ordering_is_safe(&[0, 1, 0], 0, 1, 2));
        assert!(!ordering_is_safe(&[0, 1, 0, 0], 2, 1, 2));
        assert!(!ordering_is_safe(&[0, 1, 0, 0], 0, 2, 2));
    }

    #[test]
    fn delegates_descendant_sets_to_foundation_authority() {
        let adj = vec![0, 1, 0, 0, 0, 1, 0, 0, 0];
        assert_eq!(pass_descendants(&adj, 3), vec![vec![1, 2], vec![2], vec![]]);
    }
}
