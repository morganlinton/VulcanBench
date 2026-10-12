use super::super::*;
use super::support::RecordingResidentDispatcher;

#[test]
fn resident_query_buckets_graph_sized_capacity_from_frontier_popcount() {
    let dispatcher = RecordingResidentDispatcher::default();
    let node_count = 4096u32;
    let words = vyre_primitives::bitset::bitset_words(node_count) as usize;
    let graph = ResidentCsrQueueGraph {
        node_count,
        edge_count: 0,
        max_row_degree: 0,
        high_degree_source_count: 0,
        words,
        edge_offsets_handle: 101,
        edge_targets_handle: 102,
        edge_kind_mask_handle: 103,
    };
    let mut scratch = ResidentCsrQueueScratch::default();
    let mut frontier = vec![0u32; words];
    for node in 0..257u32 {
        frontier[(node / 32) as usize] |= 1 << (node % 32);
    }
    let mut output = Vec::new();

    run_resident_csr_queue_query_into(
        &dispatcher,
        &graph,
        &mut scratch,
        &frontier,
        node_count,
        u32::MAX,
        &mut output,
    )
    .expect("Fix: recording dispatcher should complete bucketed resident CSR queue query");

    let handles = scratch
        .handles
        .expect("Fix: resident CSR queue query should allocate scratch handles");
    assert_eq!(
        handles.queue_capacity, 512,
        "257 active sources should use the 512-slot bucket, not graph-sized scratch"
    );
    assert_eq!(
        dispatcher.allocs.borrow().as_slice(),
        &[
            words * std::mem::size_of::<u32>(),
            512 * std::mem::size_of::<u32>(),
            std::mem::size_of::<u32>(),
            words * std::mem::size_of::<u32>(),
        ]
    );
    let grids = dispatcher
        .sequence_step_grids
        .borrow()
        .last()
        .cloned()
        .expect("Fix: expected one resident step grid sequence");
    assert_eq!(grids[2], Some([2, 1, 1]));
}

#[test]
fn resident_query_reuses_larger_queue_scratch_for_smaller_effective_capacity() {
    let dispatcher = RecordingResidentDispatcher::default();
    let node_count = 4096u32;
    let words = vyre_primitives::bitset::bitset_words(node_count) as usize;
    let graph = ResidentCsrQueueGraph {
        node_count,
        edge_count: 0,
        max_row_degree: 0,
        high_degree_source_count: 0,
        words,
        edge_offsets_handle: 101,
        edge_targets_handle: 102,
        edge_kind_mask_handle: 103,
    };
    let mut scratch = ResidentCsrQueueScratch::default();
    let mut larger_frontier = vec![0u32; words];
    for node in 0..257u32 {
        larger_frontier[(node / 32) as usize] |= 1 << (node % 32);
    }
    let mut output = Vec::new();

    run_resident_csr_queue_query_into(
        &dispatcher,
        &graph,
        &mut scratch,
        &larger_frontier,
        node_count,
        u32::MAX,
        &mut output,
    )
    .expect("Fix: first resident CSR queue query should allocate the larger bucket");

    let handles = scratch
        .handles
        .expect("Fix: resident CSR queue query should retain handles");
    let retained_queue_handle = handles.active_queue;
    let alloc_count = dispatcher.allocs.borrow().len();
    let mut single_frontier = vec![0u32; words];
    single_frontier[0] = 1;

    run_resident_csr_queue_query_into(
        &dispatcher,
        &graph,
        &mut scratch,
        &single_frontier,
        node_count,
        u32::MAX,
        &mut output,
    )
    .expect("Fix: second resident CSR queue query should reuse the larger bucket");

    let handles = scratch
        .handles
        .expect("Fix: resident CSR queue query should retain handles");
    assert_eq!(handles.active_queue, retained_queue_handle);
    assert_eq!(handles.queue_capacity, 512);
    assert_eq!(
        dispatcher.allocs.borrow().len(),
        alloc_count,
        "smaller sparse frontiers should not free and reallocate resident queue scratch"
    );
    assert!(dispatcher.freed.borrow().is_empty());
    let grids = dispatcher
        .sequence_step_grids
        .borrow()
        .last()
        .cloned()
        .expect("Fix: expected second resident step grid sequence");
    assert_eq!(
        grids[2],
        Some([1, 1, 1]),
        "the rebuilt program should still launch at the smaller effective capacity"
    );
}
