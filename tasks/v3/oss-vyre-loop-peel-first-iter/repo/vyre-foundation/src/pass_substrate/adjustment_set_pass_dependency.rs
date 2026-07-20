//! Optimizer pass-ordering validity via causal adjustment-set analysis.
//!
//! The pass scheduler models rewrite preconditions as a directed graph:
//! `a[i, j] != 0` means pass `i` can influence pass `j`. A candidate
//! ordering is safe for treatment pass `t` and outcome pass `o` when
//! the ordering does not place an unblocked dependency from `o` back to
//! `t`; such a path would make the proposed order cyclic under the
//! causal intervention "run `t` before `o`".

use super::dataflow_fixpoint::reachability_closure;

/// Return whether ordering pass `t` before pass `o` is acyclic.
///
/// `adj` is a row-major `n x n` pass-dependency adjacency matrix. The
/// check computes the transitive dependency closure and rejects any
/// ordering where `o` can already reach `t`.
///
#[must_use]
pub fn ordering_is_safe(adj: &[u32], treatment: u32, outcome: u32, n: u32) -> bool {
    let Some(cells) = n.checked_mul(n).map(|v| v as usize) else {
        return false;
    };
    if adj.len() != cells || treatment >= n || outcome >= n {
        return false;
    }
    if treatment == outcome {
        return true;
    }

    let closure = reachability_closure(adj, n, n);
    closure[(outcome * n + treatment) as usize] == 0
}

/// For each pass index `i`, the strict descendants reachable in the influence digraph.
///
/// `adj` is row-major `n×n` with `adj[i·n + j] != 0` meaning pass `i` may directly
/// influence pass `j`. Returns `out[i] = { j | i ≠ j and i ↪ j in the transitive
/// closure of that graph }`, sorted ascending by `j`.
#[must_use]
pub fn pass_descendants(adj: &[u32], n: u32) -> Vec<Vec<u32>> {
    if n == 0 {
        return Vec::new();
    }
    let Some(cells) = n.checked_mul(n).map(|v| v as usize) else {
        return Vec::new();
    };
    if adj.len() != cells {
        return Vec::new();
    }
    let closure = reachability_closure(adj, n, n);
    let mut out = vec![Vec::new(); n as usize];
    for i in 0..n {
        for j in 0..n {
            if i != j && closure[(i * n + j) as usize] != 0 {
                out[i as usize].push(j);
            }
        }
    }
    for row in &mut out {
        row.sort_unstable();
    }
    out
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
    fn pass_descendants_chain() {
        // 0 → 1 → 2
        let adj = vec![0, 1, 0, 0, 0, 1, 0, 0, 0];
        let d = pass_descendants(&adj, 3);
        assert_eq!(d[0], vec![1, 2]);
        assert_eq!(d[1], vec![2]);
        assert!(d[2].is_empty());
    }

    #[test]
    fn pass_descendants_empty_graph() {
        let adj: [u32; 0] = [];
        let d = pass_descendants(&adj, 0);
        assert!(d.is_empty());
    }
}
