//! CUDA parity for row-strided queue-driven CSR expansion.

#![cfg(test)]

mod common;

use common::{bytes_u32, live_backend, u32_bytes};
use vyre_driver_cuda::CudaOptimizerDispatcher;
use vyre_primitives::bitset::bitset_words;
use vyre_primitives::graph::csr_queue_strided::{
    csr_queue_strided_forward_dispatch_grid, csr_queue_strided_forward_traverse,
    csr_queue_strided_forward_traverse_cpu,
};
use vyre_self_substrate::optimizer::dispatcher::{
    OptimizerDispatcher, ResidentDispatchStep, ResidentReadRange,
};

#[test]
fn cuda_csr_queue_strided_forward_matches_cpu_on_skewed_row() {
    let backend = live_backend();
    let dispatcher = CudaOptimizerDispatcher::new(&backend);
    let node_count = 1024u32;
    let queue_capacity = 9u32;
    let active_queue = vec![0, 7, 1023, 0, 0, 0, 0, 0, 0];
    let queue_len = [3u32];
    let mut edge_offsets = Vec::with_capacity(node_count as usize + 1);
    let mut edge_targets = Vec::new();
    let mut edge_kind_mask = Vec::new();
    edge_offsets.push(0);
    for src in 0..node_count {
        let degree = match src {
            0 => 4096,
            7 => 3,
            1023 => 2,
            _ if src % 127 == 0 => 1,
            _ => 0,
        };
        for edge in 0..degree {
            edge_targets.push(src.wrapping_mul(5).wrapping_add(edge * 13 + 11) % node_count);
            edge_kind_mask.push(if edge % 4 == 0 { 2 } else { 1 });
        }
        edge_offsets.push(edge_targets.len() as u32);
    }
    let expected_out = csr_queue_strided_forward_traverse_cpu(
        &active_queue,
        queue_len[0],
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
        node_count,
        1,
    );
    let words = bitset_words(node_count) as usize;

    let active_queue_handle = dispatcher
        .alloc_resident(active_queue.len() * std::mem::size_of::<u32>())
        .expect("Fix: strided active_queue resident allocation failed.");
    let queue_len_handle = dispatcher
        .alloc_resident(std::mem::size_of::<u32>())
        .expect("Fix: strided queue_len resident allocation failed.");
    let edge_offsets_handle = dispatcher
        .alloc_resident(edge_offsets.len() * std::mem::size_of::<u32>())
        .expect("Fix: strided edge_offsets resident allocation failed.");
    let edge_targets_handle = dispatcher
        .alloc_resident(edge_targets.len() * std::mem::size_of::<u32>())
        .expect("Fix: strided edge_targets resident allocation failed.");
    let edge_kind_handle = dispatcher
        .alloc_resident(edge_kind_mask.len() * std::mem::size_of::<u32>())
        .expect("Fix: strided edge_kind_mask resident allocation failed.");
    let frontier_out_handle = dispatcher
        .alloc_resident(words * std::mem::size_of::<u32>())
        .expect("Fix: strided frontier_out resident allocation failed.");

    let program = csr_queue_strided_forward_traverse(
        "active_queue",
        "queue_len",
        "edge_offsets",
        "edge_targets",
        "edge_kind_mask",
        "frontier_out",
        node_count,
        edge_targets.len() as u32,
        queue_capacity,
        1,
    );
    let handles = [
        active_queue_handle,
        queue_len_handle,
        edge_offsets_handle,
        edge_targets_handle,
        edge_kind_handle,
        frontier_out_handle,
    ];
    let steps = [ResidentDispatchStep {
        program: &program,
        handle_ids: &handles,
        grid_override: Some(csr_queue_strided_forward_dispatch_grid(queue_capacity)),
    }];
    let zero_frontier_out = vec![0u8; words * std::mem::size_of::<u32>()];
    let active_queue_bytes = u32_bytes(&active_queue);
    let queue_len_bytes = u32_bytes(&queue_len);
    let edge_offsets_bytes = u32_bytes(&edge_offsets);
    let edge_targets_bytes = u32_bytes(&edge_targets);
    let edge_kind_bytes = u32_bytes(&edge_kind_mask);
    let uploads = [
        (active_queue_handle, active_queue_bytes.as_slice()),
        (queue_len_handle, queue_len_bytes.as_slice()),
        (edge_offsets_handle, edge_offsets_bytes.as_slice()),
        (edge_targets_handle, edge_targets_bytes.as_slice()),
        (edge_kind_handle, edge_kind_bytes.as_slice()),
        (frontier_out_handle, zero_frontier_out.as_slice()),
    ];
    let outputs = dispatcher
        .upload_resident_many_sequence_read_ranges(
            &uploads,
            &steps,
            &[ResidentReadRange {
                handle_id: frontier_out_handle,
                byte_offset: 0,
                byte_len: words * std::mem::size_of::<u32>(),
            }],
        )
        .expect("Fix: strided CSR queue resident sequence failed.");

    assert_eq!(bytes_u32(&outputs[0]), expected_out);

    for handle in [
        active_queue_handle,
        queue_len_handle,
        edge_offsets_handle,
        edge_targets_handle,
        edge_kind_handle,
        frontier_out_handle,
    ] {
        dispatcher
            .free_resident(handle)
            .expect("Fix: strided resident cleanup failed.");
    }
}
