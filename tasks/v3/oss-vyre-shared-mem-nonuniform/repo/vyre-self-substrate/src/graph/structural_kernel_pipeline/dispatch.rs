use vyre_foundation::ir::{Node, Program};
use vyre_primitives::{
    graph::{
        adjustment_set::backdoor_descendants_check,
        chebyshev_filter::{chebyshev_filter, try_chebyshev_filter},
        csr_backward_or_changed::csr_backward_or_changed_parallel,
        csr_backward_traverse::csr_backward_traverse,
        csr_frontier_queue::{csr_queue_forward_traverse, frontier_to_queue},
        do_calculus::{
            do_intervention_delete_incoming, do_rule2_reverse_incoming,
            try_do_intervention_delete_incoming, try_do_rule2_reverse_incoming,
        },
        dominator_frontier::{dominator_frontier, try_dominator_frontier},
        exploded::build_ifds_csr_program,
        functorial::functor_apply,
        knowledge_compile::{ddnnf_evaluate, try_ddnnf_evaluate},
        matroid::{matroid_exchange_bfs_step, try_matroid_exchange_bfs_step},
        path_reconstruct::batched_path_reconstruct,
        persistent_bfs::{persistent_bfs_batch, try_persistent_bfs_batch},
        program_graph::ProgramGraphShape,
        reachable::reachable_program,
        sheaf::{sheaf_diffusion_step, try_sheaf_diffusion_step},
        string_diagram::{monoidal_compose, try_monoidal_compose},
        sum_product_circuit::{sum_product_evaluate, try_sum_product_evaluate},
        tensor_flow_forward::tensor_flow_forward,
        union_find::{find_root_body, union_find_program, union_roots_body},
    },
    math::tensor_scc::tensor_scc_fixpoint,
};

/// Build a checked sum-product circuit evaluation dispatch.
#[allow(clippy::too_many_arguments)]
pub fn dispatch_sum_product_checked(
    kinds: &str,
    child_offsets: &str,
    child_counts: &str,
    children: &str,
    weights: &str,
    leaf_values: &str,
    out: &str,
    n_nodes: u32,
    n_edges: u32,
) -> Result<Program, String> {
    try_sum_product_evaluate(
        kinds,
        child_offsets,
        child_counts,
        children,
        weights,
        leaf_values,
        out,
        n_nodes,
        n_edges,
    )
}

/// Build a sum-product circuit evaluation dispatch.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn dispatch_sum_product(
    kinds: &str,
    child_offsets: &str,
    child_counts: &str,
    children: &str,
    weights: &str,
    leaf_values: &str,
    out: &str,
    n_nodes: u32,
    n_edges: u32,
) -> Program {
    sum_product_evaluate(
        kinds,
        child_offsets,
        child_counts,
        children,
        weights,
        leaf_values,
        out,
        n_nodes,
        n_edges,
    )
}

/// Build a checked matroid-exchange BFS layer dispatch.
pub fn dispatch_matroid_exchange_bfs_step_checked(
    frontier_in: &str,
    exchange_adj: &str,
    visited: &str,
    frontier_out: &str,
    any_change: &str,
    n: u32,
) -> Result<Program, String> {
    try_matroid_exchange_bfs_step(
        frontier_in,
        exchange_adj,
        visited,
        frontier_out,
        any_change,
        n,
    )
}

/// Build a matroid-exchange BFS layer dispatch.
#[must_use]
pub fn dispatch_matroid_exchange_bfs_step(
    frontier_in: &str,
    exchange_adj: &str,
    visited: &str,
    frontier_out: &str,
    any_change: &str,
    n: u32,
) -> Program {
    matroid_exchange_bfs_step(
        frontier_in,
        exchange_adj,
        visited,
        frontier_out,
        any_change,
        n,
    )
}

/// Build a checked monoidal string-diagram composition dispatch.
pub fn dispatch_monoidal_compose_checked(
    f: &str,
    g: &str,
    out: &str,
    a: u32,
    b: u32,
    c: u32,
) -> Result<Program, String> {
    try_monoidal_compose(f, g, out, a, b, c)
}

