//! CUDA parity for mixed scalar plus row-strided CSR queue traversal.

#![cfg(test)]

mod common;

use common::{bytes_u32, live_backend, u32_bytes};
use vyre_driver_cuda::CudaOptimizerDispatcher;
use vyre_primitives::bitset::bitset_words;
use vyre_primitives::graph::csr_queue_split::{
    csr_queue_split_low_dispatch_grid, csr_queue_split_low_forward_traverse,
    try_csr_queue_split_low_forward_traverse_cpu,
};
use vyre_primitives::graph::csr_queue_strided::{
    csr_queue_strided_forward_dispatch_grid, csr_queue_strided_forward_traverse,
};
use vyre_self_substrate::optimizer::dispatcher::{
    OptimizerDispatcher, ResidentDispatchStep, ResidentReadRange,
};

#[test]
fn cuda_resident_csr_queue_split_low_then_high_matches_scalar_on_overflowing_hubs() {
    let backend = live_backend();
    let dispatcher = CudaOptimizerDispatcher::new(&backend);
    let node_count = 2048_u32;
    let queue_capacity = 10_u32;
    let high_queue_capacity = 2_u32;
    let high_degree_threshold = 64_u32;
    let allow_mask = 0b0101_u32;
    let active_queue = vec![0, 7, 1024, node_count + 99, 511, 1536, 31, 0, 1023, 2047];
    let queue_len = [active_queue.len() as u32];
    let (edge_offsets, edge_targets, edge_kind_mask) = generated_power_law_graph(node_count);
    let words = bitset_words(node_count) as usize;
    let frontier_seed = generated_frontier_seed(words);

    let split_expected = try_csr_queue_split_low_forward_traverse_cpu(
        &active_queue,
        queue_len[0],
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
        &frontier_seed,
        node_count,
        high_queue_capacity as usize,
        high_degree_threshold,
        allow_mask,
    )
    .expect("Fix: split-low CPU oracle should accept the generated power-law graph");
    let mut expected_full = split_expected.frontier_out.clone();
    for &src in &split_expected.high_queue {
        emit_row(
            src,
            &edge_offsets,
            &edge_targets,
            &edge_kind_mask,
            node_count,
            allow_mask,
            &mut expected_full,
        );
    }

    let active_queue_handle = dispatcher
        .alloc_resident(active_queue.len() * std::mem::size_of::<u32>())
        .expect("Fix: split active_queue resident allocation failed.");
    let queue_len_handle = dispatcher
        .alloc_resident(std::mem::size_of::<u32>())
        .expect("Fix: split queue_len resident allocation failed.");
    let edge_offsets_handle = dispatcher
        .alloc_resident(edge_offsets.len() * std::mem::size_of::<u32>())
        .expect("Fix: split edge_offsets resident allocation failed.");
    let edge_targets_handle = dispatcher
        .alloc_resident(edge_targets.len() * std::mem::size_of::<u32>())
        .expect("Fix: split edge_targets resident allocation failed.");
    let edge_kind_handle = dispatcher
        .alloc_resident(edge_kind_mask.len() * std::mem::size_of::<u32>())
        .expect("Fix: split edge_kind_mask resident allocation failed.");
    let frontier_out_handle = dispatcher
        .alloc_resident(words * std::mem::size_of::<u32>())
        .expect("Fix: split frontier_out resident allocation failed.");
    let high_queue_handle = dispatcher
        .alloc_resident(high_queue_capacity as usize * std::mem::size_of::<u32>())
        .expect("Fix: split high_queue resident allocation failed.");
    let high_len_handle = dispatcher
        .alloc_resident(std::mem::size_of::<u32>())
        .expect("Fix: split high_len resident allocation failed.");

    let split_program = csr_queue_split_low_forward_traverse(
        "active_queue",
        "queue_len",
        "edge_offsets",
        "edge_targets",
        "edge_kind_mask",
        "frontier_out",
        "high_queue",
        "high_len",
        node_count,
        edge_targets.len() as u32,
        queue_capacity,
        high_queue_capacity,
        high_degree_threshold,
        allow_mask,
    );
    let high_program = csr_queue_strided_forward_traverse(
        "high_queue",
        "high_len",
        "edge_offsets",
        "edge_targets",
        "edge_kind_mask",
        "frontier_out",
        node_count,
        edge_targets.len() as u32,
        high_queue_capacity,
        allow_mask,
    );
    let split_handles = [
        active_queue_handle,
        queue_len_handle,
        edge_offsets_handle,
        edge_targets_handle,
        edge_kind_handle,
        frontier_out_handle,
        high_queue_handle,
        high_len_handle,
    ];
    let high_handles = [
        high_queue_handle,
        high_len_handle,
        edge_offsets_handle,
        edge_targets_handle,
        edge_kind_handle,
        frontier_out_handle,
    ];
    let steps = [
        ResidentDispatchStep {
            program: &split_program,
            handle_ids: &split_handles,
            grid_override: Some(csr_queue_split_low_dispatch_grid(queue_capacity)),
        },
        ResidentDispatchStep {
            program: &high_program,
            handle_ids: &high_handles,
            grid_override: Some(csr_queue_strided_forward_dispatch_grid(high_queue_capacity)),
        },
    ];

    let active_queue_bytes = u32_bytes(&active_queue);
    let queue_len_bytes = u32_bytes(&queue_len);
    let edge_offsets_bytes = u32_bytes(&edge_offsets);
    let edge_targets_bytes = u32_bytes(&edge_targets);
    let edge_kind_bytes = u32_bytes(&edge_kind_mask);
    let frontier_seed_bytes = u32_bytes(&frontier_seed);
    let high_queue_seed = vec![0_u8; high_queue_capacity as usize * std::mem::size_of::<u32>()];
    let high_len_seed = u32_bytes(&[0_u32]);
    let uploads = [
        (active_queue_handle, active_queue_bytes.as_slice()),
        (queue_len_handle, queue_len_bytes.as_slice()),
        (edge_offsets_handle, edge_offsets_bytes.as_slice()),
        (edge_targets_handle, edge_targets_bytes.as_slice()),
        (edge_kind_handle, edge_kind_bytes.as_slice()),
        (frontier_out_handle, frontier_seed_bytes.as_slice()),
        (high_queue_handle, high_queue_seed.as_slice()),
        (high_len_handle, high_len_seed.as_slice()),
    ];
    let outputs = dispatcher
        .upload_resident_many_sequence_read_ranges(
            &uploads,
            &steps,
            &[
                ResidentReadRange {
                    handle_id: frontier_out_handle,
                    byte_offset: 0,
                    byte_len: words * std::mem::size_of::<u32>(),
                },
                ResidentReadRange {
                    handle_id: high_queue_handle,
                    byte_offset: 0,
                    byte_len: high_queue_capacity as usize * std::mem::size_of::<u32>(),
                },
                ResidentReadRange {
                    handle_id: high_len_handle,
                    byte_offset: 0,
                    byte_len: std::mem::size_of::<u32>(),
                },
            ],
        )
        .expect("Fix: resident split-low plus high-row CSR queue sequence failed.");

    assert_eq!(bytes_u32(&outputs[0]), expected_full);
    assert_eq!(bytes_u32(&outputs[1]), split_expected.high_queue);
    assert_eq!(bytes_u32(&outputs[2]), vec![split_expected.high_len]);
    assert!(
        split_expected.high_len > high_queue_capacity,
        "Fix: this parity case must exercise high-queue overflow"
    );

    for handle in [
        active_queue_handle,
        queue_len_handle,
        edge_offsets_handle,
        edge_targets_handle,
        edge_kind_handle,
        frontier_out_handle,
        high_queue_handle,
        high_len_handle,
    ] {
        dispatcher
            .free_resident(handle)
            .expect("Fix: split resident cleanup failed.");
    }
}

fn generated_power_law_graph(node_count: u32) -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let mut offsets = Vec::with_capacity(node_count as usize + 1);
    let mut targets = Vec::new();
    let mut masks = Vec::new();
    offsets.push(0);
    for src in 0..node_count {
        let degree = match src {
            0 => 2048,
            1024 => 513,
            511 => 80,
            1536 => 257,
            7 => 3,
            31 => 9,
            1023 => 2,
            2047 => 1,
            _ if src % 257 == 0 => 5,
            _ => 0,
        };
        for edge in 0..degree {
            targets.push(src.wrapping_mul(19).wrapping_add(edge * 13 + 11) % node_count);
            masks.push(match edge % 6 {
                0 => 0,
                1 | 2 => 1,
                3 => 4,
                _ => 2,
            });
        }
        offsets.push(targets.len() as u32);
    }
    (offsets, targets, masks)
}

fn generated_frontier_seed(words: usize) -> Vec<u32> {
    (0..words)
        .map(|word| {
            if word % 17 == 0 {
                1_u32 << (word % 31)
            } else {
                0
            }
        })
        .collect()
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
