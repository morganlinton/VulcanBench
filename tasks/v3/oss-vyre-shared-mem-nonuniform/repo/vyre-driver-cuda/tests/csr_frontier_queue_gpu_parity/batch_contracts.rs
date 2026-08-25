use super::*;

#[test]
fn cuda_resident_csr_queue_batch_runs_many_queries_with_one_sync() {
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
    .expect("Fix: batched resident CSR queue graph upload failed.");
    let frontiers = [
        pack_nodes(&[0, 3], node_count),
        pack_nodes(&[3], node_count),
        pack_nodes(&[7], node_count),
    ];
    let frontier_refs: Vec<&[u32]> = frontiers.iter().map(Vec::as_slice).collect();
    let mut expected = Vec::new();
    for frontier in &frontiers {
        let (expected_queue, expected_len) =
            frontier_to_queue_cpu(frontier, node_count, queue_capacity as usize);
        expected.push(csr_queue_forward_traverse_cpu(
            &expected_queue,
            expected_len,
            &edge_offsets,
            &edge_targets,
            &edge_kind_mask,
            node_count,
            1,
        ));
    }

    let mut scratch = ResidentCsrQueueBatchScratch::default();
    let output_bytes = bitset_words(node_count) as usize * std::mem::size_of::<u32>();
    let mut outputs = vec![
        Vec::with_capacity(output_bytes),
        Vec::with_capacity(output_bytes),
        Vec::with_capacity(output_bytes),
    ];
    let output_ptrs: Vec<*const u8> = outputs.iter().map(Vec::as_ptr).collect();

    backend.reset_telemetry();
    run_resident_csr_queue_batch_into(
        &dispatcher,
        &graph,
        &mut scratch,
        &frontier_refs,
        queue_capacity,
        1,
        &mut outputs,
    )
    .expect("Fix: batched resident CSR queue execution failed on CUDA.");

    for ((output, expected_words), ptr) in outputs.iter().zip(&expected).zip(&output_ptrs) {
        assert_eq!(bytes_u32(output), *expected_words);
        assert_eq!(
            output.as_ptr(),
            *ptr,
            "Fix: batched resident CSR queue must preserve caller-owned output slots."
        );
    }
    let telemetry = backend.telemetry_snapshot();
    assert_eq!(
        telemetry.kernel_launches,
        (frontiers.len() * 3) as u64,
        "Fix: each batched CSR queue query should submit frontier clear, queue-build, and queue-consume kernels; frontier_to_queue clears queue_len itself."
    );
    assert_eq!(
        telemetry.sync_points, 1,
        "Fix: batched resident CSR queue must use one host fence for all queries."
    );
    assert_eq!(
        telemetry.readback_bytes,
        (frontiers.len() * output_bytes) as u64,
        "Fix: batched resident CSR queue must read only compact frontier outputs."
    );
    assert_eq!(
            telemetry
                .host_to_device_bytes
                .saturating_sub(telemetry.param_upload_bytes),
        (frontiers.len() * output_bytes) as u64,
        "Fix: batched resident CSR queue must upload only each frontier seed; queue length and frontier output are initialized on device."
    );
    assert_eq!(scratch.resident_query_slots(), frontiers.len());
    let retained_frontier_payload_capacity = scratch.frontier_payload_capacity();

    backend.reset_telemetry();
    run_resident_csr_queue_batch_into(
        &dispatcher,
        &graph,
        &mut scratch,
        &frontier_refs,
        queue_capacity,
        1,
        &mut outputs,
    )
    .expect("Fix: repeated batched resident CSR queue execution failed on CUDA.");
    assert_eq!(
        scratch.resident_query_slots(),
        frontiers.len(),
        "Fix: repeated batch execution must reuse resident per-query scratch slots."
    );
    assert_eq!(
        scratch.frontier_payload_capacity(),
        retained_frontier_payload_capacity,
        "Fix: repeated batch execution must reuse host frontier staging capacity."
    );
    for (output, ptr) in outputs.iter().zip(&output_ptrs) {
        assert_eq!(
            output.as_ptr(),
            *ptr,
            "Fix: repeated batched resident CSR queue must preserve caller-owned output slots."
        );
    }
    let telemetry = backend.telemetry_snapshot();
    assert_eq!(
        telemetry.sync_points, 1,
        "Fix: repeated batched resident CSR queue must still use one host fence."
    );
    assert_eq!(
        telemetry.readback_bytes,
        (frontiers.len() * output_bytes) as u64,
        "Fix: repeated batched resident CSR queue must read only compact frontier outputs."
    );

    scratch
        .free(&dispatcher)
        .expect("Fix: batched resident CSR queue scratch cleanup failed.");
    graph
        .free(&dispatcher)
        .expect("Fix: batched resident CSR queue graph cleanup failed.");
}