/// Build a monoidal string-diagram composition dispatch.
#[must_use]
pub fn dispatch_monoidal_compose(f: &str, g: &str, out: &str, a: u32, b: u32, c: u32) -> Program {
    monoidal_compose(f, g, out, a, b, c)
}

/// Build a context/field-sensitive tensor-flow propagation dispatch.
#[must_use]
pub fn dispatch_tensor_flow_forward(
    shape: ProgramGraphShape,
    tensor_in: &str,
    tensor_out: &str,
    context_limit: u32,
    field_limit: u32,
    allow_mask: u32,
) -> Program {
    tensor_flow_forward(
        shape,
        tensor_in,
        tensor_out,
        context_limit,
        field_limit,
        allow_mask,
    )
}

/// Build a functorial row-migration dispatch.
#[must_use]
pub fn dispatch_functor_apply(
    source_row: &str,
    mapping: &str,
    target_row: &str,
    n_cols: u32,
) -> Program {
    functor_apply(source_row, mapping, target_row, n_cols)
}

/// Build a checked batched persistent-BFS dispatch.
#[allow(clippy::too_many_arguments)]
pub fn dispatch_persistent_bfs_batch_checked(
    shape: ProgramGraphShape,
    frontier_in: &str,
    frontier_out: &str,
    changed: &str,
    query_count: u32,
    edge_kind_mask: u32,
    max_iters: u32,
) -> Result<Program, String> {
    try_persistent_bfs_batch(
        shape,
        frontier_in,
        frontier_out,
        changed,
        query_count,
        edge_kind_mask,
        max_iters,
    )
}

/// Build a batched persistent-BFS dispatch.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn dispatch_persistent_bfs_batch(
    shape: ProgramGraphShape,
    frontier_in: &str,
    frontier_out: &str,
    changed: &str,
    query_count: u32,
    edge_kind_mask: u32,
    max_iters: u32,
) -> Program {
    persistent_bfs_batch(
        shape,
        frontier_in,
        frontier_out,
        changed,
        query_count,
        edge_kind_mask,
        max_iters,
    )
}

/// Build a checked dominator-frontier dispatch.
pub fn dispatch_dominator_frontier_checked(
    node_count: u32,
    dom_edge_count: u32,
    pred_edge_count: u32,
    seed: &str,
    out: &str,
) -> Result<Program, String> {
    try_dominator_frontier(node_count, dom_edge_count, pred_edge_count, seed, out)
}

/// Build a dominator-frontier dispatch.
#[must_use]
pub fn dispatch_dominator_frontier(
    node_count: u32,
    dom_edge_count: u32,
    pred_edge_count: u32,
    seed: &str,
    out: &str,
) -> Program {
    dominator_frontier(node_count, dom_edge_count, pred_edge_count, seed, out)
}

/// Build a checked d-DNNF evaluation dispatch.
#[allow(clippy::too_many_arguments)]
pub fn dispatch_ddnnf_evaluate_checked(
    node_kinds: &str,
    node_var: &str,
    child_offsets: &str,
    child_counts: &str,
    children: &str,
    var_assignments: &str,
    out: &str,
    n_nodes: u32,
    n_children: u32,
    n_vars: u32,
) -> Result<Program, String> {
    try_ddnnf_evaluate(
        node_kinds,
        node_var,
        child_offsets,
        child_counts,
        children,
        var_assignments,
        out,
        n_nodes,
        n_children,
        n_vars,
    )
}

/// Build a d-DNNF evaluation dispatch.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn dispatch_ddnnf_evaluate(
    node_kinds: &str,
    node_var: &str,
    child_offsets: &str,
    child_counts: &str,
    children: &str,
    var_assignments: &str,
    out: &str,
    n_nodes: u32,
    n_children: u32,
    n_vars: u32,
) -> Program {
    ddnnf_evaluate(
        node_kinds,
        node_var,
        child_offsets,
        child_counts,
        children,
        var_assignments,
        out,
        n_nodes,
        n_children,
        n_vars,
    )
}

