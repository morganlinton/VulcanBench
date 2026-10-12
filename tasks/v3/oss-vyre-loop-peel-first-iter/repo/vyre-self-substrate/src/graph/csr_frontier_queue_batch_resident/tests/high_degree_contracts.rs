use super::super::*;
use super::support::RecordingBatchDispatcher;
use crate::csr_frontier_queue_resident::upload_resident_csr_queue_graph;
use crate::graph::csr_frontier_queue_scratch::{
    resident_csr_queue_split_low_grid, STRIDED_FORWARD_MIN_ROW_DEGREE,
};
use vyre_primitives::graph::csr_queue_strided::csr_queue_strided_forward_dispatch_grid;

#[test]
fn skewed_high_degree_batch_queries_use_bounded_split_queue() {
    let dispatcher = RecordingBatchDispatcher::default();
    let node_count = 16u32;
    let mut edge_offsets = vec![0u32; node_count as usize + 1];
    for offset in edge_offsets.iter_mut().skip(1) {
        *offset = STRIDED_FORWARD_MIN_ROW_DEGREE;
    }
    let edge_targets = vec![1u32; STRIDED_FORWARD_MIN_ROW_DEGREE as usize];
    let edge_kind_mask = vec![1u32; STRIDED_FORWARD_MIN_ROW_DEGREE as usize];
    let graph = upload_resident_csr_queue_graph(
        &dispatcher,
        node_count,
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
    )
    .expect("Fix: high-degree resident CSR graph is valid");
    let mut scratch = ResidentCsrQueueBatchScratch::default();
    let first = [0x1ffu32];
    let second = [0x1ffu32];
    let frontiers: [&[u32]; 2] = [&first, &second];
    let mut outputs = Vec::new();

    run_resident_csr_queue_batch_into(
        &dispatcher,
        &graph,
        &mut scratch,
        &frontiers,
        1024,
        u32::MAX,
        &mut outputs,
    )
    .expect("Fix: recording dispatcher should complete skewed high-degree resident CSR queue batch");

    assert_eq!(
        scratch
            .shape
            .expect("Fix: batch scratch shape should be retained")
            .high_queue_capacity,
        1
    );
    let first_high_queue = scratch.handles[0]
        .high_queue
        .expect("Fix: first mixed-split batch query should allocate high_queue");
    let first_high_len = scratch.handles[0]
        .high_len
        .expect("Fix: first mixed-split batch query should allocate high_len");
    let second_high_queue = scratch.handles[1]
        .high_queue
        .expect("Fix: second mixed-split batch query should allocate high_queue");
    let second_high_len = scratch.handles[1]
        .high_len
        .expect("Fix: second mixed-split batch query should allocate high_len");
    assert_eq!(scratch.high_len_handle_sets.len(), 2);
    assert_eq!(scratch.split_low_handle_sets.len(), 2);
    assert_eq!(scratch.high_traverse_handle_sets.len(), 2);
    assert_eq!(scratch.high_len_handle_sets[0], [first_high_len]);
    assert_eq!(
        scratch.split_low_handle_sets[0],
        [
            scratch.handles[0].active_queue,
            scratch.handles[0].queue_len,
            graph.edge_offsets_handle(),
            graph.edge_targets_handle(),
            graph.edge_kind_mask_handle(),
            scratch.handles[0].frontier_out,
            first_high_queue,
            first_high_len,
        ]
    );
    assert_eq!(
        scratch.high_traverse_handle_sets[1],
        [
            second_high_queue,
            second_high_len,
            graph.edge_offsets_handle(),
            graph.edge_targets_handle(),
            graph.edge_kind_mask_handle(),
            scratch.handles[1].frontier_out,
        ]
    );
    let steps = dispatcher
        .step_handles
        .borrow()
        .last()
        .cloned()
        .expect("Fix: expected one resident batch step sequence");
    assert_eq!(
        steps.len(),
        10,
        "skewed high-degree batch queries should add high-len init, split-low, and bounded high-row traverse per query"
    );
    assert_eq!(steps[3], scratch.split_low_handle_sets[0].as_slice());
    assert_eq!(steps[4], scratch.high_traverse_handle_sets[0].as_slice());
    assert_eq!(steps[8], scratch.split_low_handle_sets[1].as_slice());
    assert_eq!(steps[9], scratch.high_traverse_handle_sets[1].as_slice());

    let grids = dispatcher
        .step_grids
        .borrow()
        .last()
        .cloned()
        .expect("Fix: expected one resident batch grid sequence");
    assert_eq!(
        grids[3],
        Some(resident_csr_queue_split_low_grid(16)),
        "first skewed high-degree batch query must split low rows across the active queue"
    );
    assert_eq!(
        grids[4],
        Some(csr_queue_strided_forward_dispatch_grid(1)),
        "first skewed high-degree batch query must traverse only the bounded high-row queue"
    );
    assert_eq!(
        grids[8],
        Some(resident_csr_queue_split_low_grid(16)),
        "second skewed high-degree batch query must split low rows across the active queue"
    );
    assert_eq!(
        grids[9],
        Some(csr_queue_strided_forward_dispatch_grid(1)),
        "second skewed high-degree batch query must traverse only the bounded high-row queue"
    );
}

