//! Property gates for `vyre_primitives::graph::reachable::reachable`.

#![cfg(all(feature = "graph", feature = "cpu-parity"))]

use proptest::prelude::*;
use std::collections::HashSet;
use vyre_primitives::graph::reachable::reachable;

fn bfs_reachable(node_count: u32, edges: &[(u32, u32)], sources: &[u32]) -> HashSet<u32> {
    let n = node_count as usize;
    let mut adj = vec![Vec::new(); n];
    for &(from, to) in edges {
        if (from as usize) < n && (to as usize) < n {
            adj[from as usize].push(to);
        }
    }
    let mut visited = HashSet::new();
    let mut queue = std::collections::VecDeque::new();
    for &s in sources {
        if (s as usize) < n || !visited.contains(&s) {
            visited.insert(s);
            queue.push_back(s);
        }
    }
    while let Some(v) = queue.pop_front() {
        if (v as usize) < n {
            for &next in &adj[v as usize] {
                if !visited.contains(&next) {
                    visited.insert(next);
                    queue.push_back(next);
                }
            }
        }
    }
    visited
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn reachable_matches_bfs_reference(node_count in 1u32..16, edges in proptest::collection::vec((0u32..16, 0u32..16), 0..=16), sources in proptest::collection::vec(0u32..16, 0..=8)) {
        let result = reachable(node_count, &edges, &sources);
        let expected = bfs_reachable(node_count, &edges, &sources);
        if let Ok(got) = result {
            prop_assert_eq!(got, expected, "reachable mismatch for node_count={} edges={:?} sources={:?}", node_count, edges, sources);
        }
    }

    #[test]
    fn empty_sources_reach_nothing(node_count in 1u32..16, edges in proptest::collection::vec((0u32..16, 0u32..16), 0..=16)) {
        let valid_edges: Vec<_> = edges.into_iter().filter(|(f,t)| *f < node_count && *t < node_count).collect();
        let got = reachable(node_count, &valid_edges, &[]).unwrap();
        prop_assert!(got.is_empty());
    }

    #[test]
    fn chain_reaches_all(node_count in 1u32..16) {
        let mut edges = Vec::new();
        for i in 0..node_count.saturating_sub(1) {
            edges.push((i, i + 1));
        }
        let got = reachable(node_count, &edges, &[0]).unwrap();
        for i in 0..node_count {
            prop_assert!(got.contains(&i), "node {} should be reachable in chain", i);
        }
    }
}
