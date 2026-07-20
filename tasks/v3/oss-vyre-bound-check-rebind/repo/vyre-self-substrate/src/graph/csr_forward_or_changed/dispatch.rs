use vyre_primitives::graph::csr_forward_or_changed::{
    copy_csr_forward_seed_frontier_into, plan_csr_forward_or_changed_launch,
    validate_csr_forward_or_changed_flag, CsrForwardOrChangedLaunchPlan,
    CsrForwardOrChangedProgramKey, CsrForwardOrChangedStaticInputKey,
};

use crate::graph::dispatch_bridge::{
    dispatch_two_u32_outputs_from_prepared_into, refresh_keyed_dispatch_inputs,
    write_dispatch_input, CachedProgram, DispatchInput, ProgramCache,
};
use crate::hardware::scratch::reserve_vec as reserve_graph_vec;
use crate::optimizer::dispatcher::{DispatchError, OptimizerDispatcher};

/// Caller-owned GPU dispatch scratch for `csr_forward_or_changed` fixpoint loops.
#[derive(Debug, Default)]
pub struct ForwardChangedGpuScratch {
    pub(super) inputs: Vec<Vec<u8>>,
    changed_out: Vec<u32>,
    static_input_key: Option<CsrForwardOrChangedStaticInputKey>,
    program_cache: ProgramCache<CsrForwardOrChangedProgramKey, CachedForwardChangedProgram>,
}

type CachedForwardChangedProgram = CachedProgram;

impl ForwardChangedGpuScratch {
    #[cfg(test)]
    pub(super) fn program_builds(&self) -> usize {
        self.program_cache.builds()
    }

    #[cfg(test)]
    pub(super) fn with_input_capacities(
        input_capacities: &[usize],
        changed_capacity: usize,
    ) -> Self {
        let mut inputs = Vec::new();
        inputs.reserve_exact(input_capacities.len());
        for &capacity in input_capacities {
            let mut input = Vec::new();
            input.reserve_exact(capacity);
            inputs.push(input);
        }
        let mut changed_out = Vec::new();
        changed_out.reserve_exact(changed_capacity);
        Self {
            inputs,
            changed_out,
            static_input_key: None,
            program_cache: ProgramCache::default(),
        }
    }
}

/// Dispatcher-backed closure: build the `csr_forward_or_changed` Program once,
/// then iterate dispatch + read the `changed` flag to detect fixpoint.
/// Terminates when no new bits land in the frontier or after `max_iters`.
/// Returns the saturated frontier.
///
/// Uses the supplied `OptimizerDispatcher` so callers can swap CUDA /
/// WGPU / reference backends without touching this layer.
///
/// # Errors
///
/// Propagates any [`DispatchError`] surfaced by the dispatcher.
#[allow(clippy::too_many_arguments)]
pub fn forward_closure_via_change_flag_gpu(
    dispatcher: &dyn OptimizerDispatcher,
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    seed: &[u32],
    allow_mask: u32,
    max_iters: u32,
) -> Result<Vec<u32>, DispatchError> {
    let mut frontier = Vec::new();
    forward_closure_via_change_flag_gpu_into(
        dispatcher,
        node_count,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        seed,
        allow_mask,
        max_iters,
        &mut frontier,
    )?;
    Ok(frontier)
}

/// Dispatcher-backed closure into caller-owned storage.
#[allow(clippy::too_many_arguments)]
pub fn forward_closure_via_change_flag_gpu_into(
    dispatcher: &dyn OptimizerDispatcher,
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    seed: &[u32],
    allow_mask: u32,
    max_iters: u32,
    frontier: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    let mut scratch = ForwardChangedGpuScratch::default();
    forward_closure_via_change_flag_gpu_with_scratch_into(
        dispatcher,
        node_count,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        seed,
        allow_mask,
        max_iters,
        &mut scratch,
        frontier,
    )
}

