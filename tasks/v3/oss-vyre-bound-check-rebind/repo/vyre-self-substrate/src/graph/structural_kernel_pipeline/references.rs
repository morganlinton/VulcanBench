#[cfg(any(test, feature = "cpu-parity"))]
use vyre_primitives::{
    graph::{
        adjustment_set::backdoor_descendants_check_cpu,
        chebyshev_filter::chebyshev_filter_cpu_into,
        csr_frontier_queue::{try_csr_queue_forward_traverse_cpu, try_frontier_to_queue_cpu},
        do_calculus::{
            try_do_intervention_delete_incoming_cpu, try_do_rule2_reverse_incoming_cpu,
            try_do_rule3_subgraph_cpu,
        },
        dominator_frontier::validate_csr_shape,
        exploded::{
            decode_node, ifds_node_count_checked, max_ifds_col_count, validate_ifds_csr_layout,
            IfdsCsrLayout,
        },
        functorial::functor_apply_cpu,
        knowledge_compile::{ddnnf_evaluate_cpu, try_ddnnf_evaluate_cpu},
        matroid::matroid_exchange_bfs_step_cpu,
        path_reconstruct::cpu_ref_batched,
        string_diagram::monoidal_compose_cpu,
        sum_product_circuit::sum_product_evaluate_cpu,
        toposort::toposort_csr,
    },
    math::tensor_scc::cpu_ref as tensor_scc_cpu_ref,
};

/// Validate a CSR shape using the primitive's dominance-frontier contract.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_validate_csr_shape(
    label: &str,
    node_count: u32,
    offsets: &[u32],
    targets: &[u32],
) -> Result<u32, String> {
    validate_csr_shape(label, node_count, offsets, targets)
}

/// Compute checked exploded-IFDS node count.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_ifds_node_count_checked(
    num_procs: u32,
    blocks_per_proc: u32,
    facts_per_proc: u32,
) -> Option<u32> {
    ifds_node_count_checked(num_procs, blocks_per_proc, facts_per_proc)
}

/// Compute maximum exploded-IFDS CSR column count.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_max_ifds_col_count(
    intra_count: u32,
    inter_count: u32,
    gen_count: u32,
    facts_per_proc: u32,
) -> Option<u32> {
    max_ifds_col_count(intra_count, inter_count, gen_count, facts_per_proc)
}

/// Validate the exploded-IFDS CSR dispatch layout.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_validate_ifds_csr_layout(
    num_procs: u32,
    blocks_per_proc: u32,
    facts_per_proc: u32,
    intra_count: u32,
    inter_count: u32,
    gen_count: u32,
) -> Result<IfdsCsrLayout, String> {
    validate_ifds_csr_layout(
        num_procs,
        blocks_per_proc,
        facts_per_proc,
        intra_count,
        inter_count,
        gen_count,
    )
}

/// Decode a packed IFDS node id.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_decode_node(node_id: u32) -> (u32, u32, u32) {
    decode_node(node_id)
}

/// Compute a topological order over primitive-native CSR adjacency.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_toposort_csr(
    node_count: u32,
    offsets: &[u32],
    targets: &[u32],
) -> Result<Vec<u32>, vyre_primitives::graph::toposort::ToposortCsrError> {
    toposort_csr(node_count, offsets, targets)
}

/// CPU sum-product circuit reference.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
#[allow(clippy::too_many_arguments)]
pub fn reference_sum_product_evaluate(
    kinds: &[u32],
    child_offsets: &[u32],
    child_counts: &[u32],
    children: &[u32],
    weights: &[f64],
    leaf_values: &[f64],
    topo_order: &[u32],
) -> Vec<f64> {
    sum_product_evaluate_cpu(
        kinds,
        child_offsets,
        child_counts,
        children,
        weights,
        leaf_values,
        topo_order,
    )
}

/// CPU matroid-exchange BFS reference.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_matroid_exchange_bfs_step(
    frontier_in: &[u32],
    exchange_adj: &[u32],
    visited: &[u32],
    n: usize,
) -> (Vec<u32>, bool) {
    matroid_exchange_bfs_step_cpu(frontier_in, exchange_adj, visited, n)
}

/// CPU monoidal composition reference.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_monoidal_compose(f: &[f64], g: &[f64], a: u32, b: u32, c: u32) -> Vec<f64> {
    monoidal_compose_cpu(f, g, a, b, c)
}

/// CPU functor application reference.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_functor_apply(source_row: &[u32], mapping: &[u32], target_size: u32) -> Vec<u32> {
    functor_apply_cpu(source_row, mapping, target_size)
}

/// CPU d-DNNF reference.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_ddnnf_evaluate(
    nodes: &[(u32, u32, u32)],
    node_var: &[u32],
    children: &[u32],
    var_assignments: &[u32],
    topo_order: &[u32],
) -> Vec<u32> {
    ddnnf_evaluate_cpu(nodes, node_var, children, var_assignments, topo_order)
}

/// Checked CPU d-DNNF reference.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_try_ddnnf_evaluate_cpu(
    nodes: &[(u32, u32, u32)],
    node_var: &[u32],
    children: &[u32],
    var_assignments: &[u32],
    topo_order: &[u32],
) -> Result<Vec<u32>, String> {
    try_ddnnf_evaluate_cpu(nodes, node_var, children, var_assignments, topo_order)
}

