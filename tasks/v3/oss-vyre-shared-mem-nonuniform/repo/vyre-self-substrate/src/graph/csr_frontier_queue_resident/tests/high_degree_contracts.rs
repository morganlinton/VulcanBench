use super::super::*;
use super::support::RecordingResidentDispatcher;
use crate::graph::csr_frontier_queue_scratch::STRIDED_FORWARD_MIN_ROW_DEGREE;
use vyre_primitives::graph::csr_queue_strided::csr_queue_strided_forward_dispatch_grid;

#[test]
fn skewed_high_degree_resident_query_uses_bounded_split_queue() {
    let dispatcher = RecordingResidentDispatcher::default();
    let graph = ResidentCsrQueueGraph {
        node_count: 16,
        edge_count: STRIDED_FORWARD_MIN_ROW_DEGREE,
        max_row_degree: STRIDED_FORWARD_MIN_ROW_DEGREE,
        high_degree_source_count: 1,
        words: 1,
        edge_offsets_handle: 101,
        edge_targets_handle: 102,
        edge_kind_mask_handle: 103,
    };
    let mut scratch = ResidentCsrQueueScratch::default();
    let mut output = Vec::new();

    run_resident_csr_queue_query_into(
        &dispatcher,
        &graph,
        &mut scratch,
        &[0x1ff],
        1024,
        u32::MAX,
        &mut output,
    )
    .expect("Fix: recording dispatcher should complete high-degree resident CSR query");

    let handles = scratch
        .handles
        .expect("Fix: mixed split resident query should allocate scratch handles");
    let high_queue = handles
        .high_queue
        .expect("Fix: mixed split resident query should allocate high_queue");
    let high_len = handles
        .high_len
        .expect("Fix: mixed split resident query should allocate high_len");
    assert_eq!(handles.high_queue_capacity, 1);
    let steps = dispatcher
        .sequence_step_handles
        .borrow()
        .last()
        .cloned()
        .expect("Fix: expected one resident step sequence");
    assert_eq!(
        steps.len(),
        5,
        "skewed high-degree query should compact all active sources, split low/high rows, then traverse only bounded high rows"
    );
    assert_eq!(
        steps[3],
        vec![
            handles.active_queue,
            handles.queue_len,
            graph.edge_offsets_handle,
            graph.edge_targets_handle,
            graph.edge_kind_mask_handle,
            handles.frontier_out,
            high_queue,
            high_len,
        ],
        "split-low pass must bind active queue plus bounded high-row scratch"
    );
    assert_eq!(
        steps[4],
        vec![
            high_queue,
            high_len,
            graph.edge_offsets_handle,
            graph.edge_targets_handle,
            graph.edge_kind_mask_handle,
            handles.frontier_out,
        ],
        "strided follow-up must consume the bounded high-row queue"
    );
    let grids = dispatcher
        .sequence_step_grids
        .borrow()
        .last()
        .cloned()
        .expect("Fix: expected one resident step grid sequence");
    assert_eq!(
        grids[4],
        Some(csr_queue_strided_forward_dispatch_grid(1)),
        "skewed high-degree resident CSR queue traversal must launch row-strided teams only for the graph-wide high-row bound"
    );
}

#[test]
fn single_superhub_resident_query_sizes_split_queue_from_high_row_count() {
    let dispatcher = RecordingResidentDispatcher::default();
    let graph = ResidentCsrQueueGraph {
        node_count: 16,
        edge_count: STRIDED_FORWARD_MIN_ROW_DEGREE * 9,
        max_row_degree: STRIDED_FORWARD_MIN_ROW_DEGREE * 9,
        high_degree_source_count: 1,
        words: 1,
        edge_offsets_handle: 101,
        edge_targets_handle: 102,
        edge_kind_mask_handle: 103,
    };
    let mut scratch = ResidentCsrQueueScratch::default();
    let mut output = Vec::new();

    run_resident_csr_queue_query_into(
        &dispatcher,
        &graph,
        &mut scratch,
        &[0x1ff],
        1024,
        u32::MAX,
        &mut output,
    )
    .expect("Fix: recording dispatcher should complete superhub resident CSR query");

    let handles = scratch
        .handles
        .expect("Fix: superhub mixed split query should allocate scratch handles");
    assert_eq!(
        handles.high_queue_capacity, 1,
        "one enormous row should allocate one high-row slot, not edge_count / threshold slots"
    );
    assert_eq!(
        dispatcher.allocs.borrow().as_slice(),
        &[4, 64, 4, 4, 4, 4],
        "superhub split scratch should allocate frontier, 16-slot active queue, queue_len, frontier_out, one high_queue word, and high_len"
    );
}

#[test]
fn uniformly_high_degree_resident_query_uses_row_strided_traverse_grid() {
    let dispatcher = RecordingResidentDispatcher::default();
    let graph = ResidentCsrQueueGraph {
        node_count: 16,
        edge_count: STRIDED_FORWARD_MIN_ROW_DEGREE * 16,
        max_row_degree: STRIDED_FORWARD_MIN_ROW_DEGREE,
        high_degree_source_count: 16,
        words: 1,
        edge_offsets_handle: 101,
        edge_targets_handle: 102,
        edge_kind_mask_handle: 103,
    };
    let mut scratch = ResidentCsrQueueScratch::default();
    let mut output = Vec::new();

    run_resident_csr_queue_query_into(
        &dispatcher,
        &graph,
        &mut scratch,
        &[0x1ff],
        1024,
        u32::MAX,
        &mut output,
    )
    .expect("Fix: recording dispatcher should complete uniformly high-degree resident CSR query");

    let handles = scratch
        .handles
        .expect("Fix: resident CSR query should allocate scratch handles");
    assert!(handles.high_queue.is_none());
    assert!(handles.high_len.is_none());
    let steps = dispatcher
        .sequence_step_handles
        .borrow()
        .last()
        .cloned()
        .expect("Fix: expected one resident step sequence");
    assert_eq!(steps.len(), 3);
    let grids = dispatcher
        .sequence_step_grids
        .borrow()
        .last()
        .cloned()
        .expect("Fix: expected one resident step grid sequence");
    assert_eq!(
        grids[2],
        Some(csr_queue_strided_forward_dispatch_grid(16)),
        "uniformly high-degree resident CSR queue traversal must still use the full row-strided path"
    );
}
