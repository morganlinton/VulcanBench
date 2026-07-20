use super::super::*;
use super::support::RecordingResidentDispatcher;
use crate::graph::csr_frontier_queue_scratch::STRIDED_FORWARD_MIN_ROW_DEGREE;
use crate::optimizer::dispatcher::DispatchError;

#[test]
fn zero_edge_graph_uploads_padded_resident_edge_buffers() {
    let dispatcher = RecordingResidentDispatcher::default();
    let graph = upload_resident_csr_queue_graph(&dispatcher, 3, &[0, 0, 0, 0], &[], &[])
        .expect("Fix: zero-edge resident CSR graph is valid");

    assert_eq!(graph.edge_count(), 0);
    assert_eq!(graph.high_degree_source_count(), 0);
    assert_eq!(*dispatcher.allocs.borrow(), vec![16, 4, 4]);
    assert_eq!(
        *dispatcher.uploads.borrow(),
        vec![vec![0; 16], vec![0; 4], vec![0; 4]]
    );
}

#[test]
fn resident_upload_records_exact_high_degree_source_count() {
    let dispatcher = RecordingResidentDispatcher::default();
    let mut edge_offsets = Vec::new();
    let mut edge_targets = Vec::new();
    let mut edge_kind_mask = Vec::new();
    edge_offsets.push(0);
    for degree in [
        STRIDED_FORWARD_MIN_ROW_DEGREE,
        STRIDED_FORWARD_MIN_ROW_DEGREE - 1,
        STRIDED_FORWARD_MIN_ROW_DEGREE + 7,
        0,
        2,
    ] {
        edge_targets.extend((0..degree).map(|edge| edge % 5));
        edge_kind_mask.extend(std::iter::repeat(1).take(degree as usize));
        edge_offsets.push(edge_targets.len() as u32);
    }

    let graph = upload_resident_csr_queue_graph(
        &dispatcher,
        5,
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
    )
    .expect("Fix: high-degree resident CSR graph upload should validate");

    assert_eq!(graph.max_row_degree(), STRIDED_FORWARD_MIN_ROW_DEGREE + 7);
    assert_eq!(
        graph.high_degree_source_count(),
        2,
        "resident graph metadata must count rows, not infer high-row capacity from total edge count"
    );
}

#[test]
fn resident_upload_uses_primitive_csr_validation() {
    let dispatcher = RecordingResidentDispatcher::default();
    let err = upload_resident_csr_queue_graph(&dispatcher, 2, &[0, 1, 1], &[5], &[1])
        .expect_err("out-of-range targets must be rejected before upload");
    assert!(
        matches!(err, DispatchError::BadInputs(message) if message.contains("outside node_count"))
    );
    assert!(dispatcher.allocs.borrow().is_empty());
    assert!(dispatcher.uploads.borrow().is_empty());
}
