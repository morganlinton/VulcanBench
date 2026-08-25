use super::{
    CachedDominanceFrontierProgram, DominanceFrontierGpuScratch, DominatorFrontierStaticInputKey,
};
use crate::graph::dispatch_bridge::{
    dispatch_single_u32_output_from_prepared_into, refresh_keyed_dispatch_inputs, DispatchInput,
};
use crate::optimizer::dispatcher::{DispatchError, OptimizerDispatcher};
use vyre_primitives::graph::dominator_frontier::{
    plan_dominator_frontier_launch, DominatorFrontierLaunchPlan,
};

/// Dispatcher-backed dominance-frontier query.
///
/// # Errors
///
/// Propagates dispatch failures and rejects malformed dominance or
/// predecessor CSR inputs.
#[allow(clippy::too_many_arguments)]
pub fn compute_dominance_frontier_via(
    dispatcher: &dyn OptimizerDispatcher,
    node_count: u32,
    dom_offsets: &[u32],
    dom_targets: &[u32],
    pred_offsets: &[u32],
    pred_targets: &[u32],
    seed: &[u32],
) -> Result<Vec<u32>, DispatchError> {
    let mut out = Vec::new();
    compute_dominance_frontier_via_into(
        dispatcher,
        node_count,
        dom_offsets,
        dom_targets,
        pred_offsets,
        pred_targets,
        seed,
        &mut out,
    )?;
    Ok(out)
}

/// Dispatcher-backed dominance-frontier query into caller-owned storage.
#[allow(clippy::too_many_arguments)]
pub fn compute_dominance_frontier_via_into(
    dispatcher: &dyn OptimizerDispatcher,
    node_count: u32,
    dom_offsets: &[u32],
    dom_targets: &[u32],
    pred_offsets: &[u32],
    pred_targets: &[u32],
    seed: &[u32],
    frontier_out: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    let mut scratch = DominanceFrontierGpuScratch::default();
    compute_dominance_frontier_via_with_scratch_into(
        dispatcher,
        node_count,
        dom_offsets,
        dom_targets,
        pred_offsets,
        pred_targets,
        seed,
        &mut scratch,
        frontier_out,
    )
}

/// Dispatcher-backed dominance-frontier query with caller-owned scratch.
#[allow(clippy::too_many_arguments)]
pub fn compute_dominance_frontier_via_with_scratch_into(
    dispatcher: &dyn OptimizerDispatcher,
    node_count: u32,
    dom_offsets: &[u32],
    dom_targets: &[u32],
    pred_offsets: &[u32],
    pred_targets: &[u32],
    seed: &[u32],
    scratch: &mut DominanceFrontierGpuScratch,
    frontier_out: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    let plan = plan_dominator_frontier_launch(
        node_count,
        dom_offsets,
        dom_targets,
        pred_offsets,
        pred_targets,
        seed,
    )
    .map_err(DispatchError::BadInputs)?;
    if plan.dispatch_grid()[0] == 0 {
        frontier_out.clear();
        return Ok(());
    }
    let DominanceFrontierGpuScratch {
        inputs,
        program_cache,
        static_input_key,
    } = scratch;
    let cached = program_cache.get_or_try_insert_with(plan.shape(), || {
        Ok(CachedDominanceFrontierProgram {
            program: plan
                .program("seed", "frontier_out")
                .map_err(DispatchError::BadInputs)?,
        })
    })?;

    refresh_dominance_frontier_inputs(
        inputs,
        static_input_key,
        &plan,
        dom_offsets,
        dom_targets,
        pred_offsets,
        pred_targets,
        seed,
    )?;

    dispatch_single_u32_output_from_prepared_into(
        dispatcher,
        &cached.program,
        inputs,
        plan.frontier_words(),
        "compute_dominance_frontier_via frontier_out",
        Some(plan.dispatch_grid()),
        frontier_out,
    )
}

#[allow(clippy::too_many_arguments)]
fn refresh_dominance_frontier_inputs(
    inputs: &mut Vec<Vec<u8>>,
    current_key: &mut Option<DominatorFrontierStaticInputKey>,
    plan: &DominatorFrontierLaunchPlan,
    dom_offsets: &[u32],
    dom_targets: &[u32],
    pred_offsets: &[u32],
    pred_targets: &[u32],
    seed: &[u32],
) -> Result<(), DispatchError> {
    let next_key = plan.static_input_key(dom_offsets, dom_targets, pred_offsets, pred_targets);
    refresh_keyed_dispatch_inputs(
        inputs,
        current_key,
        next_key,
        &[
            DispatchInput::U32Slice(dom_offsets),
            DispatchInput::u32_slice_or_zero_words(
                dom_targets,
                plan.dom_target_words(),
                "compute_dominance_frontier_via dom_targets",
            ),
            DispatchInput::U32Slice(pred_offsets),
            DispatchInput::u32_slice_or_zero_words(
                pred_targets,
                plan.pred_target_words(),
                "compute_dominance_frontier_via pred_targets",
            ),
            DispatchInput::U32Slice(seed),
            DispatchInput::ZeroU32Words {
                words: plan.frontier_words(),
                context: "compute_dominance_frontier_via frontier_out",
            },
        ],
        &[
            (4, DispatchInput::U32Slice(seed)),
            (
                5,
                DispatchInput::ZeroU32Words {
                    words: plan.frontier_words(),
                    context: "compute_dominance_frontier_via frontier_out",
                },
            ),
        ],
    )?;
    Ok(())
}
