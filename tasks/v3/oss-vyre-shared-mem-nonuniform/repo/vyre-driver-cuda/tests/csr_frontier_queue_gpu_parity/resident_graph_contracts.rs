use super::*;

#[test]
fn cuda_resident_frontier_queue_reuses_static_graph_across_queries() {
    let backend = live_backend();
    let dispatcher = CudaOptimizerDispatcher::new(&backend);
    let node_count = 8u32;
    let queue_capacity = 8u32;
    let edge_offsets = vec![0, 2, 2, 2, 5, 5, 5, 5, 5];
    let edge_targets = vec![1, 2, 4, 5, 6];
    let edge_kind_mask = vec![1, 2, 1, 1, 2];
    let frontier_words = bitset_words(node_count) as usize;

    let frontier_handle = dispatcher
        .alloc_resident(frontier_words * std::mem::size_of::<u32>())
        .expect("Fix: resident queue reuse frontier allocation failed.");
    let queue_handle = dispatcher
        .alloc_resident(queue_capacity as usize * std::mem::size_of::<u32>())
        .expect("Fix: resident queue reuse queue allocation failed.");
    let queue_len_handle = dispatcher
        .alloc_resident(std::mem::size_of::<u32>())
        .expect("Fix: resident queue reuse queue_len allocation failed.");
    let edge_offsets_handle = dispatcher
        .alloc_resident(edge_offsets.len() * std::mem::size_of::<u32>())
        .expect("Fix: resident queue reuse edge_offsets allocation failed.");
    let edge_targets_handle = dispatcher
        .alloc_resident(edge_targets.len() * std::mem::size_of::<u32>())
        .expect("Fix: resident queue reuse edge_targets allocation failed.");
    let edge_kind_handle = dispatcher
        .alloc_resident(edge_kind_mask.len() * std::mem::size_of::<u32>())
        .expect("Fix: resident queue reuse edge_kind_mask allocation failed.");
    let frontier_out_handle = dispatcher
        .alloc_resident(frontier_words * std::mem::size_of::<u32>())
        .expect("Fix: resident queue reuse frontier_out allocation failed.");

    let edge_offsets_bytes = u32_bytes(&edge_offsets);
    let edge_targets_bytes = u32_bytes(&edge_targets);
    let edge_kind_bytes = u32_bytes(&edge_kind_mask);
    dispatcher
        .upload_resident_many(&[
            (edge_offsets_handle, edge_offsets_bytes.as_slice()),
            (edge_targets_handle, edge_targets_bytes.as_slice()),
            (edge_kind_handle, edge_kind_bytes.as_slice()),
        ])
        .expect("Fix: static CSR graph must upload once before repeated queue queries.");

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
    let read_ranges = [ResidentReadRange {
        handle_id: frontier_out_handle,
        byte_offset: 0,
        byte_len: frontier_words * std::mem::size_of::<u32>(),
    }];
    let zero_count = vec![0u8; std::mem::size_of::<u32>()];
    let zero_frontier_out = vec![0u8; frontier_words * std::mem::size_of::<u32>()];

    for active_nodes in [&[0, 3][..], &[3][..]] {
        let frontier = pack_nodes(active_nodes, node_count);
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
        let frontier_bytes = u32_bytes(&frontier);

        backend.reset_telemetry();
        let outputs = dispatcher
            .upload_resident_many_sequence_read_ranges(
                &[
                    (frontier_handle, frontier_bytes.as_slice()),
                    (queue_len_handle, zero_count.as_slice()),
                    (frontier_out_handle, zero_frontier_out.as_slice()),
                ],
                &steps,
                &read_ranges,
            )
            .expect("Fix: resident static-graph queue query must run without reuploading CSR graph state.");

        assert_eq!(bytes_u32(&outputs[0]), expected_out);
        let telemetry = backend.telemetry_snapshot();
        assert_eq!(telemetry.kernel_launches, 2);
        assert_eq!(telemetry.sync_points, 1);
        assert_eq!(
            telemetry.readback_bytes,
            (frontier_words * std::mem::size_of::<u32>()) as u64,
            "Fix: repeated resident queue query must read back only frontier_out, not queue payload or selector count."
        );
        assert_eq!(
            telemetry
                .host_to_device_bytes
                .saturating_sub(telemetry.param_upload_bytes),
            (frontier_bytes.len() + zero_count.len() + zero_frontier_out.len()) as u64,
            "Fix: repeated resident queue query must refresh only frontier/scratch/output buffers and keep CSR graph state resident."
        );
        assert!(
            telemetry.host_upload_operations <= 5,
            "Fix: repeated resident queue query must issue only frontier/scratch/output data uploads plus cached parameter uploads, not CSR graph uploads; observed {} upload operations.",
            telemetry.host_upload_operations
        );
    }

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
            .expect("Fix: resident queue reuse cleanup failed.");
    }
}

