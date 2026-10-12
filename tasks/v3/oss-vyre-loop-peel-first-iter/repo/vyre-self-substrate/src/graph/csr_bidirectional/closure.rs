use super::{BidirectionalGpuScratch, CachedBidirectionalProgram};
use crate::graph::csr_bidirectional::dispatch::{
    bidirectional_step_dispatch_prepared_inputs_into, refresh_bidirectional_step_inputs,
};
use crate::optimizer::dispatcher::{DispatchError, OptimizerDispatcher};
use vyre_primitives::graph::csr_bidirectional::{
    plan_csr_bidirectional_step, run_csr_bidirectional_closure_plan_with_step,
};

/// Dispatcher-backed bidirectional closure.
///
/// # Errors
///
/// Propagates dispatch failures from each bidirectional step.
#[allow(clippy::too_many_arguments)]
pub fn bidirectional_closure_via(
    dispatcher: &dyn OptimizerDispatcher,
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    seed: &[u32],
    allow_mask: u32,
    max_iters: u32,
) -> Result<Vec<u32>, DispatchError> {
    let mut current = Vec::new();
    let mut next = Vec::new();
    bidirectional_closure_via_into(
        dispatcher,
        node_count,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        seed,
        allow_mask,
        max_iters,
        &mut current,
        &mut next,
    )?;
    Ok(current)
}

/// Dispatcher-backed bidirectional closure using caller-owned buffers.
#[allow(clippy::too_many_arguments)]
pub fn bidirectional_closure_via_into(
    dispatcher: &dyn OptimizerDispatcher,
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    seed: &[u32],
    allow_mask: u32,
    max_iters: u32,
    current: &mut Vec<u32>,
    next: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    let mut scratch = BidirectionalGpuScratch::default();
    bidirectional_closure_via_with_scratch_into(
        dispatcher,
        node_count,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        seed,
        allow_mask,
        max_iters,
        &mut scratch,
        current,
        next,
    )
}

/// Dispatcher-backed bidirectional closure with caller-owned dispatch scratch.
#[allow(clippy::too_many_arguments)]
pub fn bidirectional_closure_via_with_scratch_into(
    dispatcher: &dyn OptimizerDispatcher,
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    seed: &[u32],
    allow_mask: u32,
    max_iters: u32,
    scratch: &mut BidirectionalGpuScratch,
    current: &mut Vec<u32>,
    next: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    let plan = plan_csr_bidirectional_step(
        node_count,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        seed,
        allow_mask,
    )
    .map_err(DispatchError::BadInputs)?;
    let BidirectionalGpuScratch {
        inputs,
        static_input_key,
        program_cache,
    } = scratch;
    let program_key = plan.program_key();
    let static_key = plan
        .static_input_key(edge_offsets, edge_targets, edge_kind_mask)
        .map_err(DispatchError::BadInputs)?;
    run_csr_bidirectional_closure_plan_with_step(
        &plan,
        seed,
        max_iters,
        current,
        next,
        DispatchError::BadInputs,
        |frontier, step_out| {
            let cached =
                program_cache.get_or_insert_with(program_key, || CachedBidirectionalProgram {
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
                frontier,
            )?;
            bidirectional_step_dispatch_prepared_inputs_into(
                dispatcher,
                &plan,
                &cached.program,
                inputs,
                step_out,
            )
        },
    )
}
