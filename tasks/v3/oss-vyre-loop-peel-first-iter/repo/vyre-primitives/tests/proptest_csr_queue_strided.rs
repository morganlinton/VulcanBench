//! Property gates for row-strided queue-driven CSR traversal.

#![cfg(all(feature = "graph", feature = "cpu-parity"))]

use proptest::prelude::*;
use vyre_primitives::bitset::bitset_words;
use vyre_primitives::graph::csr_queue_strided::{
    csr_queue_strided_forward_dispatch_grid, try_csr_queue_strided_forward_traverse_cpu,
    try_csr_queue_strided_forward_traverse_cpu_into, CSR_QUEUE_STRIDED_FORWARD_LANES_PER_SOURCE,
    CSR_QUEUE_STRIDED_FORWARD_WORKGROUP_SIZE,
};

#[derive(Clone, Debug)]
struct GeneratedSkewedCsr {
    edge_offsets: Vec<u32>,
    edge_targets: Vec<u32>,
    edge_kind_mask: Vec<u32>,
    hub: u32,
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn strided_queue_cpu_matches_independent_oracle_on_skewed_graphs(
        node_count in 1u32..=512,
        graph_seed in any::<u64>(),
        queue_seed in any::<u64>(),
        queue_slots in 0usize..=128,
        queue_len_extra in 0u32..=96,
        allow_mask in any::<u32>(),
    ) {
        let graph = generated_skewed_csr(node_count, graph_seed);
        let queue = generated_active_queue(node_count, graph.hub, queue_slots, queue_seed);
        let queue_len = (queue.len() as u32).saturating_add(queue_len_extra);
        let allow_mask = allow_mask | 1;
        let expected = independent_queue_oracle(
            &queue,
            queue_len,
            &graph.edge_offsets,
            &graph.edge_targets,
            &graph.edge_kind_mask,
            node_count,
            allow_mask,
        );

        let actual = try_csr_queue_strided_forward_traverse_cpu(
            &queue,
            queue_len,
            &graph.edge_offsets,
            &graph.edge_targets,
            &graph.edge_kind_mask,
            node_count,
            allow_mask,
        )
        .expect("Fix: generated skewed CSR queue graph should traverse");

        prop_assert_eq!(actual, expected);
    }

    #[test]
    fn strided_queue_cpu_into_erases_stale_bits_and_matches_allocating_wrapper(
        node_count in 1u32..=384,
        graph_seed in any::<u64>(),
        queue_seed in any::<u64>(),
        queue_slots in 1usize..=96,
        queue_len_extra in 0u32..=32,
        stale_seed in any::<u32>(),
    ) {
        let graph = generated_skewed_csr(node_count, graph_seed);
        let queue = generated_active_queue(node_count, graph.hub, queue_slots, queue_seed);
        let queue_len = (queue.len() as u32).saturating_add(queue_len_extra);
        let allow_mask = 0b1011;
        let expected = try_csr_queue_strided_forward_traverse_cpu(
            &queue,
            queue_len,
            &graph.edge_offsets,
            &graph.edge_targets,
            &graph.edge_kind_mask,
            node_count,
            allow_mask,
        )
        .expect("Fix: generated skewed CSR queue graph should traverse");
        let mut out = vec![stale_seed; expected.len().saturating_add(9)];

        try_csr_queue_strided_forward_traverse_cpu_into(
            &queue,
            queue_len,
            &graph.edge_offsets,
            &graph.edge_targets,
            &graph.edge_kind_mask,
            node_count,
            allow_mask,
            &mut out,
        )
        .expect("Fix: generated skewed CSR queue graph should traverse into caller storage");

        prop_assert_eq!(out, expected);
    }