#[test]
fn cuda_resident_csr_queue_api_reuses_graph_and_scratch() {
    let backend = live_backend();
    let dispatcher = CudaOptimizerDispatcher::new(&backend);
    let node_count = 8u32;
    let queue_capacity = 8u32;
    let edge_offsets = vec![0, 2, 2, 2, 5, 5, 5, 5, 5];
    let edge_targets = vec![1, 2, 4, 5, 6];
    let edge_kind_mask = vec![1, 2, 1, 1, 2];
    let graph = upload_resident_csr_queue_graph(
        &dispatcher,
        node_count,
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
    )
    .expect("Fix: reusable resident CSR queue graph upload failed.");
    let mut scratch = ResidentCsrQueueScratch::default();
    let mut output =
        Vec::with_capacity(bitset_words(node_count) as usize * std::mem::size_of::<u32>());
    let output_ptr = output.as_ptr();

    for active_nodes in [&[0, 3][..], &[3][..]] {
        let frontier = pack_nodes(active_nodes, node_count);
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

        backend.reset_telemetry();
        run_resident_csr_queue_query_into(
            &dispatcher,
            &graph,
            &mut scratch,
            &frontier,
            queue_capacity,
            1,
            &mut output,
        )
        .expect("Fix: reusable resident CSR queue query failed on CUDA.");

        assert_eq!(bytes_u32(&output), expected_out);
        assert_eq!(
            output.as_ptr(),
            output_ptr,
            "Fix: resident CSR queue API must preserve caller-owned output capacity."
        );
        let telemetry = backend.telemetry_snapshot();
        assert_eq!(telemetry.kernel_launches, 3);
        assert_eq!(telemetry.sync_points, 1);
        assert_eq!(
            telemetry.readback_bytes,
            output.len() as u64,
            "Fix: resident CSR queue API must compact readback to frontier_out only."
        );
        assert_eq!(
            telemetry
                .host_to_device_bytes
                .saturating_sub(telemetry.param_upload_bytes),
            (frontier.len() * std::mem::size_of::<u32>()) as u64,
            "Fix: resident CSR queue API must upload only the frontier seed; queue length and frontier output are initialized on device."
        );
    }

    scratch
        .free(&dispatcher)
        .expect("Fix: resident CSR queue scratch cleanup failed.");
    graph
        .free(&dispatcher)
        .expect("Fix: resident CSR queue graph cleanup failed.");
}