/// Build a frontier-to-queue compaction dispatch.
#[must_use]
pub fn dispatch_frontier_to_queue(
    frontier_in: &str,
    active_queue: &str,
    queue_len: &str,
    node_count: u32,
    queue_capacity: u32,
) -> Program {
    frontier_to_queue(
        frontier_in,
        active_queue,
        queue_len,
        node_count,
        queue_capacity,
    )
}

/// Build a queue-driven CSR traversal dispatch.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn dispatch_csr_queue_forward_traverse(
    active_queue: &str,
    queue_len: &str,
    edge_offsets: &str,
    edge_targets: &str,
    edge_kind_mask: &str,
    frontier_out: &str,
    node_count: u32,
    edge_count: u32,
    queue_capacity: u32,
    allow_mask: u32,
) -> Program {
    csr_queue_forward_traverse(
        active_queue,
        queue_len,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        frontier_out,
        node_count,
        edge_count,
        queue_capacity,
        allow_mask,
    )
}

/// Build a reverse CSR traversal dispatch.
#[must_use]
pub fn dispatch_csr_backward_traverse(
    shape: ProgramGraphShape,
    frontier_in: &str,
    frontier_out: &str,
    allow_mask: u32,
) -> Program {
    csr_backward_traverse(shape, frontier_in, frontier_out, allow_mask)
}

/// Build the deterministic exploded-IFDS CSR construction dispatch.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn dispatch_build_ifds_csr(
    num_procs: u32,
    blocks_per_proc: u32,
    facts_per_proc: u32,
    intra_count: u32,
    inter_count: u32,
    gen_count: u32,
    kill_count: u32,
    max_col_count: u32,
) -> Program {
    build_ifds_csr_program(
        num_procs,
        blocks_per_proc,
        facts_per_proc,
        intra_count,
        inter_count,
        gen_count,
        kill_count,
        max_col_count,
    )
}

/// Build a batched path-reconstruction dispatch.
#[must_use]
pub fn dispatch_batched_path_reconstruct(target_count: u32, max_depth: u32) -> Program {
    batched_path_reconstruct(target_count, max_depth)
}

/// Build a reverse in-place CSR expansion dispatch with changed flag.
#[must_use]
pub fn dispatch_csr_backward_or_changed_parallel(
    shape: ProgramGraphShape,
    frontier_out: &str,
    changed: &str,
    edge_kind_mask: u32,
) -> Program {
    csr_backward_or_changed_parallel(shape, frontier_out, changed, edge_kind_mask)
}

/// Emit the union-find root-search body.
#[must_use]
pub fn emit_find_root_body(
    parent: &str,
    id_var: &str,
    root_var: &str,
    scratch_parent_var: &str,
    node_count: u32,
) -> Vec<Node> {
    find_root_body(parent, id_var, root_var, scratch_parent_var, node_count)
}

/// Emit the union-find merge body.
#[must_use]
pub fn emit_union_roots_body(
    parent: &str,
    edge_a: &str,
    edge_b: &str,
    edge_index_var: &str,
    node_count: u32,
) -> Vec<Node> {
    union_roots_body(parent, edge_a, edge_b, edge_index_var, node_count)
}

/// Build a union-find merge dispatch.
#[must_use]
pub fn dispatch_union_find_program(
    parent: &str,
    edge_a: &str,
    edge_b: &str,
    node_count: u32,
    edge_count: u32,
) -> Program {
    union_find_program(parent, edge_a, edge_b, node_count, edge_count)
}

/// Build a checked Chebyshev graph filter dispatch.
pub fn dispatch_chebyshev_filter_checked(
    laplacian: &str,
    signal: &str,
    coeffs: &str,
    output: &str,
    scratch: &str,
    n: u32,
    k_steps: u32,
) -> Result<Program, String> {
    try_chebyshev_filter(laplacian, signal, coeffs, output, scratch, n, k_steps)
}

