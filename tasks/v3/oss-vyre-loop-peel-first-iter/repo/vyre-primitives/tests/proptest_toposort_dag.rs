//! Property gates for `vyre_primitives::graph::toposort` on DAGs.

#![cfg(feature = "graph")]

use proptest::prelude::*;
use vyre_primitives::graph::toposort::{toposort, ToposortError};

fn is_topo_order(node_count: u32, edges: &[(u32, u32)], order: &[u32]) -> bool {
    let n = node_count as usize;
    if order.len() != n {
        return false;
    }
    let mut pos = vec![None; n];
    for (i, &v) in order.iter().enumerate() {
        if v as usize >= n {
            return false;
        }
        if pos[v as usize].is_some() {
            return false;
        }
        pos[v as usize] = Some(i);
    }
    for &(from, to) in edges {
        let Some(pi) = pos[to as usize] else {
            return false;
        };
        let Some(pj) = pos[from as usize] else {
            return false;
        };
        if pi >= pj {
            return false;
        }
    }
    true
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn chain_graph_toposorts_in_order(n in 1u32..=16) {
        let edges: Vec<(u32, u32)> = (0..n.saturating_sub(1))
            .map(|i| (i + 1, i))
            .collect();
        let order = toposort(n, &edges).expect("DAG");
        prop_assert_eq!(order.len() as u32, n);
        prop_assert!(is_topo_order(n, &edges, &order));
    }

    #[test]
    fn edgeless_graph_is_any_permutation_of_nodes(n in 1u32..=12) {
        let order = toposort(n, &[]).expect("no edges");
        let mut sorted = order.clone();
        sorted.sort_unstable();
        prop_assert_eq!(sorted, (0..n).collect::<Vec<_>>());
    }

    #[test]
    fn unknown_node_is_rejected(
        (node_count, bad) in (2u32..=8, 8u32..=32),
    ) {
        let edges = [(bad, 0)];
        let err = toposort(node_count, &edges).unwrap_err();
        prop_assert!(matches!(err, ToposortError::UnknownNode { .. }), "expected UnknownNode");
    }

    #[test]
    fn two_node_cycle_errors(_dummy in Just(())) {
        let edges = [(0, 1), (1, 0)];
        let err = toposort(2, &edges).unwrap_err();
        prop_assert!(matches!(err, ToposortError::Cycle { .. }), "expected Cycle");
    }
}
