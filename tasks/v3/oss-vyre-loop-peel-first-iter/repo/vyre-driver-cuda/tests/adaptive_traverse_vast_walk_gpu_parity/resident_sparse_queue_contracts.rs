use super::*;

#[test]
fn cuda_resident_adaptive_sparse_queue_path_uses_self_substrate_api() {
    let backend = live_dispatcher();
    let dispatcher = vyre_driver_cuda::CudaOptimizerDispatcher::new(&backend);
    let node_count = 8u32;
    let edge_offsets = vec![0, 2, 2, 2, 5, 5, 5, 5, 5];
    let edge_targets = vec![1, 2, 4, 5, 6];
    let edge_kind_mask = vec![1, 2, 1, 1, 2];
    let adj = build_dense_adj(&[(0, 7)], node_count);
    let graph = upload_resident_adaptive_traversal_graph(
        &dispatcher,
        node_count,
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
        &adj,
    )
    .expect("resident adaptive graph upload");
    let frontier_in = pack_nodes(&[0, 3], node_count);
    let expected = pack_nodes(&[1, 4, 5], node_count);
    let mut scratch = AdaptiveTraversalResidentScratch::default();
    let mut out = Vec::with_capacity(1);

    backend.reset_telemetry();
    adaptive_traverse_resident_graph_sparse_queue_step_with_scratch_into(
        &dispatcher,
        &graph,
        &frontier_in,
        1,
        &mut scratch,
        &mut out,
    )
    .expect("resident adaptive sparse queue path");
    assert_eq!(out, expected);
    let telemetry = backend.telemetry_snapshot();
    assert_eq!(
        telemetry.kernel_launches, 3,
        "Fix: self-substrate sparse queue traversal must launch exactly device frontier clear + queue-build + queue-consume kernels; frontier_to_queue clears queue_len itself."
    );
    assert_eq!(
        telemetry.sync_points, 1,
        "Fix: self-substrate sparse queue traversal must fence once for upload + three kernels + compact readback."
    );
    assert_eq!(
        telemetry.readback_bytes,
        (frontier_in.len() * std::mem::size_of::<u32>()) as u64,
        "Fix: self-substrate sparse queue traversal must not read back active queue or queue length."
    );
    scratch
        .free(&dispatcher)
        .expect("resident adaptive scratch free");
    graph
        .free(&dispatcher)
        .expect("resident adaptive graph free");
}

#[test]
fn cuda_resident_adaptive_sparse_queue_csr_only_upload_skips_dense_rows() {
    let backend = live_dispatcher();
    let dispatcher = vyre_driver_cuda::CudaOptimizerDispatcher::new(&backend);
    let node_count = 8u32;
    let edge_offsets = vec![0, 2, 2, 2, 5, 5, 5, 5, 5];
    let edge_targets = vec![1, 2, 4, 5, 6];
    let edge_kind_mask = vec![1, 2, 1, 1, 2];

    backend.reset_telemetry();
    let graph = upload_resident_adaptive_sparse_queue_graph(
        &dispatcher,
        node_count,
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
    )
    .expect("resident adaptive sparse queue CSR-only graph upload");
    let upload_telemetry = backend.telemetry_snapshot();
    assert_eq!(
        upload_telemetry.host_to_device_bytes,
        ((edge_offsets.len() + edge_targets.len() + edge_kind_mask.len())
            * std::mem::size_of::<u32>()) as u64,
        "Fix: CSR-only adaptive sparse queue upload must not upload dense adjacency rows."
    );

    let frontier_in = pack_nodes(&[0, 3], node_count);
    let expected = pack_nodes(&[1, 4, 5], node_count);
    let mut scratch = AdaptiveTraversalResidentScratch::default();
    let mut out = Vec::new();

    backend.reset_telemetry();
    adaptive_traverse_resident_sparse_queue_step_with_scratch_into(
        &dispatcher,
        &graph,
        &frontier_in,
        1,
        &mut scratch,
        &mut out,
    )
    .expect("resident adaptive sparse queue CSR-only path");
    assert_eq!(out, expected);
    let telemetry = backend.telemetry_snapshot();
    assert_eq!(telemetry.kernel_launches, 3);
    assert_eq!(
        telemetry
            .host_to_device_bytes
            .saturating_sub(telemetry.param_upload_bytes),
        (frontier_in.len() * std::mem::size_of::<u32>()) as u64,
        "Fix: CSR-only adaptive sparse queue step must upload only the packed frontier."
    );
    scratch
        .free(&dispatcher)
        .expect("resident adaptive scratch free");
    graph
        .free(&dispatcher)
        .expect("resident adaptive sparse queue graph free");
}

#[test]
fn cuda_resident_adaptive_sparse_queue_word_prefix_handles_large_frontier() {
    let backend = live_dispatcher();
    let dispatcher = vyre_driver_cuda::CudaOptimizerDispatcher::new(&backend);
    let node_count = 9_000u32;
    let mut edge_offsets = Vec::with_capacity(node_count as usize + 1);
    let mut edge_targets = Vec::with_capacity(node_count as usize);
    let mut edge_kind_mask = Vec::with_capacity(node_count as usize);
    edge_offsets.push(0);
    for src in 0..node_count {
        edge_targets.push(src.wrapping_mul(17).wrapping_add(13) % node_count);
        edge_kind_mask.push(if src % 11 == 0 { 2 } else { 1 });
        edge_offsets.push(edge_targets.len() as u32);
    }
    let adj = build_dense_adj(&[(0, 64)], node_count);
    let graph = upload_resident_adaptive_traversal_graph(
        &dispatcher,
        node_count,
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
        &adj,
    )
    .expect("resident adaptive graph upload");
    let mut active_nodes = Vec::with_capacity(513);
    for word in 0..256u32 {
        active_nodes.push(word * 32);
        active_nodes.push(word * 32 + 1);
    }
    active_nodes.push(256 * 32);
    let frontier_in = pack_nodes(&active_nodes, node_count);
    let expected_nodes: Vec<u32> = active_nodes
        .iter()
        .copied()
        .filter(|src| src % 11 != 0)
        .map(|src| src.wrapping_mul(17).wrapping_add(13) % node_count)
        .collect();
    let expected = pack_nodes(&expected_nodes, node_count);
    let mut scratch = AdaptiveTraversalResidentScratch::default();
    let mut out = Vec::new();

    backend.reset_telemetry();
    adaptive_traverse_resident_graph_sparse_queue_step_with_scratch_into(
        &dispatcher,
        &graph,
        &frontier_in,
        1,
        &mut scratch,
        &mut out,
    )
    .expect("resident adaptive large sparse queue path");

    assert_eq!(out, expected);
    let telemetry = backend.telemetry_snapshot();
    assert_eq!(
        telemetry.kernel_launches, 4,
        "Fix: large adaptive sparse queue traversal must run clear, word-scan, deterministic queue scatter, and queue-consume kernels."
    );
    assert_eq!(
        telemetry
            .host_to_device_bytes
            .saturating_sub(telemetry.param_upload_bytes),
        (frontier_in.len() * std::mem::size_of::<u32>()) as u64,
        "Fix: large adaptive sparse queue traversal must upload only the packed frontier."
    );
    assert_eq!(
        telemetry.readback_bytes,
        (frontier_in.len() * std::mem::size_of::<u32>()) as u64,
        "Fix: large adaptive sparse queue traversal must read back only frontier_out."
    );
    scratch
        .free(&dispatcher)
        .expect("resident adaptive scratch free");
    graph
        .free(&dispatcher)
        .expect("resident adaptive graph free");
}

