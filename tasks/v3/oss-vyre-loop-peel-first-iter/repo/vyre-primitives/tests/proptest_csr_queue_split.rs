//! Property gates for mixed scalar plus row-strided CSR queue traversal.

#![cfg(all(feature = "graph", feature = "cpu-parity"))]

use proptest::prelude::*;
use vyre_primitives::bitset::bitset_words;
use vyre_primitives::graph::csr_queue_split::{
    csr_queue_split_low_dispatch_grid, csr_queue_split_mixed_logical_lanes,
    try_csr_queue_split_low_forward_traverse_cpu, CSR_QUEUE_SPLIT_LOW_FORWARD_WORKGROUP_SIZE,
};
use vyre_primitives::graph::csr_queue_strided::{
    CSR_QUEUE_STRIDED_FORWARD_LANES_PER_SOURCE, CSR_QUEUE_STRIDED_FORWARD_WORKGROUP_SIZE,
};

#[derive(Clone, Debug)]
struct GeneratedCsr {
    edge_offsets: Vec<u32>,
    edge_targets: Vec<u32>,
    edge_kind_mask: Vec<u32>,
    primary_hub: u32,
    secondary_hub: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SplitOracle {
    frontier_after_low: Vec<u32>,
    high_queue: Vec<u32>,
    high_len: u32,
    scalar_full: Vec<u32>,
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn split_low_cpu_matches_independent_oracle_on_mixed_skewed_queues(
        node_count in 1u32..=640,
        graph_seed in any::<u64>(),
        queue_seed in any::<u64>(),
        out_seed in any::<u64>(),
        queue_slots in 0usize..=192,
        queue_len_extra in 0u32..=160,
        high_capacity_salt in any::<u32>(),
        threshold_salt in 0u32..=255,
        allow_mask in any::<u32>(),
    ) {
        let graph = generated_csr(node_count, graph_seed);
        let active_queue =
            generated_active_queue(node_count, graph.primary_hub, graph.secondary_hub, queue_slots, queue_seed);
        let queue_len = (active_queue.len() as u32).saturating_add(queue_len_extra);
        let high_degree_threshold = 1 + threshold_salt % 129;
        let high_count = high_degree_source_count(
            &active_queue,
            queue_len,
            &graph.edge_offsets,
            node_count,
            high_degree_threshold,
        );
        let high_capacity =
            (high_capacity_salt as usize) % high_count.saturating_add(9).max(1);
        let frontier_seed = generated_frontier_seed(node_count, out_seed);
        let allow_mask = allow_mask | 1;
        let expected = split_oracle(
            &active_queue,
            queue_len,
            &graph.edge_offsets,
            &graph.edge_targets,
            &graph.edge_kind_mask,
            &frontier_seed,
            node_count,
            high_capacity,
            high_degree_threshold,
            allow_mask,
        );

        let actual = try_csr_queue_split_low_forward_traverse_cpu(
            &active_queue,
            queue_len,
            &graph.edge_offsets,
            &graph.edge_targets,
            &graph.edge_kind_mask,
            &frontier_seed,
            node_count,
            high_capacity,
            high_degree_threshold,
            allow_mask,
        )
        .expect("Fix: generated canonical CSR split queue graph should traverse");

        prop_assert_eq!(&actual.frontier_out, &expected.frontier_after_low);
        prop_assert_eq!(&actual.high_queue, &expected.high_queue);
        prop_assert_eq!(actual.high_len, expected.high_len);

        let mut mixed_full = actual.frontier_out.clone();
        for &src in &actual.high_queue {
            emit_row(
                src,
                &graph.edge_offsets,
                &graph.edge_targets,
                &graph.edge_kind_mask,
                node_count,
                allow_mask,
                &mut mixed_full,
            );
        }
        prop_assert_eq!(mixed_full, expected.scalar_full);
    }

    #[test]
    fn split_low_grid_and_lane_accounting_cover_queue_slots_without_global_striding(
        queue_capacity in any::<u32>(),
        high_queue_capacity in any::<u32>(),
    ) {
        let grid = csr_queue_split_low_dispatch_grid(queue_capacity);
        let expected_blocks = queue_capacity
            .div_ceil(CSR_QUEUE_SPLIT_LOW_FORWARD_WORKGROUP_SIZE[0])
            .max(1);
        let mixed_lanes =
            csr_queue_split_mixed_logical_lanes(queue_capacity, high_queue_capacity);
        let naive_strided_lanes =
            u64::from(queue_capacity) * u64::from(CSR_QUEUE_STRIDED_FORWARD_LANES_PER_SOURCE);

        prop_assert_eq!(grid, [expected_blocks, 1, 1]);
        prop_assert!(
            u64::from(grid[0]) * u64::from(CSR_QUEUE_SPLIT_LOW_FORWARD_WORKGROUP_SIZE[0])
                >= u64::from(queue_capacity)
        );
        if high_queue_capacity <= queue_capacity {
            prop_assert!(mixed_lanes >= u64::from(queue_capacity));
            prop_assert!(
                mixed_lanes <= naive_strided_lanes.saturating_add(u64::from(queue_capacity))
            );
        }
        prop_assert_eq!(CSR_QUEUE_STRIDED_FORWARD_WORKGROUP_SIZE, [256, 1, 1]);
    }
}

#[test]
fn split_low_with_zero_high_capacity_expands_every_high_row_scalar() {
    let node_count = 96;
    let graph = generated_csr(node_count, 0x51a7_ba5e_cafe_f00d);
    let active_queue = vec![
        graph.primary_hub,
        7,
        graph.secondary_hub,
        node_count + 3,
        31,
    ];
    let queue_len = active_queue.len() as u32;
    let frontier_seed = generated_frontier_seed(node_count, 0xa5a5_5a5a_1234_5678);
    let expected = split_oracle(
        &active_queue,
        queue_len,
        &graph.edge_offsets,
        &graph.edge_targets,
        &graph.edge_kind_mask,
        &frontier_seed,
        node_count,
        0,
        4,
        0b1011,
    );

    let actual = try_csr_queue_split_low_forward_traverse_cpu(
        &active_queue,
        queue_len,
        &graph.edge_offsets,
        &graph.edge_targets,
        &graph.edge_kind_mask,
        &frontier_seed,
        node_count,
        0,
        4,
        0b1011,
    )
    .expect("Fix: split CPU oracle should handle zero-capacity high queues");

    assert_eq!(actual.high_queue, Vec::<u32>::new());
    assert_eq!(actual.high_len, expected.high_len);
    assert_eq!(actual.frontier_out, expected.scalar_full);
}

#[test]
fn split_low_rejects_malformed_csr_with_actionable_diagnostic() {
    let err = try_csr_queue_split_low_forward_traverse_cpu(
        &[0],
        1,
        &[0, 3],
        &[0, 1],
        &[1, 1],
        &[0],
        1,
        1,
        1,
        1,
    )
    .expect_err("Fix: split-low must reject a final CSR offset beyond edge storage");

    assert!(
        err.contains("final offset declares edge_count"),
        "Fix: malformed CSR diagnostic must name the final offset mismatch, got: {err}"
    );
}

fn generated_csr(node_count: u32, seed: u64) -> GeneratedCsr {
    let primary_hub = (mix64(seed ^ 0x7096_c321_d1b5_4a32) % u64::from(node_count)) as u32;
    let secondary_hub = (mix64(seed ^ 0xf00d_5eed_8bad_f00d) % u64::from(node_count)) as u32;
    let mut edge_offsets = Vec::with_capacity(node_count as usize + 1);
    let mut edge_targets = Vec::new();
    let mut edge_kind_mask = Vec::new();
    edge_offsets.push(0);
    for src in 0..node_count {
        let row_seed = mix64(seed ^ u64::from(src).wrapping_mul(0xd1b5_4a32_d192_ed03));
        let degree = if src == primary_hub {
            256 + (row_seed % 257) as u32
        } else if src == secondary_hub {
            96 + (row_seed % 97) as u32
        } else if src % 37 == 0 {
            33 + (row_seed % 32) as u32
        } else if src % 11 == 0 {
            6 + (row_seed % 11) as u32
        } else {
            (row_seed % 5) as u32
        };
        for edge_ordinal in 0..degree {
            let edge_seed =
                mix64(row_seed ^ u64::from(edge_ordinal).wrapping_mul(0x94d0_49bb_1331_11eb));
            edge_targets.push((edge_seed % u64::from(node_count)) as u32);
            edge_kind_mask.push(match (edge_seed >> 21) % 9 {
                0 => 0,
                1..=4 => 1,
                5 => 0b0100,
                6 => 0b1010,
                7 => 0x8000_0000,
                _ => 0xffff_ffff,
            });
        }
        edge_offsets.push(edge_targets.len() as u32);
    }
    GeneratedCsr {
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        primary_hub,
        secondary_hub,
    }
}

fn generated_active_queue(
    node_count: u32,
    primary_hub: u32,
    secondary_hub: u32,
    slots: usize,
    seed: u64,
) -> Vec<u32> {
    let mut queue = Vec::with_capacity(slots);
    for slot in 0..slots {
        let slot_seed = mix64(seed ^ (slot as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15));
        let src = match slot % 17 {
            0 | 9 => primary_hub,
            1 | 10 => secondary_hub,
            2 => node_count.saturating_add((slot_seed % 1024) as u32),
            3 => node_count - 1,
            _ => (slot_seed % u64::from(node_count)) as u32,
        };
        queue.push(src);
    }
    queue
}

fn generated_frontier_seed(node_count: u32, seed: u64) -> Vec<u32> {
    let words = bitset_words(node_count) as usize;
    (0..words)
        .map(|word| {
            let mut bits = mix64(seed ^ (word as u64).wrapping_mul(0x517c_c1b7_2722_0a95)) as u32;
            if word % 7 == 0 {
                bits |= 1;
            }
            bits
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn split_oracle(
    active_queue: &[u32],
    queue_len: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    frontier_seed: &[u32],
    node_count: u32,
    high_queue_capacity: usize,
    high_degree_threshold: u32,
    allow_mask: u32,
) -> SplitOracle {
    let mut frontier_after_low = frontier_seed.to_vec();
    let mut scalar_full = frontier_seed.to_vec();
    let mut high_queue = Vec::with_capacity(high_queue_capacity);
    let mut high_len = 0_u32;
    let take = (queue_len as usize).min(active_queue.len());

    for &src in &active_queue[..take] {
        if src >= node_count {
            continue;
        }
        let start = edge_offsets[src as usize] as usize;
        let end = edge_offsets[src as usize + 1] as usize;
        emit_row(
            src,
            edge_offsets,
            edge_targets,
            edge_kind_mask,
            node_count,
            allow_mask,
            &mut scalar_full,
        );
        if end.saturating_sub(start) as u32 >= high_degree_threshold {
            high_len = high_len.saturating_add(1);
            if high_queue.len() < high_queue_capacity {
                high_queue.push(src);
                continue;
            }
        }
        emit_row(
            src,
            edge_offsets,
            edge_targets,
            edge_kind_mask,
            node_count,
            allow_mask,
            &mut frontier_after_low,
        );
    }

    SplitOracle {
        frontier_after_low,
        high_queue,
        high_len,
        scalar_full,
    }
}

fn high_degree_source_count(
    active_queue: &[u32],
    queue_len: u32,
    edge_offsets: &[u32],
    node_count: u32,
    high_degree_threshold: u32,
) -> usize {
    let take = (queue_len as usize).min(active_queue.len());
    active_queue[..take]
        .iter()
        .filter(|&&src| {
            if src >= node_count {
                return false;
            }
            let start = edge_offsets[src as usize];
            let end = edge_offsets[src as usize + 1];
            end.saturating_sub(start) >= high_degree_threshold
        })
        .count()
}

fn emit_row(
    src: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    node_count: u32,
    allow_mask: u32,
    out: &mut [u32],
) {
    if src >= node_count {
        return;
    }
    let start = edge_offsets[src as usize] as usize;
    let end = edge_offsets[src as usize + 1] as usize;
    for edge in start..end {
        if edge_kind_mask[edge] & allow_mask == 0 {
            continue;
        }
        let dst = edge_targets[edge];
        out[dst as usize / 32] |= 1_u32 << (dst % 32);
    }
}

fn mix64(mut x: u64) -> u64 {
    x ^= x >> 30;
    x = x.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}
