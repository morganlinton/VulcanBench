use super::super::*;
use super::support::RecordingResidentDispatcher;
use crate::graph::csr_frontier_queue_scratch::STRIDED_FORWARD_MIN_ROW_DEGREE;

#[test]
fn sparse_queue_step_accepts_csr_only_resident_graph() {
    let dispatcher = RecordingResidentDispatcher::default();
    let node_count = 4u32;
    let edge_offsets = [0, 1, 1, 1, 1];
    let edge_targets = [2];
    let edge_kind_mask = [1];
    let graph = upload_resident_adaptive_sparse_queue_graph(
        &dispatcher,
        node_count,
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
    )
    .expect("Fix: CSR-only adaptive sparse-queue graph upload should accept canonical CSR");
    let graph_handles = graph.handles();
    let mut scratch = AdaptiveTraversalResidentScratch::default();
    let frontier_in = [1u32];
    let mut frontier_out = Vec::new();

    adaptive_traverse_resident_sparse_queue_step_with_scratch_into(
        &dispatcher,
        &graph,
        &frontier_in,
        1,
        &mut scratch,
        &mut frontier_out,
    )
    .expect("Fix: CSR-only adaptive sparse-queue resident step should dispatch");

    let scratch_handles = scratch
        .handles
        .expect("Fix: sparse queue step should allocate frontier scratch");
    let queue_handle = scratch
        .queue_handle
        .expect("Fix: sparse queue step should allocate active queue");
    let steps = dispatcher.last_step_handles();
    assert_eq!(steps.len(), 3);
    assert_eq!(
        steps[2],
        vec![
            queue_handle,
            scratch_handles[2],
            graph_handles[0],
            graph_handles[1],
            graph_handles[2],
            scratch_handles[1],
        ],
        "CSR-only sparse queue traversal must bind only CSR graph handles"
    );
    assert_eq!(frontier_out, vec![0]);
}

#[test]
fn sparse_queue_step_sizes_active_queue_from_frontier_popcount() {
    let dispatcher = RecordingResidentDispatcher::default();
    let node_count = 8_000u32;
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
    .expect("Fix: recording dispatcher should complete sparse queue traversal");

    assert_eq!(
        scratch.queue_bytes,
        std::mem::size_of::<u32>(),
        "single-source frontier must not allocate a graph-sized active queue"
    );
    assert_eq!(
        dispatcher.resident_alloc_lengths().last().copied(),
        Some(std::mem::size_of::<u32>()),
        "active queue allocation should be sized from frontier popcount"
    );
    assert_eq!(frontier_out, vec![0; words]);
}

#[test]
fn sparse_queue_step_reuses_larger_queue_scratch_for_smaller_frontier() {
    let dispatcher = RecordingResidentDispatcher::default();
    let node_count = 4096u32;
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
    let mut larger_frontier = vec![0u32; words];
    for node in 0..300u32 {
        larger_frontier[(node / 32) as usize] |= 1 << (node % 32);
    }
    let mut frontier_out = Vec::new();

    adaptive_traverse_resident_graph_sparse_queue_step_with_scratch_into(
        &dispatcher,
        &graph,
        &larger_frontier,
        u32::MAX,
        &mut scratch,
        &mut frontier_out,
    )
    .expect("Fix: recording dispatcher should complete larger sparse queue traversal");

    let queue_handle = scratch
        .queue_handle
        .expect("Fix: sparse queue step must allocate active queue");
    assert_eq!(scratch.queue_bytes, 512 * std::mem::size_of::<u32>());
    let allocs_after_large = dispatcher.alloc_count.get();
    let mut single_frontier = vec![0u32; words];
    single_frontier[0] = 1;

    adaptive_traverse_resident_graph_sparse_queue_step_with_scratch_into(
        &dispatcher,
        &graph,
        &single_frontier,
        u32::MAX,
        &mut scratch,
        &mut frontier_out,
    )
    .expect("Fix: recording dispatcher should complete smaller sparse queue traversal");

    assert_eq!(scratch.queue_handle, Some(queue_handle));
    assert_eq!(
        scratch.queue_bytes,
        512 * std::mem::size_of::<u32>(),
        "scratch should keep the larger reusable queue buffer instead of shrinking"
    );
    assert_eq!(
        dispatcher.alloc_count.get(),
        allocs_after_large,
        "smaller frontiers should reuse the existing resident queue allocation"
    );
    assert!(
        dispatcher.freed.borrow().is_empty(),
        "smaller frontiers should not free and reallocate resident queue scratch"
    );
}

