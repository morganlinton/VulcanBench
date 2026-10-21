use super::dispatch_plan::CsrForwardOrChangedDispatchPlan;
use super::launch_plan::CsrForwardOrChangedLaunchPlan;
use super::layout::{
    csr_forward_or_changed_parallel_grid, CsrForwardOrChangedProgramKey,
    CSR_FORWARD_OR_CHANGED_HISTORY_FAST_PATH_MAX_ITERS,
};
use super::validate::validate_csr_inputs;

/// Validate CSR inputs and select a primitive-owned launch plan without
/// allocating the generated program.
///
/// # Errors
///
/// Returns an actionable diagnostic when CSR inputs are malformed.
pub fn plan_csr_forward_or_changed_launch(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    allow_mask: u32,
    max_iters: u32,
) -> Result<CsrForwardOrChangedLaunchPlan, String> {
    let layout = validate_csr_inputs(node_count, edge_offsets, edge_targets, edge_kind_mask)?;
    let uses_changed_history =
        max_iters > 0 && max_iters <= CSR_FORWARD_OR_CHANGED_HISTORY_FAST_PATH_MAX_ITERS;
    let changed_slots = if uses_changed_history { max_iters } else { 1 };
    Ok(CsrForwardOrChangedLaunchPlan::new(
        CsrForwardOrChangedProgramKey::new(layout, allow_mask, changed_slots, uses_changed_history),
        csr_forward_or_changed_parallel_grid(layout.node_count),
    ))
}

/// Validate CSR inputs and select the primitive-owned expansion launch plan.
///
/// # Errors
///
/// Returns an actionable diagnostic when CSR inputs are malformed or the
/// changed-history fast path cannot be represented by the primitive builders.
pub(crate) fn plan_csr_forward_or_changed_dispatch(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    allow_mask: u32,
    max_iters: u32,
) -> Result<CsrForwardOrChangedDispatchPlan, String> {
    let launch = plan_csr_forward_or_changed_launch(
        node_count,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        allow_mask,
        max_iters,
    )?;
    let program = launch.program()?;

    Ok(CsrForwardOrChangedDispatchPlan::new(launch, program))
}