#[test]
fn cuda_resident_csr_queue_batch_splits_skewed_high_degree_rows() {
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
    .expect("Fix: skewed high-degree batched resident CSR queue graph upload failed.");
    let frontiers = [
        pack_nodes(&[0, 1, 2, 3, 4, 5, 6, 7, 8], node_count),
        pack_nodes(&[0, 2, 5, 8], node_count),
    ];
    let frontier_refs: Vec<&[u32]> = frontiers.iter().map(Vec::as_slice).collect();
    let mut expected = Vec::new();
    for frontier in &frontiers {
        let (expected_queue, expected_len) =
            frontier_to_queue_cpu(frontier, node_count, queue_capacity as usize);
        expected.push(csr_queue_forward_traverse_cpu(
            &expected_queue,
            expected_len,
            &edge_offsets,
            &edge_targets,
            &edge_kind_mask,
            node_count,
            1,
        ));
    }

    let mut scratch = ResidentCsrQueueBatchScratch::default();
    let output_bytes = bitset_words(node_count) as usize * std::mem::size_of::<u32>();
    let mut outputs = vec![
        Vec::with_capacity(output_bytes),
        Vec::with_capacity(output_bytes),
    ];
    let output_ptrs: Vec<*const u8> = outputs.iter().map(Vec::as_ptr).collect();

    backend.reset_telemetry();
    run_resident_csr_queue_batch_into(
        &dispatcher,
        &graph,
        &mut scratch,
        &frontier_refs,
        queue_capacity,
        1,
        &mut outputs,
    )
    .expect("Fix: skewed high-degree batched resident CSR queue execution failed on CUDA.");

    for ((output, expected_words), ptr) in outputs.iter().zip(&expected).zip(&output_ptrs) {
        assert_eq!(bytes_u32(output), *expected_words);
        assert_eq!(
            output.as_ptr(),
            *ptr,
            "Fix: skewed batched resident CSR queue must preserve caller-owned output slots."
        );
    }
    let telemetry = backend.telemetry_snapshot();
    assert_eq!(
        telemetry.kernel_launches,
        (frontiers.len() * 5) as u64,
        "Fix: each skewed batched CSR query must run queue_len init, queue build, high_len init, split-low, and bounded high-row traverse."
    );
    assert_eq!(
        telemetry.sync_points, 1,
        "Fix: skewed batched resident CSR queue must use one host fence for all queries."
    );
    assert_eq!(
        telemetry.readback_bytes,
        (frontiers.len() * output_bytes) as u64,
        "Fix: skewed batched resident CSR queue must read only compact frontier outputs."
    );
    assert_eq!(
        telemetry
            .host_to_device_bytes
            .saturating_sub(telemetry.param_upload_bytes),
        (frontiers.len() * output_bytes) as u64,
        "Fix: skewed batched resident CSR queue must upload only each frontier seed."
    );

    scratch
        .free(&dispatcher)
        .expect("Fix: skewed batched resident CSR queue scratch cleanup failed.");
    graph
        .free(&dispatcher)
        .expect("Fix: skewed batched resident CSR queue graph cleanup failed.");
}

#[test]
fn cuda_resident_csr_queue_budgeted_batch_shards_before_allocation() {
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
    .expect("Fix: budgeted resident CSR queue graph upload failed.");
    let frontiers = [
        pack_nodes(&[0, 3], node_count),
        pack_nodes(&[3], node_count),
        pack_nodes(&[7], node_count),
    ];
    let frontier_refs: Vec<&[u32]> = frontiers.iter().map(Vec::as_slice).collect();
    let mut scratch = ResidentCsrQueueBatchScratch::default();
    let output_bytes = bitset_words(node_count) as usize * std::mem::size_of::<u32>();
    let two_active_query_bytes =
        output_bytes + 2 * std::mem::size_of::<u32>() + std::mem::size_of::<u32>() + output_bytes;
    let mut outputs = Vec::new();

    backend.reset_telemetry();
    let plan = run_resident_csr_queue_batch_budgeted_into(
        &dispatcher,
        &graph,
        &mut scratch,
        &frontier_refs,
        queue_capacity,
        1,
        two_active_query_bytes * 2,
        &mut outputs,
    )
    .expect("Fix: budgeted resident CSR queue batch failed on CUDA.");

    assert_eq!(plan.max_queries_per_dispatch, 2);
    assert_eq!(plan.dispatch_batches, 2);
    assert_eq!(
        scratch.resident_query_slots(),
        2,
        "Fix: budgeted resident CSR queue must retain the larger scratch shard for the final smaller shard."
    );
    assert_eq!(
        outputs.len(),
        frontiers.len(),
        "Fix: budgeted resident CSR queue must preserve one output slot per query."
    );
    let telemetry = backend.telemetry_snapshot();
    assert_eq!(
        telemetry.sync_points, 2,
        "Fix: budgeted resident CSR queue must shard into exactly two host fences for this budget."
    );
    assert_eq!(
        telemetry.kernel_launches,
        (frontiers.len() * 3) as u64,
        "Fix: budgeted resident CSR queue must still run frontier clear, queue-build, and queue-consume per query."
    );
    assert_eq!(
        telemetry.readback_bytes,
        (frontiers.len() * output_bytes) as u64,
        "Fix: budgeted resident CSR queue must read only compact frontier outputs across shards."
    );

    scratch
        .free(&dispatcher)
        .expect("Fix: budgeted resident CSR queue scratch cleanup failed.");
    graph
        .free(&dispatcher)
        .expect("Fix: budgeted resident CSR queue graph cleanup failed.");
}
