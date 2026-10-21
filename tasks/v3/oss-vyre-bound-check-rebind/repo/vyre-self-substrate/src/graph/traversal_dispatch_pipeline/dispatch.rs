use vyre_foundation::ir::{Node, Program};
use vyre_primitives::graph::{
    adaptive_traverse::{adaptive_dense_step, should_use_dense},
    csr_forward_or_changed::{
        csr_forward_or_changed_body, csr_forward_or_changed_body_prefixed,
        csr_forward_or_changed_child, csr_forward_or_changed_child_prefixed,
        csr_forward_or_changed_parallel, csr_forward_or_changed_parallel_batch,
        csr_forward_or_changed_parallel_batch_global,
        csr_forward_or_changed_parallel_batch_global_slot,
        try_csr_forward_or_changed_parallel_batch,
        try_csr_forward_or_changed_parallel_batch_global_slot,
    },
    csr_frontier_degree_sum::csr_frontier_degree_sum,
    persistent_bfs_step::{
        persistent_bfs_step, persistent_bfs_step_body, persistent_bfs_step_body_prefixed,
        persistent_bfs_step_child, persistent_bfs_step_child_prefixed,
        persistent_bfs_step_child_prefixed_with_active,
    },
    program_graph::ProgramGraphShape,
};

/// Host-side adaptive traversal decision used before resident GPU dispatch.
#[must_use]
pub fn select_dense_traversal(frontier_in: &[u32], node_count: u32) -> bool {
    should_use_dense(frontier_in, node_count)
}

/// Build a dense reverse-adjacency traversal dispatch program.
#[must_use]
pub fn dispatch_adaptive_dense_step(
    frontier_in: &str,
    frontier_out: &str,
    adj_rows_dense: &str,
    node_count: u32,
) -> Program {
    adaptive_dense_step(frontier_in, frontier_out, adj_rows_dense, node_count)
}

/// Emit a composable CSR forward-or-changed body.
#[must_use]
pub fn csr_forward_body(
    shape: ProgramGraphShape,
    frontier_out: &str,
    changed_var: &str,
    edge_kind_mask: u32,
) -> Vec<Node> {
    csr_forward_or_changed_body(shape, frontier_out, changed_var, edge_kind_mask)
}

/// Emit a composable prefixed CSR forward-or-changed body.
#[must_use]
pub fn prefixed_csr_forward_body(
    shape: ProgramGraphShape,
    frontier_out: &str,
    changed_var: &str,
    edge_kind_mask: u32,
    prefix: &str,
) -> Vec<Node> {
    csr_forward_or_changed_body_prefixed(shape, frontier_out, changed_var, edge_kind_mask, prefix)
}

/// Emit a child CSR forward-or-changed region.
#[must_use]
pub fn child_csr_forward_stage(
    parent_op_id: &str,
    shape: ProgramGraphShape,
    frontier_out: &str,
    changed_var: &str,
    edge_kind_mask: u32,
) -> Node {
    csr_forward_or_changed_child(
        parent_op_id,
        shape,
        frontier_out,
        changed_var,
        edge_kind_mask,
    )
}

/// Emit a prefixed child CSR forward-or-changed region.
#[must_use]
pub fn prefixed_child_csr_forward_stage(
    parent_op_id: &str,
    shape: ProgramGraphShape,
    frontier_out: &str,
    changed_var: &str,
    edge_kind_mask: u32,
    prefix: &str,
) -> Node {
    csr_forward_or_changed_child_prefixed(
        parent_op_id,
        shape,
        frontier_out,
        changed_var,
        edge_kind_mask,
        prefix,
    )
}

/// Build a node-parallel CSR forward-or-changed dispatch program.
#[must_use]
pub fn dispatch_csr_forward_parallel(
    shape: ProgramGraphShape,
    frontier_out: &str,
    changed: &str,
    edge_kind_mask: u32,
) -> Program {
    csr_forward_or_changed_parallel(shape, frontier_out, changed, edge_kind_mask)
}

/// Build a checked batched CSR forward-or-changed dispatch program.
pub fn dispatch_csr_forward_batch_checked(
    shape: ProgramGraphShape,
    frontier_out: &str,
    changed: &str,
    edge_kind_mask: u32,
    query_count: u32,
) -> Result<Program, String> {
    try_csr_forward_or_changed_parallel_batch(
        shape,
        frontier_out,
        changed,
        edge_kind_mask,
        query_count,
    )
}

/// Build a batched CSR forward-or-changed dispatch program.
#[must_use]
pub fn dispatch_csr_forward_batch(
    shape: ProgramGraphShape,
    frontier_out: &str,
    changed: &str,
    edge_kind_mask: u32,
    query_count: u32,
) -> Program {
    csr_forward_or_changed_parallel_batch(shape, frontier_out, changed, edge_kind_mask, query_count)
}

