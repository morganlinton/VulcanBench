use super::super::*;
use super::support::RecordingResidentDispatcher;
use crate::graph::csr_frontier_queue_scratch::STRIDED_FORWARD_MIN_ROW_DEGREE;

#[test]
fn sparse_queue_graph_upload_skips_dense_adjacency_rows() {
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

    assert_eq!(graph.node_count(), node_count);
    assert_eq!(graph.edge_count(), 1);
    assert_eq!(graph.words(), 1);
    assert_eq!(dispatcher.alloc_count.get(), 3);
    assert_eq!(
        dispatcher.resident_upload_lengths(),
        vec![
            edge_offsets.len() * std::mem::size_of::<u32>(),
            edge_targets.len() * std::mem::size_of::<u32>(),
            edge_kind_mask.len() * std::mem::size_of::<u32>(),
        ],
        "CSR-only sparse-queue upload must not allocate or upload dense adjacency rows"
    );
}

#[test]
fn adaptive_upload_records_exact_high_degree_source_count() {
    let dispatcher = RecordingResidentDispatcher::default();
    let node_count = 5u32;
    let degrees = [
        STRIDED_FORWARD_MIN_ROW_DEGREE,
        STRIDED_FORWARD_MIN_ROW_DEGREE - 1,
        STRIDED_FORWARD_MIN_ROW_DEGREE + 11,
        0,
        3,
    ];
    let mut edge_offsets = Vec::with_capacity(degrees.len() + 1);
    let mut edge_targets = Vec::new();
    let mut edge_kind_mask = Vec::new();
    edge_offsets.push(0);
    for degree in degrees {
        edge_targets.extend((0..degree).map(|edge| edge % node_count));
        edge_kind_mask.extend(std::iter::repeat(1).take(degree as usize));
        edge_offsets.push(edge_targets.len() as u32);
    }
    let adj_rows_dense = vec![0u32; node_count as usize];

    let full_graph = upload_resident_adaptive_traversal_graph(
        &dispatcher,
        node_count,
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
        &adj_rows_dense,
    )
    .expect("Fix: full adaptive resident upload should accept high-degree CSR");
    let sparse_graph = upload_resident_adaptive_sparse_queue_graph(
        &dispatcher,
        node_count,
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
    )
    .expect("Fix: CSR-only adaptive resident upload should accept high-degree CSR");

    assert_eq!(
        full_graph.high_degree_source_count(),
        2,
        "full adaptive graph metadata must count high-degree rows exactly"
    );
    assert_eq!(
        sparse_graph.high_degree_source_count(),
        2,
        "CSR-only adaptive graph metadata must count high-degree rows exactly"
    );
}
