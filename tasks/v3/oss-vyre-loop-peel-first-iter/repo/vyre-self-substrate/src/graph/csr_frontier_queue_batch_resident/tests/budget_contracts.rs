use super::super::*;
use super::support::RecordingBatchDispatcher;
use crate::csr_frontier_queue_resident::upload_resident_csr_queue_graph;
use crate::graph::csr_frontier_queue_scratch::STRIDED_FORWARD_MIN_ROW_DEGREE;

#[test]
fn budgeted_batch_memory_plan_uses_effective_queue_capacity() {
    let dispatcher = RecordingBatchDispatcher::default();
    let node_count = 4096u32;
    let edge_offsets = vec![0u32; node_count as usize + 1];
    let graph = upload_resident_csr_queue_graph(&dispatcher, node_count, &edge_offsets, &[], &[])
        .expect("Fix: zero-edge resident CSR graph is valid");
    let words = vyre_primitives::bitset::bitset_words(node_count) as usize;
    let mut one = vec![0u32; words];
    one[0] = 1;
    let frontiers: [&[u32]; 4] = [&one, &one, &one, &one];
    let mut scratch = ResidentCsrQueueBatchScratch::default();
    let mut outputs = Vec::new();

    let plan = run_resident_csr_queue_batch_budgeted_into(
        &dispatcher,
        &graph,
        &mut scratch,
        &frontiers,
        node_count,
        u32::MAX,
        2 * (words * std::mem::size_of::<u32>() * 2 + 2 * std::mem::size_of::<u32>()),
        &mut outputs,
    )
    .expect("Fix: sparse frontiers should fit a budget that graph-sized queues would exceed");

    assert_eq!(plan.query_count, frontiers.len());
    assert_eq!(
        plan.bytes_per_query,
        words * std::mem::size_of::<u32>() * 2 + 2 * std::mem::size_of::<u32>()
    );
    assert_eq!(plan.max_queries_per_dispatch, 2);
    assert_eq!(plan.dispatch_batches, 2);
    assert_eq!(
        outputs,
        vec![vec![0; words * std::mem::size_of::<u32>()]; 4]
    );
}

#[test]
fn budgeted_batch_memory_plan_accounts_for_split_high_queue_scratch() {
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
    .expect("Fix: skewed high-degree resident CSR graph is valid");
    let first = [0x1ffu32];
    let second = [0x1ffu32];
    let frontiers: [&[u32]; 2] = [&first, &second];
    let mut scratch = ResidentCsrQueueBatchScratch::default();
    let mut outputs = Vec::new();

    let plan = run_resident_csr_queue_batch_budgeted_into(
        &dispatcher,
        &graph,
        &mut scratch,
        &frontiers,
        1024,
        u32::MAX,
        152,
        &mut outputs,
    )
    .expect("Fix: split high-row scratch should fit one query per dispatch under this budget");

    assert_eq!(
        plan.bytes_per_query, 84,
        "budgeted split batches must count frontier, active queue, queue_len, frontier_out, high_queue, and high_len"
    );
    assert_eq!(plan.max_queries_per_dispatch, 1);
    assert_eq!(plan.dispatch_batches, 2);
    assert_eq!(plan.peak_batch_scratch_bytes, 84);
}

#[test]
fn budgeted_batch_packs_sparse_runs_around_dense_outlier() {
    let dispatcher = RecordingBatchDispatcher::default();
    let node_count = 4096u32;
    let edge_offsets = vec![0u32; node_count as usize + 1];
    let graph = upload_resident_csr_queue_graph(&dispatcher, node_count, &edge_offsets, &[], &[])
        .expect("Fix: zero-edge resident CSR graph is valid");
    let words = vyre_primitives::bitset::bitset_words(node_count) as usize;
    let mut sparse = vec![0u32; words];
    sparse[0] = 1;
    let dense = vec![u32::MAX; words];
    let frontiers: [&[u32]; 7] = [&sparse, &sparse, &sparse, &dense, &sparse, &sparse, &sparse];
    let mut scratch = ResidentCsrQueueBatchScratch::default();
    let mut outputs = Vec::new();
    let dense_bytes_per_query =
        words * std::mem::size_of::<u32>() * 2 + node_count as usize * 4 + 4;

    let plan = run_resident_csr_queue_batch_budgeted_into(
        &dispatcher,
        &graph,
        &mut scratch,
        &frontiers,
        node_count,
        u32::MAX,
        dense_bytes_per_query,
        &mut outputs,
    )
    .expect("Fix: sparse runs should pack into large chunks around one dense outlier");

    assert_eq!(plan.query_count, frontiers.len());
    assert_eq!(
        plan.max_queries_per_dispatch, 3,
        "sparse runs should not inherit the dense outlier's graph-sized queue capacity"
    );
    assert_eq!(plan.dispatch_batches, 3);
    assert_eq!(plan.bytes_per_query, dense_bytes_per_query);
    assert_eq!(plan.peak_batch_scratch_bytes, dense_bytes_per_query);
    assert_eq!(
        dispatcher
            .upload_handles
            .borrow()
            .iter()
            .map(Vec::len)
            .collect::<Vec<_>>(),
        vec![3, 1, 3],
        "budgeted dispatch should preserve order while packing sparse chunks on both sides of the dense frontier"
    );
    assert_eq!(
        outputs,
        vec![vec![0; words * std::mem::size_of::<u32>()]; frontiers.len()]
    );
}
