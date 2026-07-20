use super::*;

#[test]
fn cuda_resident_adaptive_sparse_dense_keeps_selector_on_device_sparse_branch() {
    let backend = live_dispatcher();
    let dispatcher = vyre_driver_cuda::CudaOptimizerDispatcher::new(&backend);
    let node_count = 8u32;
    let edge_offsets = vec![0, 1, 1, 1, 1, 1, 1, 1, 1];
    let edge_targets = vec![1];
    let edge_kind_mask = vec![1];
    let adj = build_dense_adj(&[(0, 2)], node_count);
    let graph = upload_resident_adaptive_traversal_graph(
        &dispatcher,
        node_count,
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
        &adj,
    )
    .expect("resident adaptive graph upload");
    let frontier_in = pack_nodes(&[0], node_count);
    let expected = adaptive_traverse_step(
        node_count,
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
        &adj,
        &frontier_in,
        1,
        50,
    )
    .expect("Fix: CPU adaptive traversal oracle must accept the sparse-branch fixture.");
    let mut scratch = AdaptiveTraversalResidentScratch::default();
    let mut out = Vec::with_capacity(1);
    backend.reset_telemetry();
    adaptive_traverse_resident_graph_step_with_scratch_into(
        &dispatcher,
        &graph,
        &frontier_in,
        1,
        50,
        &mut scratch,
        &mut out,
    )
    .expect("resident adaptive sparse branch");
    assert_eq!(out, expected);
    assert_eq!(out, pack_nodes(&[1], node_count));
    let telemetry = backend.telemetry_snapshot();
    assert_eq!(
        telemetry.kernel_launches, 3,
        "Fix: resident adaptive traversal must launch exactly reduce_count + device frontier clear + sparse/dense traversal."
    );
    assert_eq!(
        telemetry.sync_points, 1,
        "Fix: resident adaptive traversal must fence once for upload + two kernels + compact readback."
    );
    assert_eq!(
        telemetry.readback_bytes,
        (frontier_in.len() * std::mem::size_of::<u32>()) as u64,
        "Fix: resident adaptive traversal must not read back the selector count; only frontier_out is a release-path D2H payload."
    );
    scratch
        .free(&dispatcher)
        .expect("resident adaptive scratch free");
    graph
        .free(&dispatcher)
        .expect("resident adaptive graph free");
}

#[test]
fn cuda_resident_adaptive_sparse_dense_keeps_selector_on_device_dense_branch() {
    let backend = live_dispatcher();
    let dispatcher = vyre_driver_cuda::CudaOptimizerDispatcher::new(&backend);
    let node_count = 8u32;
    let edge_offsets = vec![0, 1, 1, 1, 1, 1, 1, 1, 1];
    let edge_targets = vec![1];
    let edge_kind_mask = vec![1];
    let adj = build_dense_adj(&[(0, 5)], node_count);
    let graph = upload_resident_adaptive_traversal_graph(
        &dispatcher,
        node_count,
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
        &adj,
    )
    .expect("resident adaptive graph upload");
    let frontier_in = pack_nodes(&[0, 1, 2, 3], node_count);
    let expected = adaptive_traverse_step(
        node_count,
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
        &adj,
        &frontier_in,
        1,
        50,
    )
    .expect("Fix: CPU adaptive traversal oracle must accept the dense-branch fixture.");
    let mut scratch = AdaptiveTraversalResidentScratch::default();
    let mut out = Vec::with_capacity(1);
    backend.reset_telemetry();
    adaptive_traverse_resident_graph_step_with_scratch_into(
        &dispatcher,
        &graph,
        &frontier_in,
        1,
        50,
        &mut scratch,
        &mut out,
    )
    .expect("resident adaptive dense branch");
    assert_eq!(out, expected);
    assert_eq!(out, pack_nodes(&[5], node_count));
    let telemetry = backend.telemetry_snapshot();
    assert_eq!(
        telemetry.kernel_launches, 3,
        "Fix: resident adaptive traversal must launch exactly reduce_count + device frontier clear + sparse/dense traversal."
    );
    assert_eq!(
        telemetry.sync_points, 1,
        "Fix: resident adaptive traversal must fence once for upload + two kernels + compact readback."
    );
    assert_eq!(
        telemetry.readback_bytes,
        (frontier_in.len() * std::mem::size_of::<u32>()) as u64,
        "Fix: resident adaptive traversal must not read back the selector count; only frontier_out is a release-path D2H payload."
    );
    scratch
        .free(&dispatcher)
        .expect("resident adaptive scratch free");
    graph
        .free(&dispatcher)
        .expect("resident adaptive graph free");
}

