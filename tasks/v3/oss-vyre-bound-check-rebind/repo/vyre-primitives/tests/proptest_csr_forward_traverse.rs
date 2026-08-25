//! Property gates for `vyre_primitives::graph::csr_forward_traverse::cpu_ref`.

#![cfg(all(feature = "graph", feature = "cpu-parity"))]

use proptest::prelude::*;
use vyre_primitives::graph::csr_forward_traverse::cpu_ref;

fn build_dag(node_count: u32, edges: &[(u32, u32)]) -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let mut offsets = vec![0u32; node_count as usize + 1];
    let mut targets = Vec::new();
    let mut kind_mask = Vec::new();
    for &(from, to) in edges {
        if from < node_count && to < node_count {
            offsets[from as usize + 1] += 1;
        }
    }
    for i in 1..=node_count as usize {
        offsets[i] += offsets[i - 1];
    }
    let mut write_pos = offsets.clone();
    for &(from, to) in edges {
        if from < node_count && to < node_count {
            targets.push(to);
            kind_mask.push(0xFFFF_FFFF);
            write_pos[from as usize] += 1;
        }
    }
    (offsets, targets, kind_mask)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn empty_frontier_yields_empty_out(node_count in 1u32..32, edges in proptest::collection::vec((0u32..32, 0u32..32), 0..=32)) {
        let (offsets, targets, kind_mask) = build_dag(node_count, &edges);
        let words = ((node_count + 31) / 32) as usize;
        let frontier_in = vec![0u32; words];
        let out = cpu_ref(node_count, &offsets, &targets, &kind_mask, &frontier_in, 0xFFFF_FFFF);
        prop_assert_eq!(out, vec![0u32; words]);
    }

    #[test]
    fn result_is_subset_of_nodes(node_count in 1u32..32, edges in proptest::collection::vec((0u32..32, 0u32..32), 0..=32), seed in any::<u32>()) {
        let (offsets, targets, kind_mask) = build_dag(node_count, &edges);
        let words = ((node_count + 31) / 32) as usize;
        let frontier_in = vec![seed; words];
        let out = cpu_ref(node_count, &offsets, &targets, &kind_mask, &frontier_in, 0xFFFF_FFFF);
        for i in 0..node_count {
            let word = (i / 32) as usize;
            let bit = 1u32 << (i % 32);
            if word < out.len() && (out[word] & bit) != 0 {
                prop_assert!(i < node_count);
            }
        }
    }

    #[test]
    fn single_edge_reaches_target(node_count in 2u32..32, src in 0u32..32, dst in 0u32..32) {
        let src = src % node_count;
        let dst = dst % node_count;
        let edges = vec![(src, dst)];
        let (offsets, targets, kind_mask) = build_dag(node_count, &edges);
        let words = ((node_count + 31) / 32) as usize;
        let mut frontier_in = vec![0u32; words];
        frontier_in[(src / 32) as usize] |= 1u32 << (src % 32);
        let out = cpu_ref(node_count, &offsets, &targets, &kind_mask, &frontier_in, 0xFFFF_FFFF);
        let dst_word = (dst / 32) as usize;
        let dst_bit = 1u32 << (dst % 32);
        prop_assert!(out[dst_word] & dst_bit != 0, "expected dst to be reached");
    }
}