#[test]
fn uniformly_high_degree_batch_queries_use_row_strided_traverse_grid() {
    let dispatcher = RecordingBatchDispatcher::default();
    let node_count = 16u32;
    let mut edge_offsets = vec![0u32; node_count as usize + 1];
    for (row, offset) in edge_offsets.iter_mut().enumerate() {
        *offset = STRIDED_FORWARD_MIN_ROW_DEGREE * row as u32;
    }
    let edge_count = STRIDED_FORWARD_MIN_ROW_DEGREE as usize * node_count as usize;
    let edge_targets = vec![1u32; edge_count];
    let edge_kind_mask = vec![1u32; edge_count];
    let graph = upload_resident_csr_queue_graph(
        &dispatcher,
        node_count,
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
    )
    .expect("Fix: uniformly high-degree resident CSR graph is valid");
    let mut scratch = ResidentCsrQueueBatchScratch::default();
    let first = [0x1ffu32];
    let second = [0x1ffu32];
    let frontiers: [&[u32]; 2] = [&first, &second];
    let mut outputs = Vec::new();

    run_resident_csr_queue_batch_into(
        &dispatcher,
        &graph,
        &mut scratch,
        &frontiers,
        1024,
        u32::MAX,
        &mut outputs,
    )
    .expect("Fix: recording dispatcher should complete uniformly high-degree resident CSR queue batch");

    assert_eq!(
        scratch
            .shape
            .expect("Fix: batch scratch shape should be retained")
            .high_queue_capacity,
        0
    );
    assert!(scratch
        .handles
        .iter()
        .all(|handles| handles.high_queue.is_none()));
    assert!(scratch
        .handles
        .iter()
        .all(|handles| handles.high_len.is_none()));
    assert!(scratch.high_len_handle_sets.is_empty());
    assert!(scratch.split_low_handle_sets.is_empty());
    assert!(scratch.high_traverse_handle_sets.is_empty());
    let steps = dispatcher
        .step_handles
        .borrow()
        .last()
        .cloned()
        .expect("Fix: expected one resident batch step sequence");
    assert_eq!(steps.len(), 6);
    let grids = dispatcher
        .step_grids
        .borrow()
        .last()
        .cloned()
        .expect("Fix: expected one resident batch grid sequence");
    assert_eq!(
        grids[2],
        Some(csr_queue_strided_forward_dispatch_grid(16)),
        "first uniformly high-degree batch query must use row-strided traverse launch at the sparse effective capacity"
    );
    assert_eq!(
        grids[5],
        Some(csr_queue_strided_forward_dispatch_grid(16)),
        "second uniformly high-degree batch query must use row-strided traverse launch at the sparse effective capacity"
    );
}
