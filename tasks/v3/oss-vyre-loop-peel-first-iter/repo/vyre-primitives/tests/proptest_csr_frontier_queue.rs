//! Property gates for queue-driven sparse CSR traversal.

#![cfg(all(feature = "graph", feature = "cpu-parity"))]

use proptest::prelude::*;
use vyre_primitives::bitset::bitset_words;
use vyre_primitives::graph::csr_frontier_queue::{
    try_csr_queue_forward_traverse_cpu, try_frontier_to_queue_cpu, validate_csr_queue_graph,
    CsrQueueGraphLayout,
};

#[derive(Clone, Debug)]
struct GeneratedCsr {
    edge_offsets: Vec<u32>,
    edge_targets: Vec<u32>,
    edge_kind_mask: Vec<u32>,
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn queue_materialization_matches_independent_sparse_frontier_oracle(
        node_count in 1u32..=4096,
        seed in any::<u64>(),
        capacity_salt in any::<u32>(),
    ) {
        let frontier = generated_frontier_words(node_count, seed);
        let queue_capacity = (capacity_salt as usize) % (node_count as usize + 33);
        let expected_nodes = active_nodes(&frontier, node_count);
        let expected_queue = expected_nodes
            .iter()
            .copied()
            .take(queue_capacity)
            .collect::<Vec<_>>();

        let (queue, seen) = try_frontier_to_queue_cpu(&frontier, node_count, queue_capacity)
            .expect("Fix: generated canonical frontier should materialize");

        prop_assert_eq!(seen, expected_nodes.len() as u32);
        prop_assert_eq!(queue, expected_queue);
    }

    #[test]
    fn queue_forward_traverse_matches_independent_csr_oracle(
        node_count in 1u32..=384,
        graph_seed in any::<u64>(),
        frontier_seed in any::<u64>(),
        capacity_salt in any::<u32>(),
        allow_mask in any::<u32>(),
    ) {
        let graph = generated_csr(node_count, graph_seed);
        let frontier = generated_frontier_words(node_count, frontier_seed);
        let queue_capacity = (capacity_salt as usize) % (node_count as usize + 1);
        let (queue, queue_len) = try_frontier_to_queue_cpu(&frontier, node_count, queue_capacity)
            .expect("Fix: generated canonical frontier should materialize");
        let expected = queue_forward_oracle(
            &queue,
            queue_len,
            &graph.edge_offsets,
            &graph.edge_targets,
            &graph.edge_kind_mask,
            node_count,
            allow_mask,
        );

        let actual = try_csr_queue_forward_traverse_cpu(
            &queue,
            queue_len,
            &graph.edge_offsets,
            &graph.edge_targets,
            &graph.edge_kind_mask,
            node_count,
            allow_mask,
        )
        .expect("Fix: generated canonical CSR queue graph should traverse");

        prop_assert_eq!(actual, expected);
    }

    #[test]
    fn queue_forward_ignores_invalid_sources_and_clamps_queue_len_to_active_storage(
        node_count in 1u32..=256,
        graph_seed in any::<u64>(),
        active_queue in prop::collection::vec(any::<u32>(), 0..96),
        queue_len in any::<u32>(),
        allow_mask in any::<u32>(),
    ) {
        let graph = generated_csr(node_count, graph_seed);
        let expected = queue_forward_oracle(
            &active_queue,
            queue_len,
            &graph.edge_offsets,
            &graph.edge_targets,
            &graph.edge_kind_mask,
            node_count,
            allow_mask,
        );

        let actual = try_csr_queue_forward_traverse_cpu(
            &active_queue,
            queue_len,
            &graph.edge_offsets,
            &graph.edge_targets,
            &graph.edge_kind_mask,
            node_count,
            allow_mask,
        )
        .expect("Fix: generated canonical CSR queue graph should traverse adversarial queues");

        prop_assert_eq!(actual, expected);
    }

