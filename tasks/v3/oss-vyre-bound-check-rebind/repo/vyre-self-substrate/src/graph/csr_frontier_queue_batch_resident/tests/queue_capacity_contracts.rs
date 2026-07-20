use super::super::*;
use super::support::RecordingBatchDispatcher;
use crate::csr_frontier_queue_resident::upload_resident_csr_queue_graph;

#[test]
fn batch_queries_bucket_graph_sized_capacity_from_max_frontier_popcount() {
    let dispatcher = RecordingBatchDispatcher::default();
    let node_count = 4096u32;
    let edge_offsets = vec![0u32; node_count as usize + 1];
    let graph = upload_resident_csr_queue_graph(&dispatcher, node_count, &edge_offsets, &[], &[])
        .expect("Fix: zero-edge resident CSR graph is valid");
    let words = vyre_primitives::bitset::bitset_words(node_count) as usize;
    let mut first = vec![0u32; words];
    first[0] = 1;
    let mut second = vec![0u32; words];
    for node in 0..257u32 {
        second[(node / 32) as usize] |= 1 << (node % 32);
    }
    let frontiers: [&[u32]; 2] = [&first, &second];
    let mut scratch = ResidentCsrQueueBatchScratch::default();
    let mut outputs = Vec::new();

    run_resident_csr_queue_batch_into(
        &dispatcher,
        &graph,
        &mut scratch,
        &frontiers,
        node_count,
        u32::MAX,
        &mut outputs,
    )
    .expect("Fix: recording dispatcher should complete bucketed resident CSR queue batch");

    assert_eq!(
        scratch
            .shape
            .expect("Fix: batch scratch shape should be retained")
            .queue_capacity,
        512,
        "batch queue capacity should be bucketed from the max active frontier, not graph size"
    );
    let grids = dispatcher
        .step_grids
        .borrow()
        .last()
        .cloned()
        .expect("Fix: expected one resident batch grid sequence");
    assert_eq!(grids[2], Some([2, 1, 1]));
    assert_eq!(grids[5], Some([2, 1, 1]));
}

#[test]
fn batch_queries_reuse_larger_queue_scratch_for_smaller_effective_capacity() {
    let dispatcher = RecordingBatchDispatcher::default();
    let node_count = 4096u32;
    let edge_offsets = vec![0u32; node_count as usize + 1];
    let graph = upload_resident_csr_queue_graph(&dispatcher, node_count, &edge_offsets, &[], &[])
        .expect("Fix: zero-edge resident CSR graph is valid");
    let words = vyre_primitives::bitset::bitset_words(node_count) as usize;
    let mut larger = vec![0u32; words];
    for node in 0..257u32 {
        larger[(node / 32) as usize] |= 1 << (node % 32);
    }
    let large_frontiers: [&[u32]; 2] = [&larger, &larger];
    let mut scratch = ResidentCsrQueueBatchScratch::default();
    let mut outputs = Vec::new();

    run_resident_csr_queue_batch_into(
        &dispatcher,
        &graph,
        &mut scratch,
        &large_frontiers,
        node_count,
        u32::MAX,
        &mut outputs,
    )
    .expect("Fix: first resident CSR queue batch dispatch should allocate the larger bucket");

    let retained_queue_handles: Vec<u64> = scratch
        .handles
        .iter()
        .map(|handles| handles.active_queue)
        .collect();
    let next_handle_after_large = dispatcher.next_handle.get();
    let mut single = vec![0u32; words];
    single[0] = 1;
    let small_frontiers: [&[u32]; 2] = [&single, &single];

    run_resident_csr_queue_batch_into(
        &dispatcher,
        &graph,
        &mut scratch,
        &small_frontiers,
        node_count,
        u32::MAX,
        &mut outputs,
    )
    .expect("Fix: second resident CSR queue batch dispatch should reuse the larger bucket");

    assert_eq!(
        scratch
            .handles
            .iter()
            .map(|handles| handles.active_queue)
            .collect::<Vec<_>>(),
        retained_queue_handles
    );
    assert_eq!(
        scratch
            .shape
            .expect("Fix: batch scratch shape should be retained")
            .queue_capacity,
        512
    );
    assert_eq!(
        dispatcher.next_handle.get(),
        next_handle_after_large,
        "smaller sparse batches should not allocate new resident queue scratch"
    );
    assert!(dispatcher.freed.borrow().is_empty());
    let grids = dispatcher
        .step_grids
        .borrow()
        .last()
        .cloned()
        .expect("Fix: expected second resident batch grid sequence");
    assert_eq!(
        grids[2],
        Some([1, 1, 1]),
        "first reused batch query should launch traversal at the smaller effective capacity"
    );
    assert_eq!(
        grids[5],
        Some([1, 1, 1]),
        "second reused batch query should launch traversal at the smaller effective capacity"
    );
}
