//! Volume oracle matrix - independent reference vs production cpu_ref.
//! Legendary testing.volume - do NOT weaken to shape-only asserts.
#![forbid(unsafe_code)]
#![cfg(all(feature = "graph", feature = "cpu-parity"))]

use std::collections::{HashSet, VecDeque};

use vyre_primitives::graph::reachable::reachable;

fn bitset_words(node_count: u32) -> usize {
    vyre_primitives::bitset::bitset_words(node_count) as usize
}

fn next_u32(rng: &mut u64) -> u32 {
    *rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
    (*rng >> 32) as u32
}

fn generated_csr_frontier(seed: u64) -> (u32, Vec<u32>, Vec<u32>, Vec<u32>, Vec<u32>, u32) {
    let mut rng = seed;
    let node_count = 1 + next_u32(&mut rng) % 96;
    let words = bitset_words(node_count);
    let mut offsets = Vec::with_capacity(node_count as usize + 1);
    let mut targets = Vec::new();
    let mut masks = Vec::new();
    offsets.push(0);
    for _ in 0..node_count {
        let degree = next_u32(&mut rng) % 6;
        for _ in 0..degree {
            targets.push(next_u32(&mut rng) % node_count);
            masks.push(1u32 << (next_u32(&mut rng) % 5));
        }
        offsets.push(targets.len() as u32);
    }
    let mut frontier = vec![0u32; words];
    let start = next_u32(&mut rng) % node_count;
    frontier[(start / 32) as usize] |= 1u32 << (start % 32);
    let allow_mask = 0xFFFF_FFFFu32;
    (node_count, offsets, targets, masks, frontier, allow_mask)
}

fn generated_edges(seed: u64, node_count: u32) -> Vec<(u32, u32)> {
    let mut rng = seed;
    let n = node_count.max(1);
    let edge_count = 1 + (next_u32(&mut rng) % 32) as usize;
    (0..edge_count)
        .map(|_| (next_u32(&mut rng) % n, next_u32(&mut rng) % n))
        .collect()
}

fn oracle_reachable(node_count: u32, edges: &[(u32, u32)], sources: &[u32]) -> HashSet<u32> {
    let mut seen = HashSet::new();
    let mut queue = VecDeque::new();
    for &s in sources {
        if s < node_count {
            seen.insert(s);
            queue.push_back(s);
        }
    }
    while let Some(u) = queue.pop_front() {
        for &(from, to) in edges {
            if from == u && to < node_count && seen.insert(to) {
                queue.push_back(to);
            }
        }
    }
    seen
}

const CASES: usize = 16384;

#[test]
fn sweep_graph_reachable_volume_oracle_matrix() {
    for case in 0..CASES {
        let seed = case as u64 ^ 0xAEAC4AB1E;
        let mut rng = seed;
        let node_count = 2 + next_u32(&mut rng) % 48;
        let edges = generated_edges(seed.rotate_left(9), node_count);
        let source = next_u32(&mut rng) % node_count;
        let sources = vec![source];
        let expected = oracle_reachable(node_count, &edges, &sources);
        let actual = reachable(node_count, &edges, &sources)
            .expect("Fix: generated reachable volume inputs must be in-range");
        assert_eq!(actual, expected, "Fix: reachable volume case {case}");
    }
}
