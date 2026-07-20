use super::super::*;
use super::support::RecordingBatchDispatcher;
use crate::csr_frontier_queue_resident::upload_resident_csr_queue_graph;

#[test]
fn generated_batch_dispatch_tables_reuse_capacity_across_calls() {
    let dispatcher = RecordingBatchDispatcher::default();
    let graph = upload_resident_csr_queue_graph(&dispatcher, 4, &[0, 0, 0, 0, 0], &[], &[])
        .expect("Fix: zero-edge resident CSR graph is valid");
    let mut scratch = ResidentCsrQueueBatchScratch::default();
    let first = [1_u32];
    let second = [2_u32];
    let frontiers: [&[u32]; 2] = [&first, &second];
    let mut outputs = Vec::new();

    run_resident_csr_queue_batch_into(
        &dispatcher,
        &graph,
        &mut scratch,
        &frontiers,
        4,
        u32::MAX,
        &mut outputs,
    )
    .expect("Fix: first resident CSR queue batch dispatch should succeed");
    let retained_capacities = (
        scratch.clear_handle_sets.capacity(),
        scratch.queue_len_handle_sets.capacity(),
        scratch.word_count_handle_sets.capacity(),
        scratch.word_block_offsets_handle_sets.capacity(),
        scratch.queue_handle_sets.capacity(),
        scratch.atomic_word_queue_handle_sets.capacity(),
        scratch.word_prefix_queue_handle_sets.capacity(),
        scratch.traverse_handle_sets.capacity(),
        scratch.high_len_handle_sets.capacity(),
        scratch.split_low_handle_sets.capacity(),
        scratch.high_traverse_handle_sets.capacity(),
        scratch.read_ranges.capacity(),
    );

    run_resident_csr_queue_batch_into(
        &dispatcher,
        &graph,
        &mut scratch,
        &frontiers,
        4,
        u32::MAX,
        &mut outputs,
    )
    .expect("Fix: second resident CSR queue batch dispatch should reuse prepared scratch");

    assert_eq!(
        (
            scratch.clear_handle_sets.capacity(),
            scratch.queue_len_handle_sets.capacity(),
            scratch.word_count_handle_sets.capacity(),
            scratch.word_block_offsets_handle_sets.capacity(),
            scratch.queue_handle_sets.capacity(),
            scratch.atomic_word_queue_handle_sets.capacity(),
            scratch.word_prefix_queue_handle_sets.capacity(),
            scratch.traverse_handle_sets.capacity(),
            scratch.high_len_handle_sets.capacity(),
            scratch.split_low_handle_sets.capacity(),
            scratch.high_traverse_handle_sets.capacity(),
            scratch.read_ranges.capacity(),
        ),
        retained_capacities,
        "resident batch sequence tables must retain allocation capacity across repeated dispatches"
    );
    assert_eq!(scratch.clear_handle_sets.len(), frontiers.len());
    assert_eq!(scratch.queue_len_handle_sets.len(), frontiers.len());
    assert_eq!(scratch.word_count_handle_sets.len(), 0);
    assert_eq!(scratch.word_block_offsets_handle_sets.len(), 0);
    assert_eq!(scratch.queue_handle_sets.len(), frontiers.len());
    assert_eq!(scratch.atomic_word_queue_handle_sets.len(), frontiers.len());
    assert_eq!(scratch.word_prefix_queue_handle_sets.len(), 0);
    assert_eq!(scratch.traverse_handle_sets.len(), frontiers.len());
    assert_eq!(scratch.high_len_handle_sets.len(), 0);
    assert_eq!(scratch.split_low_handle_sets.len(), 0);
    assert_eq!(scratch.high_traverse_handle_sets.len(), 0);
    assert_eq!(scratch.read_ranges.len(), frontiers.len());

    scratch
        .free(&dispatcher)
        .expect("Fix: resident CSR batch scratch free should release query handles");
    assert!(scratch.clear_handle_sets.is_empty());
    assert!(scratch.queue_len_handle_sets.is_empty());
    assert!(scratch.word_count_handle_sets.is_empty());
    assert!(scratch.word_block_offsets_handle_sets.is_empty());
    assert!(scratch.queue_handle_sets.is_empty());
    assert!(scratch.atomic_word_queue_handle_sets.is_empty());
    assert!(scratch.word_prefix_queue_handle_sets.is_empty());
    assert!(scratch.traverse_handle_sets.is_empty());
    assert!(scratch.high_len_handle_sets.is_empty());
    assert!(scratch.split_low_handle_sets.is_empty());
    assert!(scratch.high_traverse_handle_sets.is_empty());
    assert!(scratch.read_ranges.is_empty());
}

#[test]
fn generated_batch_scratch_free_releases_each_handle_once_in_first_seen_order() {
    for seed in 0..4096_u64 {
        let dispatcher = RecordingBatchDispatcher::default();
        let base = 40_000 + seed * 16;
        let mut scratch = ResidentCsrQueueBatchScratch::default();
        scratch.handles.push(ResidentCsrQueueBatchQueryHandles {
            frontier: base,
            active_queue: base + 1,
            queue_len: base,
            frontier_out: base + 2,
            word_partials: None,
            block_totals: None,
            high_queue: None,
            high_len: None,
        });
        scratch.handles.push(ResidentCsrQueueBatchQueryHandles {
            frontier: base + 2,
            active_queue: base + 3,
            queue_len: base + 3,
            frontier_out: base + 4,
            word_partials: Some(base + 5),
            block_totals: Some(base + 5),
            high_queue: Some(base + 6),
            high_len: Some(base + 6),
        });
        scratch
            .free(&dispatcher)
            .expect("Fix: batch scratch free dedup");
        assert_eq!(
            dispatcher.freed.borrow().as_slice(),
            &[
                base,
                base + 1,
                base + 2,
                base + 3,
                base + 4,
                base + 5,
                base + 6
            ]
        );
    }
}
