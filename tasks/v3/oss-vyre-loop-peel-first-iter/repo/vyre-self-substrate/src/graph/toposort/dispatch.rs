use super::{CachedToposortProgram, ToposortGpuScratch};
use crate::graph::dispatch_bridge::{
    dispatch_single_u32_output_from_prepared_into, refresh_keyed_dispatch_inputs, DispatchInput,
};
use crate::optimizer::dispatcher::{DispatchError, OptimizerDispatcher};
use vyre_primitives::graph::toposort::{
    plan_toposort_csr_dispatch, validate_toposort_csr_order, ToposortCsrDispatchPlan,
    ToposortCsrError, ToposortCsrStaticInputKey, TOPOSORT_INDEGREE_SCRATCH_BUFFER,
    TOPOSORT_ORDER_OUT_BUFFER, TOPOSORT_QUEUE_SCRATCH_BUFFER,
};

/// Topologically sort a dependency graph through the dispatcher using the
/// primitive-native CSR representation.
///
/// `offsets` has `node_count + 1` entries and `targets` stores outgoing edges
/// from each prerequisite node to its dependent nodes. This is the adjacency
/// shape consumed by the primitive topological-sort dispatch plan.
///
/// # Errors
///
/// Returns [`DispatchError`] when CSR shape validation fails, the backend
/// rejects the primitive, or the returned order is not a full permutation of
/// `0..node_count` (cycle or malformed backend output).
pub fn topo_order_csr_via(
    dispatcher: &impl OptimizerDispatcher,
    node_count: u32,
    offsets: &[u32],
    targets: &[u32],
) -> Result<Vec<u32>, DispatchError> {
    let mut scratch = ToposortGpuScratch::default();
    let mut order = Vec::new();
    topo_order_csr_via_with_scratch_into(
        dispatcher,
        node_count,
        offsets,
        targets,
        &mut scratch,
        &mut order,
    )?;
    Ok(order)
}

/// Topologically sort a dependency graph through the dispatcher using caller-owned scratch.
pub fn topo_order_csr_via_with_scratch(
    dispatcher: &impl OptimizerDispatcher,
    node_count: u32,
    offsets: &[u32],
    targets: &[u32],
    scratch: &mut ToposortGpuScratch,
) -> Result<Vec<u32>, DispatchError> {
    let mut order = Vec::new();
    topo_order_csr_via_with_scratch_into(
        dispatcher, node_count, offsets, targets, scratch, &mut order,
    )?;
    Ok(order)
}

/// Topologically sort a dependency graph into caller-owned output storage.
pub fn topo_order_csr_via_with_scratch_into(
    dispatcher: &impl OptimizerDispatcher,
    node_count: u32,
    offsets: &[u32],
    targets: &[u32],
    scratch: &mut ToposortGpuScratch,
    order: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    use crate::observability::{bump, toposort_calls};
    bump(&toposort_calls);

    let plan =
        plan_toposort_csr_dispatch(node_count, offsets, targets).map_err(map_toposort_csr_error)?;
    if plan.layout.node_count == 0 {
        order.clear();
        return Ok(());
    }

    let ToposortGpuScratch {
        inputs,
        program_cache,
        static_input_key,
    } = scratch;
    let cached =
        program_cache.get_or_insert_with(plan.layout.node_count, || CachedToposortProgram {
            program: plan.program(),
        });
    refresh_toposort_inputs(inputs, static_input_key, &plan, offsets, targets)?;
    dispatch_single_u32_output_from_prepared_into(
        dispatcher,
        &cached.program,
        inputs,
        plan.node_words,
        TOPOSORT_ORDER_OUT_BUFFER,
        Some(plan.grid),
        order,
    )?;
    validate_toposort_csr_order(node_count, offsets, targets, order).map_err(map_toposort_csr_error)
}

fn refresh_toposort_inputs(
    inputs: &mut Vec<Vec<u8>>,
    current_key: &mut Option<ToposortCsrStaticInputKey>,
    plan: &ToposortCsrDispatchPlan,
    offsets: &[u32],
    targets: &[u32],
) -> Result<(), DispatchError> {
    let next_key = plan
        .static_input_key(offsets, targets)
        .map_err(map_toposort_csr_error)?;
    refresh_keyed_dispatch_inputs(
        inputs,
        current_key,
        next_key,
        &[
            DispatchInput::U32Slice(offsets),
            DispatchInput::U32Slice(targets),
            DispatchInput::ZeroU32Words {
                words: plan.node_words,
                context: TOPOSORT_INDEGREE_SCRATCH_BUFFER,
            },
            DispatchInput::ZeroU32Words {
                words: plan.node_words,
                context: TOPOSORT_QUEUE_SCRATCH_BUFFER,
            },
            DispatchInput::ZeroU32Words {
                words: plan.node_words,
                context: TOPOSORT_ORDER_OUT_BUFFER,
            },
        ],
        &[
            (
                2,
                DispatchInput::ZeroU32Words {
                    words: plan.node_words,
                    context: TOPOSORT_INDEGREE_SCRATCH_BUFFER,
                },
            ),
            (
                3,
                DispatchInput::ZeroU32Words {
                    words: plan.node_words,
                    context: TOPOSORT_QUEUE_SCRATCH_BUFFER,
                },
            ),
            (
                4,
                DispatchInput::ZeroU32Words {
                    words: plan.node_words,
                    context: TOPOSORT_ORDER_OUT_BUFFER,
                },
            ),
        ],
    )?;
    Ok(())
}

fn map_toposort_csr_error(error: ToposortCsrError) -> DispatchError {
    match error {
        ToposortCsrError::BadCsr { message } => DispatchError::BadInputs(message),
        ToposortCsrError::BadOrder { message } => DispatchError::BackendError(message),
        other => DispatchError::BackendError(format!(
            "Fix: topo_order_csr_via received unknown primitive CSR validation error: {other:?}."
        )),
    }
}
