use super::super::*;
use super::support::RecordingResidentDispatcher;

#[test]
fn sparse_queue_resident_step_initializes_queue_len_on_device() {
    let dispatcher = RecordingResidentDispatcher::default();
    let graph = ResidentAdaptiveTraversalGraph {
        node_count: 1,
        edge_count: 0,
        max_row_degree: 0,
        high_degree_source_count: 0,
        words: 1,
        layout_hash: 7,
        handles: [101, 102, 103, 104],
    };
    let mut scratch = AdaptiveTraversalResidentScratch::default();
    let mut frontier_out = Vec::new();

    adaptive_traverse_resident_graph_sparse_queue_step_with_scratch_into(
        &dispatcher,
        &graph,
        &[1],
        u32::MAX,
        &mut scratch,
        &mut frontier_out,
    )
    .expect("Fix: recording dispatcher should complete resident sparse-queue sequence");

    let scratch_handles = scratch
        .handles
        .expect("Fix: sparse-queue resident step must allocate frontier/queue-len handles");
    let queue_handle = scratch
        .queue_handle
        .expect("Fix: sparse-queue resident step must allocate active queue");
    assert_eq!(dispatcher.last_upload_handles(), vec![scratch_handles[0]]);
    let steps = dispatcher.last_step_handles();
    assert_eq!(
        steps.len(),
        3,
        "sparse-queue traversal should initialize queue_len, compact packed frontier words while clearing output, then traverse"
    );
    assert_eq!(
        steps[0],
        vec![scratch_handles[2]],
        "first sparse-queue resident step must initialize queue_len on device"
    );
    assert_eq!(
        steps[1],
        vec![
            scratch_handles[0],
            queue_handle,
            scratch_handles[2],
            scratch_handles[1],
        ],
        "second sparse-queue resident step must compact packed words while clearing frontier_out"
    );
    assert_eq!(frontier_out, vec![0]);
}

#[test]
fn large_single_word_sparse_queue_resident_step_uses_atomic_materializer() {
    let dispatcher = RecordingResidentDispatcher::default();
    let node_count = 8_193u32;
    let words = vyre_primitives::bitset::bitset_words(node_count) as usize;
    let graph = ResidentAdaptiveTraversalGraph {
        node_count,
        edge_count: 0,
        max_row_degree: 0,
        high_degree_source_count: 0,
        words,
        layout_hash: 7,
        handles: [101, 102, 103, 104],
    };
    let mut scratch = AdaptiveTraversalResidentScratch::default();
    let mut frontier_in = vec![0u32; words];
    frontier_in[0] = 1;
    let mut frontier_out = Vec::new();

    adaptive_traverse_resident_graph_sparse_queue_step_with_scratch_into(
        &dispatcher,
        &graph,
        &frontier_in,
        u32::MAX,
        &mut scratch,
        &mut frontier_out,
    )
    .expect("Fix: recording dispatcher should complete large resident sparse-queue sequence");

    let scratch_handles = scratch
        .handles
        .expect("Fix: sparse-queue resident step must allocate frontier/queue-len handles");
    let queue_handle = scratch
        .queue_handle
        .expect("Fix: sparse-queue resident step must allocate active queue");
    assert!(scratch.word_partials_handle.is_none());
    assert!(scratch.word_block_totals_handle.is_none());
    assert_eq!(dispatcher.last_upload_handles(), vec![scratch_handles[0]]);
    let steps = dispatcher.last_step_handles();
    assert_eq!(
        steps.len(),
        3,
        "wide graph with one nonzero frontier word should keep the single-pass atomic word materializer"
    );
    assert_eq!(
        steps[0],
        vec![scratch_handles[2]],
        "first sparse-queue resident step must initialize queue_len on device"
    );
    assert_eq!(
        steps[1],
        vec![
            scratch_handles[0],
            queue_handle,
            scratch_handles[2],
            scratch_handles[1],
        ],
        "single-word sparse queue traversal must compact packed words while clearing frontier_out"
    );
    assert_eq!(frontier_out, vec![0; words]);
}

#[test]
fn large_dense_sparse_queue_resident_step_uses_word_prefix_materializer() {
    let dispatcher = RecordingResidentDispatcher::default();
    let node_count = 8_193u32;
    let words = vyre_primitives::bitset::bitset_words(node_count) as usize;
    let graph = ResidentAdaptiveTraversalGraph {
        node_count,
        edge_count: 0,
        max_row_degree: 0,
        high_degree_source_count: 0,
        words,
        layout_hash: 7,
        handles: [101, 102, 103, 104],
    };
    let mut scratch = AdaptiveTraversalResidentScratch::default();
    let frontier_in = vec![u32::MAX; words];
    let mut frontier_out = Vec::new();

    adaptive_traverse_resident_graph_sparse_queue_step_with_scratch_into(
        &dispatcher,
        &graph,
        &frontier_in,
        u32::MAX,
        &mut scratch,
        &mut frontier_out,
    )
    .expect("Fix: recording dispatcher should complete dense resident sparse-queue sequence");

    let scratch_handles = scratch
        .handles
        .expect("Fix: sparse-queue resident step must allocate frontier/queue-len handles");
    let queue_handle = scratch
        .queue_handle
        .expect("Fix: sparse-queue resident step must allocate active queue");
    let word_partials = scratch
        .word_partials_handle
        .expect("Fix: dense sparse-queue step must allocate word partials");
    let block_totals = scratch
        .word_block_totals_handle
        .expect("Fix: dense sparse-queue step must allocate block totals");
    assert_eq!(dispatcher.last_upload_handles(), vec![scratch_handles[0]]);
    let steps = dispatcher.last_step_handles();
    assert_eq!(
        steps.len(),
        4,
        "large dense sparse-queue traversal should clear output, scan words, scatter queue, then traverse"
    );
    assert_eq!(steps[0], vec![scratch_handles[1]]);
    assert_eq!(
        steps[1],
        vec![scratch_handles[0], word_partials, block_totals]
    );
    assert_eq!(
        steps[2],
        vec![
            scratch_handles[0],
            word_partials,
            block_totals,
            queue_handle,
            scratch_handles[2],
        ],
        "large dense sparse-queue traversal must use deterministic word-prefix queue scatter"
    );
    assert_eq!(frontier_out, vec![0; words]);
}

