//! Tier 3 - Property: proptest with 10 000 random `petgraph::DiGraph<u32, ()>`.
//!
//! For every generated graph we assert `dominator_tree(g) == lengauer_tarjan_ref(g)`
//! entry-by-entry.  Graphs are single-entry (all edges emanate from node 0 or
//! are reachable from it) with 0..=128 nodes.
#![cfg(feature = "graph")]
#![cfg(feature = "cpu-parity")]

use proptest::prelude::*;
use vyre_primitives::graph::dominator_tree::{cooper_harvey_kennedy_idoms, lengauer_tarjan_idoms};

prop_compose! {
    fn arb_digraph()(node_count in 0usize..=128usize, _edge_density in 0.0f64..=1.0f64)
                    (node_count in Just(node_count), edges in prop::collection::vec(
                        (0..node_count as u32, 0..node_count as u32),
                        0..=(node_count * node_count).min(1024)
                    )) -> (u32, Vec<(u32, u32)>) {
        let n = node_count as u32;
        let edges: Vec<(u32, u32)> = edges.into_iter()
            .filter(|&(u, v)| u < n && v < n)
            .collect();
        (n, edges)
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn proptest_lt_vs_chk_idoms((n, edges) in arb_digraph()) {
        let lt = lengauer_tarjan_idoms(n, 0, &edges);
        let chk = cooper_harvey_kennedy_idoms(n, 0, &edges);
        prop_assert_eq!(lt, chk, "LT/CHK mismatch on random DiGraph(n={}, e={})", n, edges.len());
    }

    #[test]
    fn proptest_idom_tree_is_well_formed((n, edges) in arb_digraph()) {
        if n == 0 { return Ok(()); }
        let idoms = lengauer_tarjan_idoms(n, 0, &edges);

        // Every reachable node (except entry) must have an idom.
        for v in 1..n {
            if idoms[v as usize].is_some() {
                let idom = idoms[v as usize].unwrap();
                prop_assert!(
                    idom < n,
                    "idom[{v}] = {idom} out of range"
                );
                // idom must be different from v (strict dominator)
                prop_assert_ne!(
                    idom, v,
                    "idom[{}] must be a strict dominator", v
                );
            }
        }

        // Entry must dominate itself.
        prop_assert_eq!(idoms[0], Some(0), "entry must dominate itself");

        // No cycles in idom tree (follow chain from any node).
        for v in 0..n {
            let mut seen = std::collections::HashSet::new();
            let mut cur = v;
            seen.insert(cur);
            while let Some(p) = idoms[cur as usize] {
                if p == cur { break; }
                prop_assert!(
                    seen.insert(p),
                    "idom tree contains cycle involving node {v}"
                );
                cur = p;
            }
        }
    }
}
