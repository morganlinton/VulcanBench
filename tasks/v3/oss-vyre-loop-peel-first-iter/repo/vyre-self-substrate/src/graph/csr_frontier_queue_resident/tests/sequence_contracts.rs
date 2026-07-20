use super::super::*;
use super::support::RecordingResidentDispatcher;

#[test]
fn resident_query_initializes_queue_len_on_device() {
    let dispatcher = RecordingResidentDispatcher::default();
    let graph = ResidentCsrQueueGraph {
        node_count: 1,
        edge_count: 0,
        max_row_degree: 0,
        high_degree_source_count: 0,
        words: 1,
        edge_offsets_handle: 101,
        edge_targets_handle: 102,
        edge_kind_mask_handle: 103,
    };
    let mut scratch = ResidentCsrQueueScratch::default();
    let mut output = Vec::new();

    run_resident_csr_queue_query_into(
        &dispatcher,
        &graph,
        &mut scratch,
        &[1],
        1,
        u32::MAX,
        &mut output,
    )
    .expect("Fix: recording dispatcher should complete resident CSR queue query");

    let handles = scratch
        .handles
        .expect("Fix: resident CSR queue query should allocate scratch handles");
    assert_eq!(
        dispatcher
            .sequence_upload_handles
            .borrow()
            .last()
            .cloned()
            .expect("Fix: expected one resident sequence"),
        vec![handles.frontier],
        "resident CSR queue query must only upload frontier bytes; queue_len and output clear must stay device-side"
    );
    let steps = dispatcher
        .sequence_step_handles
        .borrow()
        .last()
        .cloned()
        .expect("Fix: expected one resident step sequence");
    assert_eq!(
        steps.len(),
        3,
        "atomic-word resident CSR queue should initialize queue_len, compact packed words while clearing output, then traverse"
    );
    assert_eq!(steps[0], vec![handles.queue_len]);
    assert_eq!(
        steps[1],
        vec![
            handles.frontier,
            handles.active_queue,
            handles.queue_len,
            handles.frontier_out,
        ]
    );
    assert_eq!(output, vec![0, 0, 0, 0]);
}