#[test]
fn small_multiblock_sparse_queue_resident_step_inlines_block_offsets() {
    let dispatcher = RecordingResidentDispatcher::default();
    let node_count = 32_897u32;
    let words = vyre_primitives::bitset::bitset_words(node_count) as usize;
    let graph = ResidentAdaptiveTraversalGraph {
        node_count,
        edge_count: 0,
        max_row_degree: 0,
        high_degree_source_count: 0,
        words,
        layout_hash: 11,
        handles: [101, 102, 103, 104],
    };
    let mut scratch = AdaptiveTraversalResidentScratch::default();
    let frontier_in = vec![u32::MAX; words];
    let mut frontier_out = Vec::new();

    adaptive_traverse_resident_graph_sparse_queue_step_with_scratch_into(
        &dispatcher,
        &graph,
        &frontier_in,
        u32::MAX,
        &mut scratch,
        &mut frontier_out,
    )
    .expect("Fix: recording dispatcher should complete multiblock resident sparse-queue sequence");

    let scratch_handles = scratch
        .handles
        .expect("Fix: sparse-queue resident step must allocate frontier/queue-len handles");
    let queue_handle = scratch
        .queue_handle
        .expect("Fix: sparse-queue resident step must allocate active queue");
    let word_partials = scratch
        .word_partials_handle
        .expect("Fix: multiblock sparse-queue step must allocate word partials");
    let block_totals = scratch
        .word_block_totals_handle
        .expect("Fix: multiblock sparse-queue step must allocate block totals");
    let steps = dispatcher.last_step_handles();
    assert_eq!(
        steps.len(),
        4,
        "small multiblock sparse-queue traversal should clear, count words, scatter with inline block offsets, then traverse"
    );
    assert_eq!(steps[0], vec![scratch_handles[1]]);
    assert_eq!(
        steps[1],
        vec![scratch_handles[0], word_partials, block_totals]
    );
    assert_eq!(
        steps[2],
        vec![
            scratch_handles[0],
            word_partials,
            block_totals,
            queue_handle,
            scratch_handles[2],
        ],
        "small multiblock sparse-queue traversal must scatter with inline block offsets"
    );
    assert_eq!(
        scratch.plan_cache_snapshot(),
        AdaptiveTraversalPlanCacheSnapshot {
            entries: 4,
            hits: 0,
            misses: 4,
        }
    );
    assert_eq!(frontier_out, vec![0; words]);
}

#[test]
fn many_block_sparse_queue_resident_step_scans_block_offsets_once() {
    let dispatcher = RecordingResidentDispatcher::default();
    let node_count = 262_177u32;
    let words = vyre_primitives::bitset::bitset_words(node_count) as usize;
    let graph = ResidentAdaptiveTraversalGraph {
        node_count,
        edge_count: 0,
        max_row_degree: 0,
        high_degree_source_count: 0,
        words,
        layout_hash: 11,
        handles: [101, 102, 103, 104],
    };
    let mut scratch = AdaptiveTraversalResidentScratch::default();
    let frontier_in = vec![u32::MAX; words];
    let mut frontier_out = Vec::new();

    adaptive_traverse_resident_graph_sparse_queue_step_with_scratch_into(
        &dispatcher,
        &graph,
        &frontier_in,
        u32::MAX,
        &mut scratch,
        &mut frontier_out,
    )
    .expect("Fix: recording dispatcher should complete many-block resident sparse-queue sequence");

    let scratch_handles = scratch
        .handles
        .expect("Fix: sparse-queue resident step must allocate frontier/queue-len handles");
    let queue_handle = scratch
        .queue_handle
        .expect("Fix: sparse-queue resident step must allocate active queue");
    let word_partials = scratch
        .word_partials_handle
        .expect("Fix: many-block sparse-queue step must allocate word partials");
    let block_totals = scratch
        .word_block_totals_handle
        .expect("Fix: many-block sparse-queue step must allocate block totals");
    let steps = dispatcher.last_step_handles();
    assert_eq!(
        steps.len(),
        5,
        "many-block sparse-queue traversal should clear, count words, scan block offsets once, scatter, then traverse"
    );
    assert_eq!(steps[0], vec![scratch_handles[1]]);
    assert_eq!(
        steps[1],
        vec![scratch_handles[0], word_partials, block_totals]
    );
    assert_eq!(
        steps[2],
        vec![block_totals],
        "many-block sparse-queue traversal must convert block totals into offsets once"
    );
    assert_eq!(
        steps[3],
        vec![
            scratch_handles[0],
            word_partials,
            block_totals,
            queue_handle,
            scratch_handles[2],
        ],
        "many-block sparse-queue traversal must scatter with precomputed block offsets"
    );
    assert_eq!(
        scratch.plan_cache_snapshot(),
        AdaptiveTraversalPlanCacheSnapshot {
            entries: 5,
            hits: 0,
            misses: 5,
        }
    );
    assert_eq!(frontier_out, vec![0; words]);
}
