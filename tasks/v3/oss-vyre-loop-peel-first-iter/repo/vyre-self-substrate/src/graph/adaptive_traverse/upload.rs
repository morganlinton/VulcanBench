use super::state::{adaptive_four_russians_layout_hash, adaptive_traversal_layout_hash};
use super::{
    ResidentAdaptiveFourRussiansDenseGraph, ResidentAdaptiveSparseQueueGraph,
    ResidentAdaptiveTraversalGraph,
};

use crate::graph::csr_frontier_queue_scratch::resident_csr_queue_high_degree_source_count;
use crate::graph::dispatch_bridge::{upload_resident_dispatch_inputs, DispatchInput};
use crate::optimizer::dispatcher::{DispatchError, OptimizerDispatcher};
use vyre_primitives::bitset::bitset_words;
use vyre_primitives::graph::adaptive_traverse::{
    adaptive_sparse_queue_graph_content_hash as adaptive_sparse_queue_layout_hash,
    four_russians_dense_lut_from_adj_rows as primitive_four_russians_dense_lut_from_adj_rows,
    validate_adaptive_traversal_layout,
};
use vyre_primitives::graph::csr_frontier_queue::validate_csr_queue_graph;

/// Upload CSR plus dense reverse-adjacency rows once into resident buffers.
///
/// # Errors
///
/// Rejects malformed graph layouts or dispatchers without resident support.
pub fn upload_resident_adaptive_traversal_graph(
    dispatcher: &dyn OptimizerDispatcher,
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    adj_rows_dense: &[u32],
) -> Result<ResidentAdaptiveTraversalGraph, DispatchError> {
    let layout = validate_adaptive_traversal_layout(
        node_count,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        adj_rows_dense,
    )
    .map_err(DispatchError::BadInputs)?;

    let mut payload_storage = Vec::new();
    let handles = upload_resident_dispatch_inputs(
        dispatcher,
        &mut payload_storage,
        [
            DispatchInput::u32_slice(edge_offsets),
            DispatchInput::u32_slice_or_zero_words(
                edge_targets,
                layout.edge_storage_words,
                "resident adaptive traversal edge_targets",
            ),
            DispatchInput::u32_slice_or_zero_words(
                edge_kind_mask,
                layout.edge_storage_words,
                "resident adaptive traversal edge_kind_mask",
            ),
            DispatchInput::u32_slice(adj_rows_dense),
        ],
    )?;

    Ok(ResidentAdaptiveTraversalGraph {
        node_count,
        edge_count: layout.edge_count,
        max_row_degree: layout.max_row_degree,
        high_degree_source_count: resident_csr_queue_high_degree_source_count(edge_offsets),
        words: layout.words,
        layout_hash: adaptive_traversal_layout_hash(
            node_count,
            edge_offsets,
            edge_targets,
            edge_kind_mask,
            adj_rows_dense,
        ),
        handles,
    })
}

/// Upload CSR graph buffers for adaptive sparse-queue traversal without dense rows.
///
/// # Errors
///
/// Rejects malformed CSR graph layouts or dispatchers without resident support.
pub fn upload_resident_adaptive_sparse_queue_graph(
    dispatcher: &dyn OptimizerDispatcher,
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
) -> Result<ResidentAdaptiveSparseQueueGraph, DispatchError> {
    let layout = validate_csr_queue_graph(node_count, edge_offsets, edge_targets, edge_kind_mask)
        .map_err(DispatchError::BadInputs)?;

    let mut payload_storage = Vec::new();
    let handles = upload_resident_dispatch_inputs(
        dispatcher,
        &mut payload_storage,
        [
            DispatchInput::u32_slice(edge_offsets),
            DispatchInput::u32_slice_or_zero_words(
                edge_targets,
                layout.edge_storage_words,
                "resident adaptive sparse-queue edge_targets",
            ),
            DispatchInput::u32_slice_or_zero_words(
                edge_kind_mask,
                layout.edge_storage_words,
                "resident adaptive sparse-queue edge_kind_mask",
            ),
        ],
    )?;

    Ok(ResidentAdaptiveSparseQueueGraph {
        node_count,
        edge_count: layout.edge_count,
        max_row_degree: layout.max_row_degree,
        high_degree_source_count: resident_csr_queue_high_degree_source_count(edge_offsets),
        words: layout.words,
        layout_hash: adaptive_sparse_queue_layout_hash(
            node_count,
            edge_offsets,
            edge_targets,
            edge_kind_mask,
        ),
        handles,
    })
}

/// Upload a reusable Four-Russians dense traversal LUT into resident memory.
///
/// # Errors
///
/// Rejects malformed dense reverse-adjacency rows or dispatchers without
/// resident support.
pub fn upload_resident_adaptive_four_russians_dense_graph(
    dispatcher: &dyn OptimizerDispatcher,
    node_count: u32,
    adj_rows_dense: &[u32],
) -> Result<ResidentAdaptiveFourRussiansDenseGraph, DispatchError> {
    let lut = primitive_four_russians_dense_lut_from_adj_rows(node_count, adj_rows_dense)
        .map_err(DispatchError::BadInputs)?;
    let words = bitset_words(node_count) as usize;

    let mut payload_storage = Vec::new();
    let [lut_handle] = upload_resident_dispatch_inputs(
        dispatcher,
        &mut payload_storage,
        [DispatchInput::u32_slice(&lut)],
    )?;

    Ok(ResidentAdaptiveFourRussiansDenseGraph {
        node_count,
        words,
        layout_hash: adaptive_four_russians_layout_hash(node_count, adj_rows_dense),
        lut_handle,
    })
}
