use super::ResidentCsrQueueGraph;
use crate::graph::csr_frontier_queue_scratch::resident_csr_queue_high_degree_source_count;
use vyre_primitives::graph::csr_frontier_queue::validate_csr_queue_graph;

use crate::graph::dispatch_bridge::{upload_resident_dispatch_inputs, DispatchInput};
use crate::optimizer::dispatcher::{DispatchError, OptimizerDispatcher};

/// Upload a CSR graph into resident device buffers once.
pub fn upload_resident_csr_queue_graph(
    dispatcher: &dyn OptimizerDispatcher,
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
) -> Result<ResidentCsrQueueGraph, DispatchError> {
    let layout = validate_csr_queue_graph(node_count, edge_offsets, edge_targets, edge_kind_mask)
        .map_err(DispatchError::BadInputs)?;
    let mut payload_storage = Vec::new();
    let [edge_offsets_handle, edge_targets_handle, edge_kind_mask_handle] =
        upload_resident_dispatch_inputs(
            dispatcher,
            &mut payload_storage,
            [
                DispatchInput::u32_slice(edge_offsets),
                DispatchInput::u32_slice_or_zero_words(
                    edge_targets,
                    layout.edge_storage_words,
                    "resident CSR queue graph edge_targets",
                ),
                DispatchInput::u32_slice_or_zero_words(
                    edge_kind_mask,
                    layout.edge_storage_words,
                    "resident CSR queue graph edge_kind_mask",
                ),
            ],
        )?;
    Ok(ResidentCsrQueueGraph {
        node_count: layout.node_count,
        edge_count: layout.edge_count,
        max_row_degree: layout.max_row_degree,
        high_degree_source_count: resident_csr_queue_high_degree_source_count(edge_offsets),
        words: layout.words,
        edge_offsets_handle,
        edge_targets_handle,
        edge_kind_mask_handle,
    })
}