    #[test]
    fn csr_queue_validation_accepts_canonical_and_rejects_single_field_mutations(
        node_count in 1u32..=512,
        graph_seed in any::<u64>(),
    ) {
        let graph = generated_csr(node_count, graph_seed);
        let layout = validate_csr_queue_graph(
            node_count,
            &graph.edge_offsets,
            &graph.edge_targets,
            &graph.edge_kind_mask,
        )
        .expect("Fix: generated canonical CSR queue graph should validate");
        prop_assert_eq!(
            layout,
            CsrQueueGraphLayout {
                node_count,
                edge_count: graph.edge_targets.len() as u32,
                max_row_degree: max_row_degree(&graph.edge_offsets),
                words: bitset_words(node_count) as usize,
                edge_storage_words: graph.edge_targets.len().max(1),
            }
        );

        let mut non_zero_start = graph.edge_offsets.clone();
        non_zero_start[0] = 1;
        prop_assert!(
            validate_csr_queue_graph(
                node_count,
                &non_zero_start,
                &graph.edge_targets,
                &graph.edge_kind_mask,
            )
            .expect_err("Fix: non-zero CSR start offset must be rejected")
            .contains("edge_offsets[0] == 0")
        );

        let mut bad_final = graph.edge_offsets.clone();
        let last = bad_final
            .last_mut()
            .expect("Fix: generated CSR offsets include node_count + 1 entries");
        *last = last.saturating_add(1);
        prop_assert!(
            validate_csr_queue_graph(
                node_count,
                &bad_final,
                &graph.edge_targets,
                &graph.edge_kind_mask,
            )
            .expect_err("Fix: final CSR offset mismatch must be rejected")
            .contains("final offset declares edge_count")
        );

        let mut mismatched_masks = graph.edge_kind_mask.clone();
        mismatched_masks.push(1);
        prop_assert!(
            validate_csr_queue_graph(
                node_count,
                &graph.edge_offsets,
                &graph.edge_targets,
                &mismatched_masks,
            )
            .expect_err("Fix: CSR edge target/mask length mismatch must be rejected")
            .contains("edge_targets.len() == edge_kind_mask.len()")
        );

        if !graph.edge_targets.is_empty() {
            let mut out_of_range_targets = graph.edge_targets.clone();
            out_of_range_targets[0] = node_count;
            prop_assert!(
                validate_csr_queue_graph(
                    node_count,
                    &graph.edge_offsets,
                    &out_of_range_targets,
                    &graph.edge_kind_mask,
                )
                .expect_err("Fix: out-of-range CSR target must be rejected")
                .contains("outside node_count")
            );
        }
    }
}

fn generated_frontier_words(node_count: u32, seed: u64) -> Vec<u32> {
    let words = bitset_words(node_count) as usize;
    let mut frontier = Vec::with_capacity(words);
    for word in 0..words {
        frontier.push(mix64(seed ^ (word as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15)) as u32);
    }
    if seed & 1 == 0 {
        set_node(&mut frontier, 0);
    }
    if seed & 2 == 0 {
        set_node(&mut frontier, node_count - 1);
    }
    if node_count > 32 && seed & 4 == 0 {
        set_node(&mut frontier, 32);
    }
    frontier
}

fn generated_csr(node_count: u32, seed: u64) -> GeneratedCsr {
    let mut edge_offsets = Vec::with_capacity(node_count as usize + 1);
    let mut edge_targets = Vec::new();
    let mut edge_kind_mask = Vec::new();
    edge_offsets.push(0);
    for src in 0..node_count {
        let row_seed = mix64(seed ^ (src as u64).wrapping_mul(0xd1b5_4a32_d192_ed03));
        let degree = (row_seed % 5) as u32;
        for edge_ordinal in 0..degree {
            let edge_seed =
                mix64(row_seed ^ (edge_ordinal as u64).wrapping_mul(0x94d0_49bb_1331_11eb));
            edge_targets.push((edge_seed % u64::from(node_count)) as u32);
            let mask_bit = ((edge_seed >> 17) % 9) as u32;
            edge_kind_mask.push(if mask_bit == 8 { 0 } else { 1u32 << mask_bit });
        }
        edge_offsets.push(edge_targets.len() as u32);
    }
    GeneratedCsr {
        edge_offsets,
        edge_targets,
        edge_kind_mask,
    }
}

fn active_nodes(frontier: &[u32], node_count: u32) -> Vec<u32> {
    (0..node_count)
        .filter(|&node| frontier_has_node(frontier, node))
        .collect()
}

fn max_row_degree(edge_offsets: &[u32]) -> u32 {
    edge_offsets
        .windows(2)
        .map(|pair| pair[1] - pair[0])
        .max()
        .unwrap_or(0)
}

fn queue_forward_oracle(
    active_queue: &[u32],
    queue_len: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    node_count: u32,
    allow_mask: u32,
) -> Vec<u32> {
    let mut out = vec![0u32; bitset_words(node_count) as usize];
    let take = (queue_len as usize).min(active_queue.len());
    for &src in &active_queue[..take] {
        if src >= node_count {
            continue;
        }
        let start = edge_offsets[src as usize] as usize;
        let end = edge_offsets[src as usize + 1] as usize;
        for edge_index in start..end {
            if edge_kind_mask[edge_index] & allow_mask != 0 {
                set_node(&mut out, edge_targets[edge_index]);
            }
        }
    }
    out
}

fn frontier_has_node(frontier: &[u32], node: u32) -> bool {
    frontier[node as usize / 32] & (1u32 << (node & 31)) != 0
}

fn set_node(frontier: &mut [u32], node: u32) {
    frontier[node as usize / 32] |= 1u32 << (node & 31);
}

fn mix64(mut value: u64) -> u64 {
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}
