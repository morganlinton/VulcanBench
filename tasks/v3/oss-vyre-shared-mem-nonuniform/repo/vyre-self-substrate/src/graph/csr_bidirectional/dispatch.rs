use super::{BidirectionalGpuScratch, CachedBidirectionalProgram};
use vyre_foundation::ir::Program;
use vyre_primitives::graph::csr_bidirectional::{
    plan_csr_bidirectional_step, CsrBidirectionalDispatchPlan, CsrBidirectionalStaticInputKey,
    CSR_BIDIRECTIONAL_FRONTIER_OUT_BUFFER, CSR_BIDIRECTIONAL_NODES_BUFFER,
    CSR_BIDIRECTIONAL_NODE_TAGS_BUFFER,
};

use crate::graph::dispatch_bridge::{
    dispatch_single_u32_output_from_prepared_into, refresh_keyed_dispatch_inputs, DispatchInput,
};
use crate::optimizer::dispatcher::{DispatchError, OptimizerDispatcher};

/// Dispatcher-backed bidirectional CSR step.
///
/// # Errors
///
/// Propagates dispatch failures and rejects malformed CSR/frontier
/// shapes or truncated readback.
pub fn bidirectional_step_via(
    dispatcher: &dyn OptimizerDispatcher,
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    frontier_in: &[u32],
    allow_mask: u32,
) -> Result<Vec<u32>, DispatchError> {
    let mut out = Vec::new();
    bidirectional_step_via_into(
        dispatcher,
        node_count,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        frontier_in,
        allow_mask,
        &mut out,
    )?;
    Ok(out)
}

/// Dispatcher-backed bidirectional CSR step into caller-owned storage.
#[allow(clippy::too_many_arguments)]
pub fn bidirectional_step_via_into(
    dispatcher: &dyn OptimizerDispatcher,
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    frontier_in: &[u32],
    allow_mask: u32,
    out: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    let mut scratch = BidirectionalGpuScratch::default();
    bidirectional_step_via_with_scratch_into(
        dispatcher,
        node_count,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        frontier_in,
        allow_mask,
        &mut scratch,
        out,
    )
}

/// Dispatcher-backed bidirectional CSR step with caller-owned scratch.
#[allow(clippy::too_many_arguments)]
pub fn bidirectional_step_via_with_scratch_into(
    dispatcher: &dyn OptimizerDispatcher,
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    frontier_in: &[u32],
    allow_mask: u32,
    scratch: &mut BidirectionalGpuScratch,
    out: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    let plan = plan_csr_bidirectional_step(
        node_count,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        frontier_in,
        allow_mask,
    )
    .map_err(DispatchError::BadInputs)?;
    if node_count == 0 {
        out.clear();
        return Ok(());
    }
    let BidirectionalGpuScratch {
        inputs,
        static_input_key,
        program_cache,
    } = scratch;
    let program_key = plan.program_key();
    let static_key = plan
        .static_input_key(edge_offsets, edge_targets, edge_kind_mask)
        .map_err(DispatchError::BadInputs)?;
    let cached = program_cache.get_or_insert_with(program_key, || CachedBidirectionalProgram {
        program: plan.program(),
    });
    refresh_bidirectional_step_inputs(
        inputs,
        static_input_key,
        static_key,
        &plan,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        frontier_in,
    )?;
    bidirectional_step_dispatch_prepared_inputs_into(
        dispatcher,
        &plan,
        &cached.program,
        inputs,
        out,
    )
}

pub(super) fn bidirectional_step_dispatch_prepared_inputs_into(
    dispatcher: &dyn OptimizerDispatcher,
    plan: &CsrBidirectionalDispatchPlan,
    program: &Program,
    inputs: &[Vec<u8>],
    out: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    dispatch_single_u32_output_from_prepared_into(
        dispatcher,
        program,
        inputs,
        plan.frontier_words,
        CSR_BIDIRECTIONAL_FRONTIER_OUT_BUFFER,
        Some(plan.grid),
        out,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn refresh_bidirectional_step_inputs(
    inputs: &mut Vec<Vec<u8>>,
    static_input_key: &mut Option<CsrBidirectionalStaticInputKey>,
    next_static_input_key: CsrBidirectionalStaticInputKey,
    plan: &CsrBidirectionalDispatchPlan,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    frontier_in: &[u32],
) -> Result<(), DispatchError> {
    refresh_keyed_dispatch_inputs(
        inputs,
        static_input_key,
        next_static_input_key,
        &[
            DispatchInput::zero_u32_words(plan.node_words, CSR_BIDIRECTIONAL_NODES_BUFFER),
            DispatchInput::u32_slice(edge_offsets),
            DispatchInput::u32_slice_or_zero_words(
                edge_targets,
                plan.edge_storage_words,
                "csr_bidirectional edge_targets",
            ),
            DispatchInput::u32_slice_or_zero_words(
                edge_kind_mask,
                plan.edge_storage_words,
                "csr_bidirectional edge_kind_mask",
            ),
            DispatchInput::zero_u32_words(plan.node_words, CSR_BIDIRECTIONAL_NODE_TAGS_BUFFER),
            DispatchInput::u32_slice(frontier_in),
            DispatchInput::ZeroU32Words {
                words: plan.frontier_words,
                context: CSR_BIDIRECTIONAL_FRONTIER_OUT_BUFFER,
            },
        ],
        &[
            (5, DispatchInput::u32_slice(frontier_in)),
            (
                6,
                DispatchInput::zero_u32_words(
                    plan.frontier_words,
                    CSR_BIDIRECTIONAL_FRONTIER_OUT_BUFFER,
                ),
            ),
        ],
    )
}
