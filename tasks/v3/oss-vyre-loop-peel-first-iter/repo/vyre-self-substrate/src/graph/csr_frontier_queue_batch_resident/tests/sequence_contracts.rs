use super::super::*;
use super::support::RecordingBatchDispatcher;
use crate::csr_frontier_queue_resident::upload_resident_csr_queue_graph;

#[test]
fn batch_queries_initialize_queue_len_on_device() {
    let dispatcher = RecordingBatchDispatcher::default();
    let graph = upload_resident_csr_queue_graph(&dispatcher, 2, &[0, 0, 0], &[], &[])
        .expect("Fix: zero-edge resident CSR graph is valid");
    let mut scratch = ResidentCsrQueueBatchScratch::default();
    let first = [1u32];
    let second = [2u32];
    let frontiers: [&[u32]; 2] = [&first, &second];
    let mut outputs = Vec::new();

    run_resident_csr_queue_batch_into(
        &dispatcher,
        &graph,
        &mut scratch,
        &frontiers,
        2,
        u32::MAX,
        &mut outputs,
    )
    .expect("Fix: recording dispatcher should complete resident CSR queue batch");

    let expected_uploads: Vec<u64> = scratch
        .handles
        .iter()
        .map(|handles| handles.frontier)
        .collect();
    assert_eq!(
        dispatcher
            .upload_handles
            .borrow()
            .last()
            .cloned()
            .expect("Fix: expected one resident upload sequence"),
        expected_uploads,
        "batch CSR queue traversal must only upload per-query frontier bytes; queue_len and output clear must stay device-side"
    );
    let steps = dispatcher
        .step_handles
        .borrow()
        .last()
        .cloned()
        .expect("Fix: expected one resident step sequence");
    assert_eq!(
        steps.len(),
        6,
        "atomic-word resident CSR queue batches should initialize queue_len, compact packed words while clearing output, then traverse per query"
    );
    assert_eq!(steps[0], vec![scratch.handles[0].queue_len]);
    assert_eq!(
        steps[1],
        vec![
            scratch.handles[0].frontier,
            scratch.handles[0].active_queue,
            scratch.handles[0].queue_len,
            scratch.handles[0].frontier_out,
        ]
    );
    assert_eq!(steps[3], vec![scratch.handles[1].queue_len]);
    assert_eq!(
        steps[4],
        vec![
            scratch.handles[1].frontier,
            scratch.handles[1].active_queue,
            scratch.handles[1].queue_len,
            scratch.handles[1].frontier_out,
        ]
    );
    assert_eq!(outputs, vec![vec![0; 4], vec![0; 4]]);
}
