use super::super::*;
use super::support::RecordingResidentDispatcher;
use crate::graph::csr_frontier_queue_scratch::ResidentCsrQueueMaterializer;

#[test]
fn large_single_word_resident_query_uses_atomic_word_materializer() {
    let dispatcher = RecordingResidentDispatcher::default();
    let node_count = 8_193u32;
    let words = vyre_primitives::bitset::bitset_words(node_count) as usize;
    let graph = ResidentCsrQueueGraph {
        node_count,
        edge_count: 0,
        max_row_degree: 0,
        high_degree_source_count: 0,
        words,
        edge_offsets_handle: 201,
        edge_targets_handle: 202,
        edge_kind_mask_handle: 203,
    };
    let mut scratch = ResidentCsrQueueScratch::default();
    let mut output = Vec::new();
    let mut frontier = vec![0u32; words];
    frontier[0] = 1;

    run_resident_csr_queue_query_into(
        &dispatcher,
        &graph,
        &mut scratch,
        &frontier,
        8,
        u32::MAX,
        &mut output,
    )
    .expect("Fix: recording dispatcher should complete large resident CSR queue query");

    let handles = scratch
        .handles
        .expect("Fix: large resident CSR queue query should allocate scratch handles");
    assert_eq!(
        handles.materializer,
        ResidentCsrQueueMaterializer::AtomicWordScan
    );
    assert!(handles.word_partials.is_none());
    assert!(handles.block_totals.is_none());
    let steps = dispatcher
        .sequence_step_handles
        .borrow()
        .last()
        .cloned()
        .expect("Fix: expected one resident step sequence");

    assert_eq!(steps.len(), 3);
    assert_eq!(steps[0], vec![handles.queue_len]);
    assert_eq!(
        steps[1],
        vec![
            handles.frontier,
            handles.active_queue,
            handles.queue_len,
            handles.frontier_out,
        ],
        "wide graph with one nonzero frontier word should use the single-pass atomic word materializer"
    );
    assert_eq!(output, vec![0; words * std::mem::size_of::<u32>()]);
}

#[test]
fn large_dense_resident_query_uses_word_prefix_queue_materializer() {
    let dispatcher = RecordingResidentDispatcher::default();
    let node_count = 8_193u32;
    let words = vyre_primitives::bitset::bitset_words(node_count) as usize;
    let graph = ResidentCsrQueueGraph {
        node_count,
        edge_count: 0,
        max_row_degree: 0,
        high_degree_source_count: 0,
        words,
        edge_offsets_handle: 201,
        edge_targets_handle: 202,
        edge_kind_mask_handle: 203,
    };
    let mut scratch = ResidentCsrQueueScratch::default();
    let mut output = Vec::new();
    let frontier = vec![u32::MAX; words];

    run_resident_csr_queue_query_into(
        &dispatcher,
        &graph,
        &mut scratch,
        &frontier,
        node_count,
        u32::MAX,
        &mut output,
    )
    .expect("Fix: recording dispatcher should complete large dense resident CSR queue query");

    let handles = scratch
        .handles
        .expect("Fix: large dense resident CSR queue query should allocate scratch handles");
    assert_eq!(
        handles.materializer,
        ResidentCsrQueueMaterializer::DeterministicWordPrefix
    );
    let word_partials = handles
        .word_partials
        .expect("Fix: word-prefix query should allocate word_partials");
    let block_totals = handles
        .block_totals
        .expect("Fix: word-prefix query should allocate block_totals");
    let steps = dispatcher
        .sequence_step_handles
        .borrow()
        .last()
        .cloned()
        .expect("Fix: expected one resident step sequence");

    assert_eq!(steps.len(), 4);
    assert_eq!(steps[0], vec![handles.frontier_out]);
    assert_eq!(
        steps[1],
        vec![handles.frontier, word_partials, block_totals]
    );
    assert_eq!(
        steps[2],
        vec![
            handles.frontier,
            word_partials,
            block_totals,
            handles.active_queue,
            handles.queue_len,
        ]
    );
    assert_eq!(output, vec![0; words * std::mem::size_of::<u32>()]);
}

