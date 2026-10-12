use super::dispatch_plan::PersistentBfsDispatchPlan;
use super::hash::persistent_bfs_layout_hash;
use super::resident_plan::{
    PersistentBfsResidentBatchDispatchPlan, PersistentBfsResidentDispatchPlan,
};
use super::validate::{
    validate_persistent_bfs_batch_frontiers, validate_persistent_bfs_frontier,
    validate_persistent_bfs_inputs,
};

/// Validate full non-resident persistent-BFS inputs and derive the dispatch plan.
///
/// # Errors
///
/// Returns an actionable diagnostic when the graph CSR, edge masks, or seed
/// frontier do not match the primitive contract.
pub fn plan_persistent_bfs_dispatch(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    frontier_in: &[u32],
    allow_mask: u32,
    max_iters: u32,
) -> Result<PersistentBfsDispatchPlan, String> {
    let layout = validate_persistent_bfs_inputs(
        node_count,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        frontier_in,
    )?;
    Ok(PersistentBfsDispatchPlan::new(
        layout,
        persistent_bfs_layout_hash(
            layout.node_count,
            edge_offsets,
            edge_targets,
            edge_kind_mask,
        ),
        allow_mask,
        max_iters,
    ))
}

/// Validate a resident graph frontier and derive the single-query dispatch plan.
///
/// # Errors
///
/// Returns an actionable diagnostic when `frontier_in` does not match the
/// resident graph's frontier width.
pub fn plan_persistent_bfs_resident_dispatch(
    node_count: u32,
    edge_count: u32,
    words_per_query: usize,
    frontier_in: &[u32],
    allow_mask: u32,
    max_iters: u32,
) -> Result<PersistentBfsResidentDispatchPlan, String> {
    Ok(PersistentBfsResidentDispatchPlan::new(
        validate_persistent_bfs_frontier(words_per_query, frontier_in)?,
        node_count,
        edge_count,
        allow_mask,
        max_iters,
    ))
}

/// Validate resident batched frontiers and derive the batch dispatch plan.
///
/// # Errors
///
/// Returns an actionable diagnostic when the flat frontier buffer does not match
/// `query_count * words_per_query` or when the batch cannot fit GPU grid
/// dimensions.
pub fn plan_persistent_bfs_resident_batch_dispatch(
    node_count: u32,
    edge_count: u32,
    words_per_query: usize,
    frontier_inputs: &[u32],
    query_count: usize,
    allow_mask: u32,
    max_iters: u32,
) -> Result<PersistentBfsResidentBatchDispatchPlan, String> {
    let batch_layout =
        validate_persistent_bfs_batch_frontiers(words_per_query, frontier_inputs, query_count)?;
    let words_per_query = u32::try_from(words_per_query).map_err(|_| {
        format!(
            "Fix: persistent_bfs_batch words_per_query {words_per_query} exceeds u32::MAX; shard the graph before GPU dispatch."
        )
    })?;
    Ok(PersistentBfsResidentBatchDispatchPlan::new(
        batch_layout,
        node_count,
        edge_count,
        words_per_query,
        allow_mask,
        max_iters,
    ))
}

/// Copy a seed frontier into caller-owned output storage.
///
/// Reservation happens before mutation, so a failed allocation does not clobber
/// the previous frontier. Dispatch wrappers use this for zero-iteration and
/// validation-only fast paths without forking seed-copy semantics.
///
/// # Errors
///
/// Returns the caller-mapped reservation error.
pub fn copy_persistent_bfs_seed_frontier_into<E, MapError>(
    frontier_out: &mut Vec<u32>,
    frontier_in: &[u32],
    context: &'static str,
    mut map_error: MapError,
) -> Result<(), E>
where
    MapError: FnMut(String) -> E,
{
    crate::graph::scratch::reserve_graph_items_with(
        frontier_out,
        frontier_in.len(),
        context,
        "persistent BFS seed frontier",
        |message| map_error(message),
    )?;
    frontier_out.clear();
    frontier_out.extend_from_slice(frontier_in);
    Ok(())
}

/// Copy flat batched seed frontiers and clear per-query changed flags.
///
/// Both output buffers reserve before mutation, preventing allocation failures
/// from destroying reusable frontier or changed-flag storage.
///
/// # Errors
///
/// Returns the caller-mapped reservation error.
pub fn copy_persistent_bfs_batch_seed_and_clear_changed_into<E, MapError>(
    frontier_outputs: &mut Vec<u32>,
    frontier_inputs: &[u32],
    changed_outputs: &mut Vec<u32>,
    query_count: usize,
    context: &'static str,
    mut map_error: MapError,
) -> Result<(), E>
where
    MapError: FnMut(String) -> E,
{
    crate::graph::scratch::reserve_graph_items_with(
        frontier_outputs,
        frontier_inputs.len(),
        context,
        "persistent BFS batch frontier",
        |message| map_error(message),
    )?;
    crate::graph::scratch::reserve_graph_items_with(
        changed_outputs,
        query_count,
        context,
        "persistent BFS batch changed flags",
        |message| map_error(message),
    )?;
    frontier_outputs.clear();
    frontier_outputs.extend_from_slice(frontier_inputs);
    changed_outputs.clear();
    changed_outputs.resize(query_count, 0);
    Ok(())
}

/// Validate a persistent-BFS changed flag read back from a backend.
///
/// # Errors
///
/// Returns an actionable diagnostic when the scalar flag is not boolean.
pub fn validate_persistent_bfs_changed_flag(changed: u32) -> Result<(), String> {
    if changed > 1 {
        return Err(format!(
            "Fix: persistent BFS changed flag readback must be 0 or 1, got {changed}. Treat this as malformed GPU readback or a backend bug."
        ));
    }
    Ok(())
}
