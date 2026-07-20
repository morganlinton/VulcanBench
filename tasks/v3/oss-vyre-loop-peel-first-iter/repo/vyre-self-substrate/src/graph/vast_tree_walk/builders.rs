use super::VastTreeWalkPlan;
use vyre_foundation::ir::Program;
use vyre_primitives::graph::vast_tree_walk::{
    try_ast_walk_plan, try_ast_walk_postorder, try_ast_walk_preorder, POSTORDER_OP_ID,
    PREORDER_OP_ID,
};

/// Build checked VAST traversal programs for self-hosted compiler passes.
///
/// `node_count` is the number of VAST nodes. `traversal_capacity` is the number
/// of output slots available in each traversal buffer; callers normally pass
/// `node_count`, but the value is explicit so bounded partial walks can be
/// planned without changing the primitive.
///
/// # Errors
///
/// Returns the primitive launch-shape diagnostic if the VAST node buffer or
/// traversal output capacity cannot be represented safely.
pub fn build_vast_tree_walk_plan(
    nodes: &str,
    preorder_out: &str,
    postorder_out: &str,
    node_count: u32,
    traversal_capacity: u32,
) -> Result<VastTreeWalkPlan, String> {
    use crate::observability::{bump, vast_tree_walk_calls};
    bump(&vast_tree_walk_calls);
    bump(&vast_tree_walk_calls);
    try_ast_walk_plan(
        nodes,
        preorder_out,
        postorder_out,
        node_count,
        traversal_capacity,
    )
}

/// Build the checked preorder VAST traversal used by top-down compiler passes.
///
/// # Errors
///
/// Returns when the primitive rejects the requested launch shape.
pub fn build_checked_preorder_walk(
    nodes: &str,
    out: &str,
    node_count: u32,
    traversal_capacity: u32,
) -> Result<Program, String> {
    use crate::observability::{bump, vast_tree_walk_calls};
    bump(&vast_tree_walk_calls);
    try_ast_walk_preorder(nodes, out, node_count, traversal_capacity)
}

/// Build the checked postorder VAST traversal used by bottom-up compiler
/// passes.
///
/// # Errors
///
/// Returns when the primitive rejects the requested launch shape.
pub fn build_checked_postorder_walk(
    nodes: &str,
    out: &str,
    node_count: u32,
    traversal_capacity: u32,
) -> Result<Program, String> {
    use crate::observability::{bump, vast_tree_walk_calls};
    bump(&vast_tree_walk_calls);
    try_ast_walk_postorder(nodes, out, node_count, traversal_capacity)
}

/// Build a preorder traversal for already-validated VAST layouts.
///
/// Prefer [`build_checked_preorder_walk`] at system boundaries. This helper is
/// for internal pass pipelines that have already validated the same
/// `node_count` and output capacity while building the surrounding compiler
/// workspace.
#[must_use]
pub fn build_trusted_preorder_walk(
    nodes: &str,
    out: &str,
    node_count: u32,
    traversal_capacity: u32,
) -> Program {
    use crate::observability::{bump, vast_tree_walk_calls};
    bump(&vast_tree_walk_calls);
    // Trusted = caller promised the shape was prevalidated. If that promise is
    // broken, fail fast naming the violated contract — never silently emit an
    // inert empty kernel that walks nothing. Boundary callers use
    // `build_checked_preorder_walk`.
    try_ast_walk_preorder(nodes, out, node_count, traversal_capacity).unwrap_or_else(|error| {
        panic!("trusted VAST preorder walk shape was not prevalidated: {error}")
    })
}

/// Build a postorder traversal for already-validated VAST layouts.
///
/// Prefer [`build_checked_postorder_walk`] at system boundaries. This helper is
/// for internal pass pipelines that have already validated the same
/// `node_count` and output capacity while building the surrounding compiler
/// workspace.
#[must_use]
pub fn build_trusted_postorder_walk(
    nodes: &str,
    out: &str,
    node_count: u32,
    traversal_capacity: u32,
) -> Program {
    use crate::observability::{bump, vast_tree_walk_calls};
    bump(&vast_tree_walk_calls);
    // Trusted = caller promised the shape was prevalidated; on a broken promise
    // fail fast naming the contract instead of emitting an inert empty kernel.
    try_ast_walk_postorder(nodes, out, node_count, traversal_capacity).unwrap_or_else(|error| {
        panic!("trusted VAST postorder walk shape was not prevalidated: {error}")
    })
}

/// Stable primitive op ids consumed by this self-substrate wrapper.
#[must_use]
pub const fn primitive_op_ids() -> [&'static str; 2] {
    [PREORDER_OP_ID, POSTORDER_OP_ID]
}