#[test]
fn small_multiblock_resident_query_inlines_block_offsets() {
    let dispatcher = RecordingResidentDispatcher::default();
    let node_count = 32_897u32;
    let words = vyre_primitives::bitset::bitset_words(node_count) as usize;
    let graph = ResidentCsrQueueGraph {
        node_count,
        edge_count: 0,
        max_row_degree: 0,
        high_degree_source_count: 0,
        words,
        edge_offsets_handle: 201,
        edge_targets_handle: 202,
        edge_kind_mask_handle: 203,
    };
    let mut scratch = ResidentCsrQueueScratch::default();
    let mut output = Vec::new();
    let frontier = vec![u32::MAX; words];

    run_resident_csr_queue_query_into(
        &dispatcher,
        &graph,
        &mut scratch,
        &frontier,
        node_count,
        u32::MAX,
        &mut output,
    )
    .expect("Fix: recording dispatcher should complete multiblock resident CSR queue query");

    let handles = scratch
        .handles
        .expect("Fix: multiblock resident CSR queue query should allocate scratch handles");
    let word_partials = handles
        .word_partials
        .expect("Fix: multiblock word-prefix query should allocate word_partials");
    let block_totals = handles
        .block_totals
        .expect("Fix: multiblock word-prefix query should allocate block_totals");
    let steps = dispatcher
        .sequence_step_handles
        .borrow()
        .last()
        .cloned()
        .expect("Fix: expected one resident step sequence");

    assert_eq!(steps.len(), 4);
    assert_eq!(steps[0], vec![handles.frontier_out]);
    assert_eq!(
        steps[1],
        vec![handles.frontier, word_partials, block_totals]
    );
    assert_eq!(
        steps[2],
        vec![
            handles.frontier,
            word_partials,
            block_totals,
            handles.active_queue,
            handles.queue_len,
        ]
    );
    assert_eq!(output, vec![0; words * std::mem::size_of::<u32>()]);
}

#[test]
fn many_block_resident_query_scans_block_offsets_once() {
    let dispatcher = RecordingResidentDispatcher::default();
    let node_count = 262_177u32;
    let words = vyre_primitives::bitset::bitset_words(node_count) as usize;
    let graph = ResidentCsrQueueGraph {
        node_count,
        edge_count: 0,
        max_row_degree: 0,
        high_degree_source_count: 0,
        words,
        edge_offsets_handle: 201,
        edge_targets_handle: 202,
        edge_kind_mask_handle: 203,
    };
    let mut scratch = ResidentCsrQueueScratch::default();
    let mut output = Vec::new();
    let frontier = vec![u32::MAX; words];

    run_resident_csr_queue_query_into(
        &dispatcher,
        &graph,
        &mut scratch,
        &frontier,
        node_count,
        u32::MAX,
        &mut output,
    )
    .expect("Fix: recording dispatcher should complete many-block resident CSR queue query");

    let handles = scratch
        .handles
        .expect("Fix: many-block resident CSR queue query should allocate scratch handles");
    let word_partials = handles
        .word_partials
        .expect("Fix: many-block word-prefix query should allocate word_partials");
    let block_totals = handles
        .block_totals
        .expect("Fix: many-block word-prefix query should allocate block_totals");
    let steps = dispatcher
        .sequence_step_handles
        .borrow()
        .last()
        .cloned()
        .expect("Fix: expected one resident step sequence");

    assert_eq!(steps.len(), 5);
    assert_eq!(steps[0], vec![handles.frontier_out]);
    assert_eq!(
        steps[1],
        vec![handles.frontier, word_partials, block_totals]
    );
    assert_eq!(
        steps[2],
        vec![block_totals],
        "many-block query must convert block totals into offsets once"
    );
    assert_eq!(
        steps[3],
        vec![
            handles.frontier,
            word_partials,
            block_totals,
            handles.active_queue,
            handles.queue_len,
        ]
    );
    assert_eq!(output, vec![0; words * std::mem::size_of::<u32>()]);
}