#[test]
fn skewed_high_degree_sparse_queue_step_uses_bounded_split_queue() {
    let dispatcher = RecordingResidentDispatcher::default();
    let node_count = 2048u32;
    let words = vyre_primitives::bitset::bitset_words(node_count) as usize;
    let graph = ResidentAdaptiveTraversalGraph {
        node_count,
        edge_count: STRIDED_FORWARD_MIN_ROW_DEGREE,
        max_row_degree: STRIDED_FORWARD_MIN_ROW_DEGREE,
        high_degree_source_count: 1,
        words,
        layout_hash: 13,
        handles: [101, 102, 103, 104],
    };
    let mut scratch = AdaptiveTraversalResidentScratch::default();
    let mut frontier_in = vec![0u32; words];
    for node in 0..9u32 {
        frontier_in[(node / 32) as usize] |= 1 << (node % 32);
    }
    let mut frontier_out = Vec::new();

    adaptive_traverse_resident_graph_sparse_queue_step_with_scratch_into(
        &dispatcher,
        &graph,
        &frontier_in,
        u32::MAX,
        &mut scratch,
        &mut frontier_out,
    )
    .expect("Fix: recording dispatcher should complete high-degree sparse queue traversal");

    assert_eq!(scratch.queue_bytes, 16 * std::mem::size_of::<u32>());
    assert_eq!(
        scratch.high_queue_bytes,
        std::mem::size_of::<u32>(),
        "a graph with one possible hub must not launch a strided team for every active source"
    );
    let high_queue = scratch
        .high_queue_handle
        .expect("Fix: mixed split traversal should allocate a high-degree queue");
    let high_len = scratch
        .high_len_handle
        .expect("Fix: mixed split traversal should allocate a high-degree queue length");
    let scratch_handles = scratch
        .handles
        .expect("Fix: mixed split traversal should allocate frontier scratch");
    let active_queue = scratch
        .queue_handle
        .expect("Fix: mixed split traversal should allocate active queue scratch");
    let steps = dispatcher.last_step_handles();
    assert_eq!(
        steps.len(),
        5,
        "skewed sparse queue traversal should materialize active sources, clear high_len, split low/high rows, then traverse only bounded high rows"
    );
    assert_eq!(
        steps[3],
        vec![
            active_queue,
            scratch_handles[2],
            graph.handles[0],
            graph.handles[1],
            graph.handles[2],
            scratch_handles[1],
            high_queue,
            high_len,
        ],
        "split-low pass must read the active queue and write only bounded high-row scratch"
    );
    assert_eq!(
        steps[4],
        vec![
            high_queue,
            high_len,
            graph.handles[0],
            graph.handles[1],
            graph.handles[2],
            scratch_handles[1],
        ],
        "strided follow-up must consume the bounded high-row queue, not the whole active queue"
    );
    assert_eq!(
        dispatcher.last_step_grids()[4],
        Some(vyre_primitives::graph::csr_queue_strided::csr_queue_strided_forward_dispatch_grid(1)),
        "skewed high-degree sparse queue traversal must launch row-strided teams only for the graph-wide high-row bound"
    );
}