/// Build a Chebyshev graph filter dispatch.
#[must_use]
pub fn dispatch_chebyshev_filter(
    laplacian: &str,
    signal: &str,
    coeffs: &str,
    output: &str,
    scratch: &str,
    n: u32,
    k_steps: u32,
) -> Program {
    chebyshev_filter(laplacian, signal, coeffs, output, scratch, n, k_steps)
}

/// Build a transitive-reachability dispatch composition.
#[must_use]

pub fn dispatch_reachable_program(
    node_count: u32,
    edge_count: u32,
    sources_buf: &str,
    reach_out: &str,
    max_iters: u32,
) -> Program {
    reachable_program(node_count, edge_count, sources_buf, reach_out, max_iters)
}

/// Build a checked sheaf diffusion dispatch.
pub fn dispatch_sheaf_diffusion_step_checked(
    stalks: &str,
    restriction_diag: &str,
    damping_scaled: &str,
    stalks_next: &str,
    n_nodes: u32,
    d: u32,
) -> Result<Program, String> {
    try_sheaf_diffusion_step(
        stalks,
        restriction_diag,
        damping_scaled,
        stalks_next,
        n_nodes,
        d,
    )
}

/// Build a sheaf diffusion dispatch.
#[must_use]
pub fn dispatch_sheaf_diffusion_step(
    stalks: &str,
    restriction_diag: &str,
    damping_scaled: &str,
    stalks_next: &str,
    n_nodes: u32,
    d: u32,
) -> Program {
    sheaf_diffusion_step(
        stalks,
        restriction_diag,
        damping_scaled,
        stalks_next,
        n_nodes,
        d,
    )
}

/// Build a backdoor-descendant causal criterion dispatch.
#[must_use]
pub fn dispatch_backdoor_descendants_check(
    candidate_z: &str,
    descendants_of_x: &str,
    out_violation: &str,
    n: u32,
) -> Program {
    backdoor_descendants_check(candidate_z, descendants_of_x, out_violation, n)
}

/// Build a checked do-calculus incoming-edge deletion dispatch.
pub fn dispatch_do_intervention_delete_incoming_checked(
    adjacency: &str,
    intervention_mask: &str,
    out_adjacency: &str,
    n: u32,
) -> Result<Program, String> {
    try_do_intervention_delete_incoming(adjacency, intervention_mask, out_adjacency, n)
}

/// Build a do-calculus incoming-edge deletion dispatch.
#[must_use]
pub fn dispatch_do_intervention_delete_incoming(
    adjacency: &str,
    intervention_mask: &str,
    out_adjacency: &str,
    n: u32,
) -> Program {
    do_intervention_delete_incoming(adjacency, intervention_mask, out_adjacency, n)
}

/// Build a checked do-calculus Rule-2 incoming-edge reversal dispatch.
pub fn dispatch_do_rule2_reverse_incoming_checked(
    adjacency: &str,
    treatment_mask: &str,
    out_adjacency: &str,
    n: u32,
) -> Result<Program, String> {
    try_do_rule2_reverse_incoming(adjacency, treatment_mask, out_adjacency, n)
}

/// Build a do-calculus Rule-2 incoming-edge reversal dispatch.
#[must_use]
pub fn dispatch_do_rule2_reverse_incoming(
    adjacency: &str,
    treatment_mask: &str,
    out_adjacency: &str,
    n: u32,
) -> Program {
    do_rule2_reverse_incoming(adjacency, treatment_mask, out_adjacency, n)
}

/// Build a tensor-SCC bitset fixpoint dispatch.
#[must_use]
pub fn dispatch_tensor_scc_fixpoint(
    matrix_rows: &str,
    seed_mask: &str,
    group_mask: &str,
    out_mask: &str,
    row_count: u32,
    iteration_limit: u32,
) -> Program {
    tensor_scc_fixpoint(
        matrix_rows,
        seed_mask,
        group_mask,
        out_mask,
        row_count,
        iteration_limit,
    )
}