    #[test]
    fn strided_dispatch_grid_covers_all_queue_lane_teams(queue_capacity in any::<u32>()) {
        let grid = csr_queue_strided_forward_dispatch_grid(queue_capacity);
        let total_lanes =
            queue_capacity.saturating_mul(CSR_QUEUE_STRIDED_FORWARD_LANES_PER_SOURCE);
        let expected_blocks = total_lanes
            .div_ceil(CSR_QUEUE_STRIDED_FORWARD_WORKGROUP_SIZE[0])
            .max(1);

        prop_assert_eq!(grid, [expected_blocks, 1, 1]);
        prop_assert!(u64::from(grid[0]) * u64::from(CSR_QUEUE_STRIDED_FORWARD_WORKGROUP_SIZE[0])
            >= u64::from(total_lanes));
    }
}

#[test]
fn strided_queue_rejects_malformed_csr_without_clobbering_output() {
    let mut out = vec![0xA5A5_A5A5, 0x5A5A_5A5A];

    let err = try_csr_queue_strided_forward_traverse_cpu_into(
        &[0],
        1,
        &[0, 2],
        &[0],
        &[1],
        1,
        1,
        &mut out,
    )
    .expect_err("Fix: malformed CSR final offset must be rejected");

    assert!(
        err.contains("final offset declares edge_count"),
        "Fix: malformed CSR diagnostic must identify the final offset mismatch, got: {err}"
    );
    assert_eq!(out, vec![0xA5A5_A5A5, 0x5A5A_5A5A]);
}

fn generated_skewed_csr(node_count: u32, seed: u64) -> GeneratedSkewedCsr {
    let hub = (mix64(seed ^ 0x8d12_f4b7_0c55_9d33) % u64::from(node_count)) as u32;
    let hub_degree = CSR_QUEUE_STRIDED_FORWARD_LANES_PER_SOURCE
        .saturating_mul(CSR_QUEUE_STRIDED_FORWARD_LANES_PER_SOURCE)
        .saturating_add((mix64(seed ^ 0x4471_4f03_abcd_02d1) % 2049) as u32);
    let mut edge_offsets = Vec::with_capacity(node_count as usize + 1);
    let mut edge_targets = Vec::new();
    let mut edge_kind_mask = Vec::new();
    edge_offsets.push(0);
    for src in 0..node_count {
        let row_seed = mix64(seed ^ (src as u64).wrapping_mul(0xd1b5_4a32_d192_ed03));
        let degree = if src == hub {
            hub_degree
        } else if src % 31 == 0 {
            32 + (row_seed % 17) as u32
        } else {
            (row_seed % 6) as u32
        };
        for edge_ordinal in 0..degree {
            let edge_seed =
                mix64(row_seed ^ (edge_ordinal as u64).wrapping_mul(0x94d0_49bb_1331_11eb));
            edge_targets.push((edge_seed % u64::from(node_count)) as u32);
            let selector = ((edge_seed >> 19) % 11) as u32;
            edge_kind_mask.push(match selector {
                0 => 0,
                1..=8 => 1u32 << (selector - 1),
                9 => 0b1011,
                _ => 0x8000_0001,
            });
        }
        edge_offsets.push(edge_targets.len() as u32);
    }
    GeneratedSkewedCsr {
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        hub,
    }
}

fn generated_active_queue(node_count: u32, hub: u32, slots: usize, seed: u64) -> Vec<u32> {
    let mut queue = Vec::with_capacity(slots);
    for slot in 0..slots {
        let slot_seed = mix64(seed ^ (slot as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15));
        let src = match slot % 13 {
            0 | 5 => hub,
            1 => node_count.saturating_add((slot_seed % 257) as u32),
            2 => node_count - 1,
            _ => (slot_seed % u64::from(node_count)) as u32,
        };
        queue.push(src);
    }
    queue
}

fn independent_queue_oracle(
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
        for edge in start..end {
            if edge_kind_mask[edge] & allow_mask == 0 {
                continue;
            }
            let dst = edge_targets[edge];
            out[dst as usize / 32] |= 1u32 << (dst % 32);
        }
    }
    out
}

fn mix64(mut x: u64) -> u64 {
    x ^= x >> 30;
    x = x.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}