/// CPU frontier-to-queue reference.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_frontier_to_queue(
    frontier_in: &[u32],
    node_count: u32,
    queue_capacity: usize,
) -> (Vec<u32>, u32) {
    reference_try_frontier_to_queue(frontier_in, node_count, queue_capacity).unwrap_or_else(|err| {
        panic!("structural frontier-to-queue reference rejected input. {err}")
    })
}

/// Checked CPU frontier-to-queue reference.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_try_frontier_to_queue(
    frontier_in: &[u32],
    node_count: u32,
    queue_capacity: usize,
) -> Result<(Vec<u32>, u32), String> {
    try_frontier_to_queue_cpu(frontier_in, node_count, queue_capacity)
}

/// CPU queue-driven CSR expansion reference.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
#[allow(clippy::too_many_arguments)]
pub fn reference_csr_queue_forward_traverse(
    active_queue: &[u32],
    queue_len: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    node_count: u32,
    allow_mask: u32,
) -> Vec<u32> {
    reference_try_csr_queue_forward_traverse(
        active_queue,
        queue_len,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        node_count,
        allow_mask,
    )
    .unwrap_or_else(|err| panic!("structural CSR queue traversal reference rejected input. {err}"))
}

/// Checked CPU queue-driven CSR expansion reference.
#[cfg(any(test, feature = "cpu-parity"))]
#[allow(clippy::too_many_arguments)]
pub fn reference_try_csr_queue_forward_traverse(
    active_queue: &[u32],
    queue_len: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    node_count: u32,
    allow_mask: u32,
) -> Result<Vec<u32>, String> {
    try_csr_queue_forward_traverse_cpu(
        active_queue,
        queue_len,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        node_count,
        allow_mask,
    )
}

/// CPU batched path-reconstruction reference.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_cpu_ref_batched(
    parent: &[u32],
    targets: &[u32],
    max_depth: u32,
    paths: &mut Vec<u32>,
    lens: &mut Vec<u32>,
) {
    cpu_ref_batched(parent, targets, max_depth, paths, lens);
}

/// CPU Chebyshev reference using caller-owned storage.
#[cfg(any(test, feature = "cpu-parity"))]
#[allow(clippy::too_many_arguments)]
pub fn reference_chebyshev_filter_into(
    laplacian: &[f32],
    signal: &[f32],
    coeffs: &[f32],
    n: u32,
    k_steps: u32,
    out: &mut Vec<f32>,
    t_prev: &mut Vec<f32>,
    t_curr: &mut Vec<f32>,
    t_next: &mut Vec<f32>,
) {
    chebyshev_filter_cpu_into(
        laplacian, signal, coeffs, n, k_steps, out, t_prev, t_curr, t_next,
    );
}

/// CPU backdoor-descendants criterion reference.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_backdoor_descendants_check(candidate_z: &[u32], descendants_of_x: &[u32]) -> bool {
    backdoor_descendants_check_cpu(candidate_z, descendants_of_x)
}

/// CPU do-calculus incoming-edge deletion reference.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_do_intervention_delete_incoming(
    adjacency: &[u32],
    intervention_mask: &[u32],
    n: u32,
) -> Vec<u32> {
    reference_try_do_intervention_delete_incoming(adjacency, intervention_mask, n)
        .unwrap_or_else(|err| panic!("structural intervention reference rejected input. {err}"))
}

/// Checked CPU do-calculus incoming-edge deletion reference.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_try_do_intervention_delete_incoming(
    adjacency: &[u32],
    intervention_mask: &[u32],
    n: u32,
) -> Result<Vec<u32>, String> {
    try_do_intervention_delete_incoming_cpu(adjacency, intervention_mask, n)
}

/// CPU do-calculus Rule-2 reversal reference.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_do_rule2_reverse_incoming(
    adjacency: &[u32],
    treatment_mask: &[u32],
    n: u32,
) -> Vec<u32> {
    reference_try_do_rule2_reverse_incoming(adjacency, treatment_mask, n)
        .unwrap_or_else(|err| panic!("structural rule2 reference rejected input. {err}"))
}

/// Checked CPU do-calculus Rule-2 reversal reference.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_try_do_rule2_reverse_incoming(
    adjacency: &[u32],
    treatment_mask: &[u32],
    n: u32,
) -> Result<Vec<u32>, String> {
    try_do_rule2_reverse_incoming_cpu(adjacency, treatment_mask, n)
}

/// CPU do-calculus Rule-3 subgraph reference.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_do_rule3_subgraph(
    adjacency: &[u32],
    keep_mask: &[u32],
    n: u32,
) -> (Vec<u32>, Vec<u32>) {
    reference_try_do_rule3_subgraph(adjacency, keep_mask, n)
        .unwrap_or_else(|err| panic!("structural rule3 reference rejected input. {err}"))
}

/// Checked CPU do-calculus Rule-3 subgraph reference.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_try_do_rule3_subgraph(
    adjacency: &[u32],
    keep_mask: &[u32],
    n: u32,
) -> Result<(Vec<u32>, Vec<u32>), String> {
    try_do_rule3_subgraph_cpu(adjacency, keep_mask, n)
}

/// CPU tensor-SCC reference.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_tensor_scc_fixpoint(
    matrix_rows: &[u32],
    seed_mask: u32,
    group_mask: u32,
    iteration_limit: u32,
) -> u32 {
    tensor_scc_cpu_ref(matrix_rows, seed_mask, group_mask, iteration_limit)
}