/// Dispatcher-backed closure using caller-owned dispatch scratch for the seven
/// input slots and changed flag.
#[allow(clippy::too_many_arguments)]
pub fn forward_closure_via_change_flag_gpu_with_scratch_into(
    dispatcher: &dyn OptimizerDispatcher,
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    seed: &[u32],
    allow_mask: u32,
    max_iters: u32,
    scratch: &mut ForwardChangedGpuScratch,
    frontier: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    let plan = plan_csr_forward_or_changed_launch(
        node_count,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        allow_mask,
        max_iters,
    )
    .map_err(DispatchError::BadInputs)?;
    let changed_words = plan.changed_words();
    let frontier_words = plan.frontier_words();

    copy_csr_forward_seed_frontier_into(
        seed,
        frontier_words,
        frontier,
        reserve_graph_vec,
        DispatchError::BadInputs,
    )?;

    let ForwardChangedGpuScratch {
        inputs,
        changed_out,
        static_input_key,
        program_cache,
    } = scratch;
    let cached = program_cache.get_or_try_insert_with(plan.program_key(), || {
        Ok(CachedForwardChangedProgram {
            program: plan.program().map_err(DispatchError::BadInputs)?,
        })
    })?;
    let next_static_input_key = plan
        .static_input_key(edge_offsets, edge_targets, edge_kind_mask)
        .map_err(DispatchError::BadInputs)?;

    refresh_forward_changed_inputs(
        inputs,
        static_input_key,
        next_static_input_key,
        &plan,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        frontier,
        changed_words,
    )?;

    for iter in 0..max_iters {
        use crate::observability::{bump, graph_dispatch_calls};
        bump(&graph_dispatch_calls);

        write_dispatch_input(&mut inputs[5], DispatchInput::u32_slice(frontier))?;
        if let Some(changed_slot) = plan.changed_slot_value(iter) {
            write_dispatch_input(&mut inputs[7], DispatchInput::u32_slice(&[changed_slot]))?;
        } else {
            write_dispatch_input(
                &mut inputs[6],
                DispatchInput::zero_u32_words(1, "csr_forward_or_changed changed scratch"),
            )?;
        }
        dispatch_two_u32_outputs_from_prepared_into(
            dispatcher,
            &cached.program,
            inputs,
            plan.frontier_words(),
            "csr_forward_or_changed frontier_out",
            frontier,
            changed_words,
            "csr_forward_or_changed changed",
            changed_out,
            Some(plan.dispatch_grid()),
        )?;
        let changed_index = plan
            .changed_read_index(iter)
            .map_err(DispatchError::BadInputs)?;
        let changed = changed_out[changed_index];
        validate_csr_forward_or_changed_flag(changed).map_err(DispatchError::BackendError)?;
        if changed == 0 {
            break;
        }
    }
    Ok(())
}

fn refresh_forward_changed_inputs(
    inputs: &mut Vec<Vec<u8>>,
    static_input_key: &mut Option<CsrForwardOrChangedStaticInputKey>,
    next_static_input_key: CsrForwardOrChangedStaticInputKey,
    plan: &CsrForwardOrChangedLaunchPlan,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    frontier: &[u32],
    changed_words: usize,
) -> Result<(), DispatchError> {
    if plan.uses_changed_history() {
        return refresh_keyed_dispatch_inputs(
            inputs,
            static_input_key,
            next_static_input_key,
            &[
                DispatchInput::zero_u32_words(
                    plan.node_words(),
                    "csr_forward_or_changed source scratch",
                ),
                DispatchInput::u32_slice_or_zero_words(
                    edge_offsets,
                    plan.edge_offset_words(),
                    "csr_forward_or_changed edge_offsets",
                ),
                DispatchInput::u32_slice_or_zero_words(
                    edge_targets,
                    plan.edge_storage_words(),
                    "csr_forward_or_changed edge_targets",
                ),
                DispatchInput::u32_slice_or_zero_words(
                    edge_kind_mask,
                    plan.edge_storage_words(),
                    "csr_forward_or_changed edge_kind_mask",
                ),
                DispatchInput::zero_u32_words(
                    plan.node_words(),
                    "csr_forward_or_changed frontier seed scratch",
                ),
                DispatchInput::u32_slice(frontier),
                DispatchInput::zero_u32_words(
                    changed_words,
                    "csr_forward_or_changed changed history scratch",
                ),
                DispatchInput::u32_slice(&[0]),
            ],
            &[
                (5, DispatchInput::u32_slice(frontier)),
                (
                    6,
                    DispatchInput::zero_u32_words(
                        changed_words,
                        "csr_forward_or_changed changed history scratch",
                    ),
                ),
                (7, DispatchInput::u32_slice(&[0])),
            ],
        );
    }
    refresh_keyed_dispatch_inputs(
        inputs,
        static_input_key,
        next_static_input_key,
        &[
            DispatchInput::zero_u32_words(
                plan.node_words(),
                "csr_forward_or_changed source scratch",
            ),
            DispatchInput::u32_slice_or_zero_words(
                edge_offsets,
                plan.edge_offset_words(),
                "csr_forward_or_changed edge_offsets",
            ),
            DispatchInput::u32_slice_or_zero_words(
                edge_targets,
                plan.edge_storage_words(),
                "csr_forward_or_changed edge_targets",
            ),
            DispatchInput::u32_slice_or_zero_words(
                edge_kind_mask,
                plan.edge_storage_words(),
                "csr_forward_or_changed edge_kind_mask",
            ),
            DispatchInput::zero_u32_words(
                plan.node_words(),
                "csr_forward_or_changed frontier seed scratch",
            ),
            DispatchInput::u32_slice(frontier),
            DispatchInput::zero_u32_words(1, "csr_forward_or_changed changed scratch"),
        ],
        &[
            (5, DispatchInput::u32_slice(frontier)),
            (
                6,
                DispatchInput::zero_u32_words(1, "csr_forward_or_changed changed scratch"),
            ),
        ],
    )?;
    Ok(())
}
