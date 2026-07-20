use super::super::*;
use super::support::RecordingBatchDispatcher;
use crate::csr_frontier_queue_resident::upload_resident_csr_queue_graph;
use crate::graph::csr_frontier_queue_scratch::ResidentCsrQueueMaterializer;

#[test]
fn large_sparse_batch_queries_use_atomic_word_materializer() {
    let dispatcher = RecordingBatchDispatcher::default();
    let node_count = 8_193u32;
    let edge_offsets = vec![0u32; node_count as usize + 1];
    let graph = upload_resident_csr_queue_graph(&dispatcher, node_count, &edge_offsets, &[], &[])
        .expect("Fix: zero-edge large resident CSR graph is valid");
    let words = vyre_primitives::bitset::bitset_words(node_count) as usize;
    let mut first = vec![0u32; words];
    first[0] = 1;
    let second = vec![0u32; words];
    let frontiers: [&[u32]; 2] = [&first, &second];
    let mut scratch = ResidentCsrQueueBatchScratch::default();
    let mut outputs = Vec::new();

    run_resident_csr_queue_batch_into(
        &dispatcher,
        &graph,
        &mut scratch,
        &frontiers,
        8,
        u32::MAX,
        &mut outputs,
    )
    .expect("Fix: recording dispatcher should complete large resident CSR queue batch");

    assert_eq!(
        scratch
            .shape
            .expect("Fix: batch scratch shape should be retained")
            .materializer,
        ResidentCsrQueueMaterializer::AtomicWordScan
    );
    assert_eq!(scratch.word_count_handle_sets.len(), 0);
    assert_eq!(scratch.word_prefix_queue_handle_sets.len(), 0);
    assert_eq!(scratch.atomic_word_queue_handle_sets.len(), frontiers.len());
    assert_eq!(scratch.queue_handle_sets.len(), frontiers.len());
    let steps = dispatcher
        .step_handles
        .borrow()
        .last()
        .cloned()
        .expect("Fix: expected one resident step sequence");
    assert_eq!(steps.len(), 6);
    assert_eq!(steps[0], vec![scratch.handles[0].queue_len]);
    assert_eq!(
        steps[1],
        scratch.atomic_word_queue_handle_sets[0].as_slice(),
        "wide sparse batch query should use single-pass atomic word compaction"
    );
    assert_eq!(steps[3], vec![scratch.handles[1].queue_len]);
    assert_eq!(
        steps[4],
        scratch.atomic_word_queue_handle_sets[1].as_slice()
    );
    assert_eq!(
        outputs,
        vec![
            vec![0; words * std::mem::size_of::<u32>()],
            vec![0; words * std::mem::size_of::<u32>()],
        ]
    );
}

#[test]
fn large_dense_batch_queries_use_word_prefix_queue_materializer() {
    let dispatcher = RecordingBatchDispatcher::default();
    let node_count = 8_193u32;
    let edge_offsets = vec![0u32; node_count as usize + 1];
    let graph = upload_resident_csr_queue_graph(&dispatcher, node_count, &edge_offsets, &[], &[])
        .expect("Fix: zero-edge large resident CSR graph is valid");
    let words = vyre_primitives::bitset::bitset_words(node_count) as usize;
    let first = vec![u32::MAX; words];
    let second = vec![0u32; words];
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
    .expect("Fix: recording dispatcher should complete large resident CSR queue batch");

    assert_eq!(
        scratch
            .shape
            .expect("Fix: batch scratch shape should be retained")
            .materializer,
        ResidentCsrQueueMaterializer::DeterministicWordPrefix
    );
    assert_eq!(scratch.word_count_handle_sets.len(), frontiers.len());
    assert_eq!(scratch.word_prefix_queue_handle_sets.len(), frontiers.len());
    assert_eq!(scratch.queue_handle_sets.len(), frontiers.len());
    let steps = dispatcher
        .step_handles
        .borrow()
        .last()
        .cloned()
        .expect("Fix: expected one resident step sequence");
    assert_eq!(steps.len(), 8);
    assert_eq!(steps[0], vec![scratch.handles[0].frontier_out]);
    assert_eq!(
        steps[1],
        scratch.word_count_handle_sets[0].as_slice(),
        "large dense batch query must run word popcount scan before queue scatter"
    );
    assert_eq!(
        steps[2],
        scratch.word_prefix_queue_handle_sets[0].as_slice(),
        "large dense batch query must run deterministic word-prefix scatter"
    );
    assert_eq!(steps[4], vec![scratch.handles[1].frontier_out]);
    assert_eq!(steps[5], scratch.word_count_handle_sets[1].as_slice());
    assert_eq!(
        steps[6],
        scratch.word_prefix_queue_handle_sets[1].as_slice()
    );
    assert_eq!(
        outputs,
        vec![
            vec![0; words * std::mem::size_of::<u32>()],
            vec![0; words * std::mem::size_of::<u32>()],
        ]
    );
}