#[test]
fn single_superhub_csr_only_sparse_queue_sizes_split_queue_from_high_row_count() {
    let dispatcher = RecordingResidentDispatcher::default();
    let node_count = 2048u32;
    let words = vyre_primitives::bitset::bitset_words(node_count) as usize;
    let hub_degree = STRIDED_FORWARD_MIN_ROW_DEGREE * 9;
    let mut edge_offsets = vec![hub_degree; node_count as usize + 1];
    edge_offsets[0] = 0;
    let edge_targets = vec![1u32; hub_degree as usize];
    let edge_kind_mask = vec![1u32; hub_degree as usize];
    let graph = upload_resident_adaptive_sparse_queue_graph(
        &dispatcher,
        node_count,
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
    )
    .expect("Fix: CSR-only adaptive sparse queue graph should accept a one-superhub graph");
    let mut scratch = AdaptiveTraversalResidentScratch::default();
    let mut frontier_in = vec![0u32; words];
    for node in 0..9u32 {
        frontier_in[(node / 32) as usize] |= 1 << (node % 32);
    }
    let mut frontier_out = Vec::new();

    adaptive_traverse_resident_sparse_queue_step_with_scratch_into(
        &dispatcher,
        &graph,
        &frontier_in,
        u32::MAX,
        &mut scratch,
        &mut frontier_out,
    )
    .expect("Fix: CSR-only adaptive sparse queue step should complete one-superhub traversal");

    assert_eq!(graph.high_degree_source_count(), 1);
    assert_eq!(scratch.queue_bytes, 16 * std::mem::size_of::<u32>());
    assert_eq!(
        scratch.high_queue_bytes,
        std::mem::size_of::<u32>(),
        "one enormous row should allocate one adaptive high-row slot, not edge_count / threshold slots"
    );
    assert_eq!(
        dispatcher.last_step_grids()[4],
        Some(vyre_primitives::graph::csr_queue_strided::csr_queue_strided_forward_dispatch_grid(1)),
        "one-superhub adaptive traversal must launch only one row-strided team"
    );
}

#[test]
fn uniformly_high_degree_sparse_queue_step_keeps_global_strided_consumer() {
    let dispatcher = RecordingResidentDispatcher::default();
    let node_count = 2048u32;
    let words = vyre_primitives::bitset::bitset_words(node_count) as usize;
    let queue_slots = 16u32;
    let graph = ResidentAdaptiveTraversalGraph {
        node_count,
        edge_count: STRIDED_FORWARD_MIN_ROW_DEGREE * queue_slots,
        max_row_degree: STRIDED_FORWARD_MIN_ROW_DEGREE,
        high_degree_source_count: queue_slots,
        words,
        layout_hash: 17,
        handles: [101, 102, 103, 104],
    };
    let mut scratch = AdaptiveTraversalResidentScratch::default();
    let mut frontier_in = vec![0u32; words];
    for node in 0..9u32 {
        frontier_in[(node / 32) as usize] |= 1 << (node % 32);
    }
    let mut frontier_out = Vec::new();

    adaptive_traverse_resident_graph_sparse_queue_step_with_scratch_into(
        &dispatcher,
        &graph,
        &frontier_in,
        u32::MAX,
        &mut scratch,
        &mut frontier_out,
    )
    .expect("Fix: recording dispatcher should complete uniformly high-degree traversal");

    assert!(scratch.high_queue_handle.is_none());
    assert!(scratch.high_len_handle.is_none());
    assert_eq!(
        scratch.queue_bytes,
        queue_slots as usize * std::mem::size_of::<u32>()
    );
    assert_eq!(dispatcher.last_step_handles().len(), 3);
    assert_eq!(
        dispatcher.last_step_grids()[2],
        Some(vyre_primitives::graph::csr_queue_strided::csr_queue_strided_forward_dispatch_grid(
            queue_slots
        )),
        "uniformly high-degree sparse queue traversal should keep the single row-strided consumer"
    );
}
