use super::super::*;
use super::support::RecordingResidentDispatcher;

#[test]
fn sparse_dense_zero_frontier_returns_zero_without_resident_work_or_cache() {
    let dispatcher = RecordingResidentDispatcher::default();
    let graph = ResidentAdaptiveTraversalGraph {
        node_count: 33,
        edge_count: 0,
        max_row_degree: 0,
        high_degree_source_count: 0,
        words: 2,
        layout_hash: 7,
        handles: [101, 102, 103, 104],
    };
    let mut scratch = AdaptiveTraversalResidentScratch::default();
    let mut frontier_out = vec![9, 9, 9];

    adaptive_traverse_resident_graph_step_with_scratch_into(
        &dispatcher,
        &graph,
        &[0, 0],
        u32::MAX,
        25,
        &mut scratch,
        &mut frontier_out,
    )
    .expect("Fix: zero sparse/dense frontier should complete on host");

    assert_eq!(frontier_out, vec![0, 0]);
    assert!(scratch.handles.is_none());
    assert_eq!(
        scratch.plan_cache_snapshot(),
        AdaptiveTraversalPlanCacheSnapshot::default()
    );
    dispatcher.assert_no_resident_work();
}

#[test]
fn four_russians_zero_frontier_returns_zero_without_resident_work_or_cache() {
    let dispatcher = RecordingResidentDispatcher::default();
    let graph = ResidentAdaptiveFourRussiansDenseGraph {
        node_count: 33,
        words: 2,
        layout_hash: 7,
        lut_handle: 201,
    };
    let mut scratch = AdaptiveTraversalResidentScratch::default();
    let mut frontier_out = vec![9, 9, 9];

    adaptive_traverse_resident_graph_four_russians_dense_step_with_scratch_into(
        &dispatcher,
        &graph,
        &[0, 0],
        &mut scratch,
        &mut frontier_out,
    )
    .expect("Fix: zero Four-Russians frontier should complete on host");

    assert_eq!(frontier_out, vec![0, 0]);
    assert!(scratch.handles.is_none());
    assert_eq!(
        scratch.plan_cache_snapshot(),
        AdaptiveTraversalPlanCacheSnapshot::default()
    );
    dispatcher.assert_no_resident_work();
}

#[test]
fn sparse_queue_zero_frontier_returns_zero_without_queue_allocation_or_cache() {
    let dispatcher = RecordingResidentDispatcher::default();
    let graph = ResidentAdaptiveTraversalGraph {
        node_count: 33,
        edge_count: 0,
        max_row_degree: 0,
        high_degree_source_count: 0,
        words: 2,
        layout_hash: 7,
        handles: [101, 102, 103, 104],
    };
    let mut scratch = AdaptiveTraversalResidentScratch::default();
    let mut frontier_out = vec![9, 9, 9];

    adaptive_traverse_resident_graph_sparse_queue_step_with_scratch_into(
        &dispatcher,
        &graph,
        &[0, 0],
        u32::MAX,
        &mut scratch,
        &mut frontier_out,
    )
    .expect("Fix: zero sparse-queue frontier should complete on host");

    assert_eq!(frontier_out, vec![0, 0]);
    assert!(scratch.handles.is_none());
    assert!(scratch.queue_handle.is_none());
    assert!(scratch.word_partials_handle.is_none());
    assert!(scratch.word_block_totals_handle.is_none());
    assert_eq!(
        scratch.plan_cache_snapshot(),
        AdaptiveTraversalPlanCacheSnapshot::default()
    );
    dispatcher.assert_no_resident_work();
}

#[test]
fn auto_zero_frontier_returns_sparse_queue_without_resident_work_or_cache() {
    let dispatcher = RecordingResidentDispatcher::default();
    let graph = ResidentAdaptiveTraversalGraph {
        node_count: 33,
        edge_count: 128,
        max_row_degree: 8,
        high_degree_source_count: 0,
        words: 2,
        layout_hash: 7,
        handles: [101, 102, 103, 104],
    };
    let mut scratch = AdaptiveTraversalResidentScratch::default();
    let mut frontier_out = vec![9, 9, 9];

    let mode = adaptive_traverse_resident_graph_auto_step_with_scratch_into(
        &dispatcher,
        &graph,
        &[0, 0],
        u32::MAX,
        25,
        &mut scratch,
        &mut frontier_out,
    )
    .expect("Fix: zero auto frontier should complete on host");

    assert_eq!(mode, AdaptiveTraversalMode::SparseQueue);
    assert_eq!(frontier_out, vec![0, 0]);
    assert!(scratch.handles.is_none());
    assert!(scratch.queue_handle.is_none());
    assert!(scratch.word_partials_handle.is_none());
    assert!(scratch.word_block_totals_handle.is_none());
    assert_eq!(
        scratch.plan_cache_snapshot(),
        AdaptiveTraversalPlanCacheSnapshot::default()
    );
    dispatcher.assert_no_resident_work();
}

#[test]
fn auto_step_rejects_bad_frontier_before_resident_allocation() {
    let dispatcher = RecordingResidentDispatcher::default();
    let graph = ResidentAdaptiveTraversalGraph {
        node_count: 33,
        edge_count: 0,
        max_row_degree: 0,
        high_degree_source_count: 0,
        words: 2,
        layout_hash: 7,
        handles: [101, 102, 103, 104],
    };
    let mut scratch = AdaptiveTraversalResidentScratch::default();
    let mut frontier_out = vec![123];

    let err = adaptive_traverse_resident_graph_auto_step_with_scratch_into(
        &dispatcher,
        &graph,
        &[1],
        u32::MAX,
        25,
        &mut scratch,
        &mut frontier_out,
    )
    .expect_err("Fix: malformed frontier must be rejected before mode dispatch");

    assert!(
        err.to_string().contains("expected 2 word(s)"),
        "unexpected frontier validation error: {err}"
    );
    assert_eq!(
        dispatcher.alloc_count.get(),
        0,
        "auto mode must validate frontier shape before allocating resident scratch"
    );
    assert_eq!(
        frontier_out,
        vec![123],
        "failed validation must not mutate caller output storage"
    );
}
