use super::*;

#[test]
fn cuda_resident_frontier_queue_drives_sparse_csr_without_selector_readback() {
    let backend = live_backend();
    let dispatcher = CudaOptimizerDispatcher::new(&backend);
    let node_count = 8u32;
    let queue_capacity = 8u32;
    let frontier = pack_nodes(&[0, 3], node_count);
    let edge_offsets = vec![0, 2, 2, 2, 5, 5, 5, 5, 5];
    let edge_targets = vec![1, 2, 4, 5, 6];
    let edge_kind_mask = vec![1, 2, 1, 1, 2];
    let (expected_queue, expected_len) =
        frontier_to_queue_cpu(&frontier, node_count, queue_capacity as usize);
    let expected_out = csr_queue_forward_traverse_cpu(
        &expected_queue,
        expected_len,
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
        node_count,
        1,
    );

    let frontier_handle = dispatcher
        .alloc_resident(frontier.len() * std::mem::size_of::<u32>())
        .expect("Fix: frontier resident allocation failed.");
    let queue_handle = dispatcher
        .alloc_resident(queue_capacity as usize * std::mem::size_of::<u32>())
        .expect("Fix: queue resident allocation failed.");
    let queue_len_handle = dispatcher
        .alloc_resident(std::mem::size_of::<u32>())
        .expect("Fix: queue_len resident allocation failed.");
    let edge_offsets_handle = dispatcher
        .alloc_resident(edge_offsets.len() * std::mem::size_of::<u32>())
        .expect("Fix: edge_offsets resident allocation failed.");
    let edge_targets_handle = dispatcher
        .alloc_resident(edge_targets.len() * std::mem::size_of::<u32>())
        .expect("Fix: edge_targets resident allocation failed.");
    let edge_kind_handle = dispatcher
        .alloc_resident(edge_kind_mask.len() * std::mem::size_of::<u32>())
        .expect("Fix: edge_kind_mask resident allocation failed.");
    let frontier_out_handle = dispatcher
        .alloc_resident(frontier.len() * std::mem::size_of::<u32>())
        .expect("Fix: frontier_out resident allocation failed.");

    let queue_program = frontier_to_queue(
        "frontier",
        "active_queue",
        "queue_len",
        node_count,
        queue_capacity,
    );
    let traverse_program = csr_queue_forward_traverse(
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
    let queue_handles = [frontier_handle, queue_handle, queue_len_handle];
    let traverse_handles = [
        queue_handle,
        queue_len_handle,
        edge_offsets_handle,
        edge_targets_handle,
        edge_kind_handle,
        frontier_out_handle,
    ];
    let steps = [
        ResidentDispatchStep {
            program: &queue_program,
            handle_ids: &queue_handles,
            grid_override: Some([node_count.div_ceil(256).max(1), 1, 1]),
        },
        ResidentDispatchStep {
            program: &traverse_program,
            handle_ids: &traverse_handles,
            grid_override: Some([queue_capacity.div_ceil(256).max(1), 1, 1]),
        },
    ];
    let zero_queue = vec![0u8; queue_capacity as usize * std::mem::size_of::<u32>()];
    let zero_count = vec![0u8; std::mem::size_of::<u32>()];
    let zero_frontier_out = vec![0u8; frontier.len() * std::mem::size_of::<u32>()];
    let frontier_bytes = u32_bytes(&frontier);
    let edge_offsets_bytes = u32_bytes(&edge_offsets);
    let edge_targets_bytes = u32_bytes(&edge_targets);
    let edge_kind_bytes = u32_bytes(&edge_kind_mask);
    let uploads = [
        (frontier_handle, frontier_bytes.as_slice()),
        (queue_handle, zero_queue.as_slice()),
        (queue_len_handle, zero_count.as_slice()),
        (edge_offsets_handle, edge_offsets_bytes.as_slice()),
        (edge_targets_handle, edge_targets_bytes.as_slice()),
        (edge_kind_handle, edge_kind_bytes.as_slice()),
        (frontier_out_handle, zero_frontier_out.as_slice()),
    ];

    backend.reset_telemetry();
    let read_ranges = [
        ResidentReadRange {
            handle_id: frontier_out_handle,
            byte_offset: 0,
            byte_len: frontier.len() * std::mem::size_of::<u32>(),
        },
        ResidentReadRange {
            handle_id: queue_len_handle,
            byte_offset: 0,
            byte_len: std::mem::size_of::<u32>(),
        },
    ];
    let outputs = dispatcher
        .upload_resident_many_sequence_read_ranges(&uploads, &steps, &read_ranges)
        .expect("Fix: resident queue sparse traversal sequence failed.");
    assert_eq!(bytes_u32(&outputs[0]), expected_out);
    assert_eq!(bytes_u32(&outputs[1]), vec![expected_len]);
    let telemetry = backend.telemetry_snapshot();
    assert_eq!(
        telemetry.kernel_launches, 2,
        "Fix: queue sparse traversal must be exactly queue-build + queue-consume kernels."
    );
    assert_eq!(
        telemetry.sync_points, 1,
        "Fix: resident queue sparse traversal must fence once for uploads, kernels, and compact readbacks."
    );
    assert_eq!(
        telemetry.readback_bytes,
        (frontier.len() * std::mem::size_of::<u32>() + std::mem::size_of::<u32>()) as u64,
        "Fix: queue sparse traversal readback must be compact and avoid queue payload D2H."
    );

    for handle in [
        frontier_handle,
        queue_handle,
        queue_len_handle,
        edge_offsets_handle,
        edge_targets_handle,
        edge_kind_handle,
        frontier_out_handle,
    ] {
        dispatcher
            .free_resident(handle)
            .expect("Fix: resident queue sparse traversal cleanup failed.");
    }
}

#[test]
fn cuda_resident_parallel_frontier_queue_scans_large_sparse_bitset() {
    let backend = live_backend();
    let dispatcher = CudaOptimizerDispatcher::new(&backend);
    let node_count = 1024u32;
    let queue_capacity = 16u32;
    let frontier = pack_nodes(&[0, 3, 511, 700], node_count);
    let mut edge_offsets = Vec::with_capacity(node_count as usize + 1);
    let mut edge_targets = Vec::with_capacity(node_count as usize);
    let mut edge_kind_mask = Vec::with_capacity(node_count as usize);
    edge_offsets.push(0);
    for src in 0..node_count {
        edge_targets.push((src.wrapping_mul(17).wrapping_add(9)) % node_count);
        edge_kind_mask.push(if src % 5 == 0 { 2 } else { 1 });
        edge_offsets.push(edge_targets.len() as u32);
    }
    let (expected_queue, expected_len) =
        frontier_to_queue_cpu(&frontier, node_count, queue_capacity as usize);
    let expected_out = csr_queue_forward_traverse_cpu(
        &expected_queue,
        expected_len,
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
        node_count,
        1,
    );

    let frontier_handle = dispatcher
        .alloc_resident(frontier.len() * std::mem::size_of::<u32>())
        .expect("Fix: frontier resident allocation failed.");
    let queue_handle = dispatcher
        .alloc_resident(queue_capacity as usize * std::mem::size_of::<u32>())
        .expect("Fix: queue resident allocation failed.");
    let queue_len_handle = dispatcher
        .alloc_resident(std::mem::size_of::<u32>())
        .expect("Fix: queue_len resident allocation failed.");
    let edge_offsets_handle = dispatcher
        .alloc_resident(edge_offsets.len() * std::mem::size_of::<u32>())
        .expect("Fix: edge_offsets resident allocation failed.");
    let edge_targets_handle = dispatcher
        .alloc_resident(edge_targets.len() * std::mem::size_of::<u32>())
        .expect("Fix: edge_targets resident allocation failed.");
    let edge_kind_handle = dispatcher
        .alloc_resident(edge_kind_mask.len() * std::mem::size_of::<u32>())
        .expect("Fix: edge_kind_mask resident allocation failed.");
    let frontier_out_handle = dispatcher
        .alloc_resident(frontier.len() * std::mem::size_of::<u32>())
        .expect("Fix: frontier_out resident allocation failed.");

    let queue_program = frontier_to_queue_parallel(
        "frontier",
        "active_queue",
        "queue_len",
        node_count,
        queue_capacity,
    );
    let traverse_program = csr_queue_forward_traverse(
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
    let queue_handles = [frontier_handle, queue_handle, queue_len_handle];
    let traverse_handles = [
        queue_handle,
        queue_len_handle,
        edge_offsets_handle,
        edge_targets_handle,
        edge_kind_handle,
        frontier_out_handle,
    ];
    let steps = [
        ResidentDispatchStep {
            program: &queue_program,
            handle_ids: &queue_handles,
            grid_override: Some([node_count.div_ceil(256).max(1), 1, 1]),
        },
        ResidentDispatchStep {
            program: &traverse_program,
            handle_ids: &traverse_handles,
            grid_override: Some([queue_capacity.div_ceil(256).max(1), 1, 1]),
        },
    ];
    let zero_queue = vec![0u8; queue_capacity as usize * std::mem::size_of::<u32>()];
    let zero_count = vec![0u8; std::mem::size_of::<u32>()];
    let zero_frontier_out = vec![0u8; frontier.len() * std::mem::size_of::<u32>()];
    let frontier_bytes = u32_bytes(&frontier);
    let edge_offsets_bytes = u32_bytes(&edge_offsets);
    let edge_targets_bytes = u32_bytes(&edge_targets);
    let edge_kind_bytes = u32_bytes(&edge_kind_mask);
    let uploads = [
        (frontier_handle, frontier_bytes.as_slice()),
        (queue_handle, zero_queue.as_slice()),
        (queue_len_handle, zero_count.as_slice()),
        (edge_offsets_handle, edge_offsets_bytes.as_slice()),
        (edge_targets_handle, edge_targets_bytes.as_slice()),
        (edge_kind_handle, edge_kind_bytes.as_slice()),
        (frontier_out_handle, zero_frontier_out.as_slice()),
    ];
    let read_ranges = [
        ResidentReadRange {
            handle_id: frontier_out_handle,
            byte_offset: 0,
            byte_len: frontier.len() * std::mem::size_of::<u32>(),
        },
        ResidentReadRange {
            handle_id: queue_len_handle,
            byte_offset: 0,
            byte_len: std::mem::size_of::<u32>(),
        },
    ];

    backend.reset_telemetry();
    let outputs = dispatcher
        .upload_resident_many_sequence_read_ranges(&uploads, &steps, &read_ranges)
        .expect("Fix: resident parallel queue sparse traversal sequence failed.");

    assert_eq!(bytes_u32(&outputs[0]), expected_out);
    assert_eq!(bytes_u32(&outputs[1]), vec![expected_len]);
    let telemetry = backend.telemetry_snapshot();
    assert_eq!(
        telemetry.kernel_launches, 2,
        "Fix: parallel queue traversal should be queue-build + queue-consume kernels after setup uploads."
    );
    assert_eq!(
        telemetry.readback_bytes,
        (frontier.len() * std::mem::size_of::<u32>() + std::mem::size_of::<u32>()) as u64
    );

    for handle in [
        frontier_handle,
        queue_handle,
        queue_len_handle,
        edge_offsets_handle,
        edge_targets_handle,
        edge_kind_handle,
        frontier_out_handle,
    ] {
        dispatcher
            .free_resident(handle)
            .expect("Fix: resident parallel queue traversal cleanup failed.");
    }
}