/// Build a batched CSR forward-or-changed dispatch with one global change flag.
#[must_use]
pub fn dispatch_csr_forward_batch_global(
    shape: ProgramGraphShape,
    frontier_out: &str,
    changed: &str,
    edge_kind_mask: u32,
    query_count: u32,
) -> Program {
    csr_forward_or_changed_parallel_batch_global(
        shape,
        frontier_out,
        changed,
        edge_kind_mask,
        query_count,
    )
}

/// Build a checked batched CSR forward-or-changed dispatch with a global slot.
#[allow(clippy::too_many_arguments)]
pub fn dispatch_csr_forward_batch_global_slot_checked(
    shape: ProgramGraphShape,
    frontier_out: &str,
    changed: &str,
    edge_kind_mask: u32,
    query_count: u32,
    changed_slot: u32,
    changed_slots: u32,
) -> Result<Program, String> {
    try_csr_forward_or_changed_parallel_batch_global_slot(
        shape,
        frontier_out,
        changed,
        edge_kind_mask,
        query_count,
        changed_slot,
        changed_slots,
    )
}

/// Build a batched CSR forward-or-changed dispatch with a global slot.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn dispatch_csr_forward_batch_global_slot(
    shape: ProgramGraphShape,
    frontier_out: &str,
    changed: &str,
    edge_kind_mask: u32,
    query_count: u32,
    changed_slot: u32,
    changed_slots: u32,
) -> Program {
    csr_forward_or_changed_parallel_batch_global_slot(
        shape,
        frontier_out,
        changed,
        edge_kind_mask,
        query_count,
        changed_slot,
        changed_slots,
    )
}

/// Build a frontier degree-sum dispatch program for load-balanced expansion.
#[must_use]
pub fn dispatch_frontier_degree_sum(shape: ProgramGraphShape) -> Program {
    csr_frontier_degree_sum(shape)
}

/// Emit a persistent BFS step body.
#[must_use]
pub fn persistent_step_body(
    shape: ProgramGraphShape,
    frontier_out: &str,
    changed: &str,
    scratch: &str,
    edge_kind_mask: u32,
) -> Vec<Node> {
    persistent_bfs_step_body(shape, frontier_out, changed, scratch, edge_kind_mask)
}

/// Emit a prefixed persistent BFS step body.
#[must_use]
pub fn prefixed_persistent_step_body(
    shape: ProgramGraphShape,
    frontier_out: &str,
    changed: &str,
    scratch: &str,
    edge_kind_mask: u32,
    prefix: &str,
) -> Vec<Node> {
    persistent_bfs_step_body_prefixed(
        shape,
        frontier_out,
        changed,
        scratch,
        edge_kind_mask,
        prefix,
    )
}

/// Emit a child persistent BFS step region.
#[must_use]
pub fn child_persistent_step(
    parent_op_id: &str,
    shape: ProgramGraphShape,
    frontier_out: &str,
    changed: &str,
    scratch: &str,
    edge_kind_mask: u32,
) -> Node {
    persistent_bfs_step_child(
        parent_op_id,
        shape,
        frontier_out,
        changed,
        scratch,
        edge_kind_mask,
    )
}

/// Emit a prefixed child persistent BFS step region.
#[must_use]
pub fn prefixed_child_persistent_step(
    parent_op_id: &str,
    shape: ProgramGraphShape,
    frontier_out: &str,
    changed: &str,
    scratch: &str,
    edge_kind_mask: u32,
    prefix: &str,
) -> Node {
    persistent_bfs_step_child_prefixed(
        parent_op_id,
        shape,
        frontier_out,
        changed,
        scratch,
        edge_kind_mask,
        prefix,
    )
}

/// Emit an active-gated child persistent BFS step region.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn active_child_persistent_step(
    parent_op_id: &str,
    shape: ProgramGraphShape,
    frontier_out: &str,
    changed: &str,
    scratch: &str,
    active_scratch: &str,
    edge_kind_mask: u32,
    prefix: &str,
) -> Node {
    persistent_bfs_step_child_prefixed_with_active(
        parent_op_id,
        shape,
        frontier_out,
        changed,
        scratch,
        active_scratch,
        edge_kind_mask,
        prefix,
    )
}

/// Build a standalone persistent BFS step dispatch program.
#[must_use]
pub fn dispatch_persistent_bfs_step(
    shape: ProgramGraphShape,
    frontier_out: &str,
    changed: &str,
    edge_kind_mask: u32,
) -> Program {
    persistent_bfs_step(shape, frontier_out, changed, edge_kind_mask)
}