#[test]
fn small_multiblock_batch_queries_inline_block_offsets() {
    let dispatcher = RecordingBatchDispatcher::default();
    let node_count = 32_897u32;
    let edge_offsets = vec![0u32; node_count as usize + 1];
    let graph = upload_resident_csr_queue_graph(&dispatcher, node_count, &edge_offsets, &[], &[])
        .expect("Fix: zero-edge multiblock resident CSR graph is valid");
    let words = vyre_primitives::bitset::bitset_words(node_count) as usize;
    let first = vec![u32::MAX; words];
    let second = vec![0u32; words];
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
    .expect("Fix: recording dispatcher should complete multiblock resident CSR queue batch");

    assert_eq!(scratch.word_count_handle_sets.len(), frontiers.len());
    assert_eq!(
        scratch.word_block_offsets_handle_sets.len(),
        0,
        "small multiblock batch queries should not pay a block-offset scan launch"
    );
    assert_eq!(scratch.word_prefix_queue_handle_sets.len(), frontiers.len());
    let steps = dispatcher
        .step_handles
        .borrow()
        .last()
        .cloned()
        .expect("Fix: expected one resident step sequence");
    assert_eq!(steps.len(), 8);
    assert_eq!(steps[0], vec![scratch.handles[0].frontier_out]);
    assert_eq!(steps[1], scratch.word_count_handle_sets[0].as_slice());
    assert_eq!(
        steps[2],
        scratch.word_prefix_queue_handle_sets[0].as_slice()
    );
    assert_eq!(steps[4], vec![scratch.handles[1].frontier_out]);
    assert_eq!(steps[5], scratch.word_count_handle_sets[1].as_slice());
    assert_eq!(
        steps[6],
        scratch.word_prefix_queue_handle_sets[1].as_slice()
    );
    assert_eq!(
        outputs,
        vec![
            vec![0; words * std::mem::size_of::<u32>()],
            vec![0; words * std::mem::size_of::<u32>()],
        ]
    );
}

#[test]
fn many_block_batch_queries_scan_block_offsets_once_per_query() {
    let dispatcher = RecordingBatchDispatcher::default();
    let node_count = 262_177u32;
    let edge_offsets = vec![0u32; node_count as usize + 1];
    let graph = upload_resident_csr_queue_graph(&dispatcher, node_count, &edge_offsets, &[], &[])
        .expect("Fix: zero-edge many-block resident CSR graph is valid");
    let words = vyre_primitives::bitset::bitset_words(node_count) as usize;
    let first = vec![u32::MAX; words];
    let second = vec![0u32; words];
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
    .expect("Fix: recording dispatcher should complete many-block resident CSR queue batch");

    assert_eq!(scratch.word_count_handle_sets.len(), frontiers.len());
    assert_eq!(
        scratch.word_block_offsets_handle_sets.len(),
        frontiers.len()
    );
    assert_eq!(scratch.word_prefix_queue_handle_sets.len(), frontiers.len());
    let steps = dispatcher
        .step_handles
        .borrow()
        .last()
        .cloned()
        .expect("Fix: expected one resident step sequence");
    assert_eq!(steps.len(), 10);
    assert_eq!(steps[0], vec![scratch.handles[0].frontier_out]);
    assert_eq!(steps[1], scratch.word_count_handle_sets[0].as_slice());
    assert_eq!(
        steps[2],
        scratch.word_block_offsets_handle_sets[0].as_slice(),
        "many-block batch query must scan block offsets before scatter"
    );
    assert_eq!(
        steps[3],
        scratch.word_prefix_queue_handle_sets[0].as_slice()
    );
    assert_eq!(steps[5], vec![scratch.handles[1].frontier_out]);
    assert_eq!(steps[6], scratch.word_count_handle_sets[1].as_slice());
    assert_eq!(
        steps[7],
        scratch.word_block_offsets_handle_sets[1].as_slice()
    );
    assert_eq!(
        steps[8],
        scratch.word_prefix_queue_handle_sets[1].as_slice()
    );
    assert_eq!(
        outputs,
        vec![
            vec![0; words * std::mem::size_of::<u32>()],
            vec![0; words * std::mem::size_of::<u32>()],
        ]
    );
}
