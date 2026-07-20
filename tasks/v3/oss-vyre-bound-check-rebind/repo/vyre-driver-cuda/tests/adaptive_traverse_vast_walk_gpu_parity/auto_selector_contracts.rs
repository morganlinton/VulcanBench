use super::*;

#[test]
fn cuda_resident_adaptive_auto_selects_sparse_queue_for_tiny_frontier() {
    let backend = live_dispatcher();
    let dispatcher = vyre_driver_cuda::CudaOptimizerDispatcher::new(&backend);
    let node_count = 128u32;
    let mut edge_offsets = Vec::with_capacity(node_count as usize + 1);
    let mut edge_targets = Vec::with_capacity(node_count as usize);
    let mut edge_kind_mask = Vec::with_capacity(node_count as usize);
    for src in 0..node_count {
        edge_offsets.push(src);
        edge_targets.push((src + 1) % node_count);
        edge_kind_mask.push(1);
    }
    edge_offsets.push(node_count);
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
    let frontier_in = pack_nodes(&[0], node_count);
    let expected = pack_nodes(&[1], node_count);
    let mut scratch = AdaptiveTraversalResidentScratch::default();
    let mut out = Vec::new();

    backend.reset_telemetry();
    let mode = adaptive_traverse_resident_graph_auto_step_with_scratch_into(
        &dispatcher,
        &graph,
        &frontier_in,
        1,
        25,
        &mut scratch,
        &mut out,
    )
    .expect("resident adaptive auto sparse queue path");
    assert_eq!(mode, AdaptiveTraversalMode::SparseQueue);
    assert_eq!(out, expected);
    let mode_again = adaptive_traverse_resident_graph_auto_step_with_scratch_into(
        &dispatcher,
        &graph,
        &frontier_in,
        1,
        25,
        &mut scratch,
        &mut out,
    )
    .expect("resident adaptive auto sparse queue path repeat");
    assert_eq!(mode_again, AdaptiveTraversalMode::SparseQueue);
    assert_eq!(
        scratch.plan_cache_snapshot(),
        AdaptiveTraversalPlanCacheSnapshot {
            entries: 3,
            hits: 3,
            misses: 3,
        },
        "Fix: auto sparse queue traversal must reuse device frontier clear, queue-build, and queue-consume Programs on repeated resident graph calls."
    );
    let telemetry = backend.telemetry_snapshot();
    assert_eq!(telemetry.kernel_launches, 6);
    assert_eq!(telemetry.sync_points, 2);
    assert_eq!(
        telemetry.readback_bytes,
        (2 * frontier_in.len() * std::mem::size_of::<u32>()) as u64
    );
    scratch
        .free(&dispatcher)
        .expect("resident adaptive scratch free");
    graph
        .free(&dispatcher)
        .expect("resident adaptive graph free");
}

#[test]

fn cuda_resident_adaptive_auto_selects_sparse_dense_for_dense_frontier() {
    let backend = live_dispatcher();
    let dispatcher = vyre_driver_cuda::CudaOptimizerDispatcher::new(&backend);
    let node_count = 32u32;
    let edge_offsets = vec![0u32; node_count as usize + 1];
    let edge_targets = Vec::new();
    let edge_kind_mask = Vec::new();
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
    let frontier_in = pack_nodes(&(0..16).collect::<Vec<_>>(), node_count);
    let expected = pack_nodes(&[7], node_count);
    let mut scratch = AdaptiveTraversalResidentScratch::default();
    let mut out = Vec::new();

    backend.reset_telemetry();
    let mode = adaptive_traverse_resident_graph_auto_step_with_scratch_into(
        &dispatcher,
        &graph,
        &frontier_in,
        1,
        25,
        &mut scratch,
        &mut out,
    )
    .expect("resident adaptive auto sparse/dense path");
    assert_eq!(mode, AdaptiveTraversalMode::SparseDense);
    assert_eq!(out, expected);
    let mode_again = adaptive_traverse_resident_graph_auto_step_with_scratch_into(
        &dispatcher,
        &graph,
        &frontier_in,
        1,
        25,
        &mut scratch,
        &mut out,
    )
    .expect("resident adaptive auto sparse/dense path repeat");
    assert_eq!(mode_again, AdaptiveTraversalMode::SparseDense);
    assert_eq!(
        scratch.plan_cache_snapshot(),
        AdaptiveTraversalPlanCacheSnapshot {
            entries: 3,
            hits: 3,
            misses: 3,
        },
        "Fix: auto sparse/dense traversal must reuse popcount, device frontier clear, and traversal Programs on repeated resident graph calls."
    );
    let telemetry = backend.telemetry_snapshot();
    assert_eq!(telemetry.kernel_launches, 6);
    assert_eq!(telemetry.sync_points, 2);
    assert_eq!(
        telemetry.readback_bytes,
        (2 * frontier_in.len() * std::mem::size_of::<u32>()) as u64
    );
    scratch
        .free(&dispatcher)
        .expect("resident adaptive scratch free");
    graph
        .free(&dispatcher)
        .expect("resident adaptive graph free");
}

// ---------------------------------------------------------------------
// VAST preorder walk
// ---------------------------------------------------------------------