#[test]
fn cuda_resident_csr_queue_uses_atomic_word_scan_for_large_sparse_frontier() {
    let backend = live_backend();
    let dispatcher = CudaOptimizerDispatcher::new(&backend);
    let node_count = 9_000u32;
    let queue_capacity = 16u32;
    let mut edge_offsets = Vec::with_capacity(node_count as usize + 1);
    let mut edge_targets = Vec::with_capacity(node_count as usize);
    let mut edge_kind_mask = Vec::with_capacity(node_count as usize);
    edge_offsets.push(0);
    for src in 0..node_count {
        edge_targets.push(src.wrapping_mul(17).wrapping_add(13) % node_count);
        edge_kind_mask.push(if src % 11 == 0 { 2 } else { 1 });
        edge_offsets.push(edge_targets.len() as u32);
    }
    let graph = upload_resident_csr_queue_graph(
        &dispatcher,
        node_count,
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
    )
    .expect("Fix: large resident CSR queue graph upload failed.");
    let frontier = pack_nodes(&[0, 3, 511, 7_000, 8_999], node_count);
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
    let mut scratch = ResidentCsrQueueScratch::default();
    let mut output =
        Vec::with_capacity(bitset_words(node_count) as usize * std::mem::size_of::<u32>());

    backend.reset_telemetry();
    run_resident_csr_queue_query_into(
        &dispatcher,
        &graph,
        &mut scratch,
        &frontier,
        queue_capacity,
        1,
        &mut output,
    )
    .expect("Fix: large sparse resident CSR queue query failed on CUDA.");

    assert_eq!(bytes_u32(&output), expected_out);
    let telemetry = backend.telemetry_snapshot();
    assert_eq!(
        telemetry.kernel_launches, 3,
        "Fix: sparse resident CSR queue should run clear, atomic word queue-build, and traverse kernels; deterministic word-prefix is reserved for dense high-capacity frontiers."
    );
    assert_eq!(
        telemetry
            .host_to_device_bytes
            .saturating_sub(telemetry.param_upload_bytes),
        (frontier.len() * std::mem::size_of::<u32>()) as u64,
        "Fix: large sparse resident CSR queue must upload only the packed frontier; queue scratch stays device-side."
    );

    scratch
        .free(&dispatcher)
        .expect("Fix: large sparse resident CSR queue scratch cleanup failed.");
    graph
        .free(&dispatcher)
        .expect("Fix: large sparse resident CSR queue graph cleanup failed.");
}

#[test]
fn cuda_resident_csr_queue_api_splits_skewed_high_degree_rows() {
    let backend = live_backend();
    let dispatcher = CudaOptimizerDispatcher::new(&backend);
    let node_count = 64u32;
    let queue_capacity = 1024u32;
    let (edge_offsets, edge_targets, edge_kind_mask) = skewed_high_degree_graph(node_count);
    let graph = upload_resident_csr_queue_graph(
        &dispatcher,
        node_count,
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
    )
    .expect("Fix: skewed high-degree resident CSR queue graph upload failed.");
    let frontier = pack_nodes(&[0, 1, 2, 3, 4, 5, 6, 7, 8], node_count);
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
    let mut scratch = ResidentCsrQueueScratch::default();
    let mut output =
        Vec::with_capacity(bitset_words(node_count) as usize * std::mem::size_of::<u32>());

    backend.reset_telemetry();
    run_resident_csr_queue_query_into(
        &dispatcher,
        &graph,
        &mut scratch,
        &frontier,
        queue_capacity,
        1,
        &mut output,
    )
    .expect("Fix: skewed high-degree resident CSR queue query failed on CUDA.");

    assert_eq!(bytes_u32(&output), expected_out);
    let telemetry = backend.telemetry_snapshot();
    assert_eq!(
        telemetry.kernel_launches, 5,
        "Fix: skewed resident CSR queue query must run queue_len init, queue build, high_len init, split-low, and bounded high-row traverse."
    );
    assert_eq!(telemetry.sync_points, 1);
    assert_eq!(
        telemetry.readback_bytes,
        output.len() as u64,
        "Fix: skewed resident CSR queue query must read back only frontier_out."
    );
    assert_eq!(
        telemetry
            .host_to_device_bytes
            .saturating_sub(telemetry.param_upload_bytes),
        (frontier.len() * std::mem::size_of::<u32>()) as u64,
        "Fix: skewed resident CSR queue query must upload only the packed frontier seed."
    );

    scratch
        .free(&dispatcher)
        .expect("Fix: skewed resident CSR queue scratch cleanup failed.");
    graph
        .free(&dispatcher)
        .expect("Fix: skewed resident CSR queue graph cleanup failed.");
}

