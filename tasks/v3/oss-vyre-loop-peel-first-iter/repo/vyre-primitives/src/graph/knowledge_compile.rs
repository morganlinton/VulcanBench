//! Probabilistic knowledge compilation primitive (#38).
//!
//! Knowledge compilation (Darwiche 2002) compiles a probabilistic
//! logic program into a tractable circuit (d-DNNF, SDD). The
//! compilation step is host-side; the **evaluation** of a compiled
//! circuit is GPU-shaped  -  exactly what #10 sum_product_circuit
//! does. This file ships a thin wrapper that confirms the compose
//! contract and adds a host-side d-DNNF satisfiability oracle helper.
//!
//! # Why this primitive is dual-use
//!
//! | Consumer | Use |
//! |---|---|
//! | `vyre-libs::ml::probabilistic_logic` | neuro-symbolic systems |
//! | `vyre-libs::security::policy_engine` | rule-conflict resolution as probabilistic logic |

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// d-DNNF "literal kind" tag.
pub const LITERAL_TRUE: u32 = 1;
/// d-DNNF "literal kind" tag for false.
pub const LITERAL_FALSE: u32 = 2;
/// AND node tag.
pub const AND_NODE: u32 = 3;
/// OR node tag.
pub const OR_NODE: u32 = 4;

/// Op id for the GPU-shaped d-DNNF evaluator.
pub const OP_ID: &str = "vyre-primitives::graph::ddnnf_evaluate";
/// One lane per compiled d-DNNF node in a bottom-up evaluation wave.
pub const DDNNF_EVALUATE_WORKGROUP_SIZE: [u32; 3] = [256, 1, 1];

/// Dispatch grid that covers every compiled d-DNNF node lane.
#[must_use]
pub const fn ddnnf_evaluate_dispatch_grid(n_nodes: u32) -> [u32; 3] {
    let lanes_per_block = DDNNF_EVALUATE_WORKGROUP_SIZE[0];
    let full_blocks = n_nodes / lanes_per_block;
    let tail_block = if n_nodes % lanes_per_block == 0 { 0 } else { 1 };
    let blocks = full_blocks + tail_block;
    [if blocks == 0 { 1 } else { blocks }, 1, 1]
}

/// Emit one bottom-up d-DNNF evaluation step. The dispatch is
/// `n_nodes` lanes; each lane evaluates one node from already-evaluated
/// children. Callers compose this with `level_wave_program` or another
/// topological wave scheduler when parent nodes must wait for child
/// outputs.
///
/// Buffers:
/// - `node_kinds`: u32 per node, using [`LITERAL_TRUE`],
///   [`LITERAL_FALSE`], [`AND_NODE`], [`OR_NODE`].
/// - `node_var`: u32 per node, meaningful for literal nodes.
/// - `child_offsets`: u32 per node into `children`.
/// - `child_counts`: u32 per node.
/// - `children`: concatenated child node indices.
/// - `var_assignments`: u32 per variable, 0/1/`u32::MAX` unknown.
/// - `out`: u32 per node.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn ddnnf_evaluate(
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
    match try_ddnnf_evaluate(
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
    ) {
        Ok(program) => program,
        Err(error) => crate::invalid_output_program(OP_ID, out, DataType::U32, error),
    }
}

/// Emit one bottom-up d-DNNF evaluation step with checked domain shape.
#[allow(clippy::too_many_arguments)]
pub fn try_ddnnf_evaluate(
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
    if n_nodes == 0 {
        return Err(format!(
            "Fix: ddnnf_evaluate requires n_nodes > 0, got {n_nodes}."
        ));
    }
    if n_vars == 0 {
        return Err(format!(
            "Fix: ddnnf_evaluate requires n_vars > 0, got {n_vars}."
        ));
    }

    let lane = Expr::InvocationId { axis: 0 };
    let child_index = Expr::add(Expr::var("child_base"), Expr::var("k"));
    let body = vec![Node::if_then(
        Expr::lt(lane.clone(), Expr::u32(n_nodes)),
        vec![
            Node::let_bind("kind", Expr::load(node_kinds, lane.clone())),
            Node::let_bind("var_id", Expr::load(node_var, lane.clone())),
            Node::let_bind("child_base", Expr::load(child_offsets, lane.clone())),
            Node::let_bind("child_count", Expr::load(child_counts, lane.clone())),
            Node::if_then(
                Expr::eq(Expr::var("kind"), Expr::u32(LITERAL_TRUE)),
                vec![
                    Node::let_bind(
                        "assigned_true",
                        Expr::load(var_assignments, Expr::var("var_id")),
                    ),
                    Node::store(
                        out,
                        lane.clone(),
                        Expr::select(
                            Expr::or(
                                Expr::eq(Expr::var("assigned_true"), Expr::u32(1)),
                                Expr::eq(Expr::var("assigned_true"), Expr::u32(u32::MAX)),
                            ),
                            Expr::u32(1),
                            Expr::u32(0),
                        ),
                    ),
                ],
            ),
            Node::if_then(
                Expr::eq(Expr::var("kind"), Expr::u32(LITERAL_FALSE)),
                vec![
                    Node::let_bind(
                        "assigned_false",
                        Expr::load(var_assignments, Expr::var("var_id")),
                    ),
                    Node::store(
                        out,
                        lane.clone(),
                        Expr::select(
                            Expr::or(
                                Expr::eq(Expr::var("assigned_false"), Expr::u32(0)),
                                Expr::eq(Expr::var("assigned_false"), Expr::u32(u32::MAX)),
                            ),
                            Expr::u32(1),
                            Expr::u32(0),
                        ),
                    ),
                ],
            ),
            Node::if_then(
                Expr::eq(Expr::var("kind"), Expr::u32(AND_NODE)),
                vec![
                    Node::let_bind("acc_and", Expr::u32(1)),
                    Node::loop_for(
                        "k",
                        Expr::u32(0),
                        Expr::var("child_count"),
                        vec![
                            Node::let_bind("child_node", Expr::load(children, child_index.clone())),
                            Node::assign(
                                "acc_and",
                                Expr::mul(
                                    Expr::var("acc_and"),
                                    Expr::load(out, Expr::var("child_node")),
                                ),
                            ),
                        ],
                    ),
                    Node::store(out, lane.clone(), Expr::var("acc_and")),
                ],
            ),
            Node::if_then(
                Expr::eq(Expr::var("kind"), Expr::u32(OR_NODE)),
                vec![
                    Node::let_bind("acc_or", Expr::u32(0)),
                    Node::loop_for(
                        "kk",
                        Expr::u32(0),
                        Expr::var("child_count"),
                        vec![
                            Node::let_bind(
                                "or_child_node",
                                Expr::load(
                                    children,
                                    Expr::add(Expr::var("child_base"), Expr::var("kk")),
                                ),
                            ),
                            Node::assign(
                                "acc_or",
                                Expr::add(
                                    Expr::var("acc_or"),
                                    Expr::load(out, Expr::var("or_child_node")),
                                ),
                            ),
                        ],
                    ),
                    Node::store(out, lane.clone(), Expr::var("acc_or")),
                ],
            ),
            Node::if_then(
                Expr::and(
                    Expr::and(
                        Expr::ne(Expr::var("kind"), Expr::u32(LITERAL_TRUE)),
                        Expr::ne(Expr::var("kind"), Expr::u32(LITERAL_FALSE)),
                    ),
                    Expr::and(
                        Expr::ne(Expr::var("kind"), Expr::u32(AND_NODE)),
                        Expr::ne(Expr::var("kind"), Expr::u32(OR_NODE)),
                    ),
                ),
                vec![Node::store(out, lane.clone(), Expr::u32(0))],
            ),
        ],
    )];

    Ok(Program::wrapped(
        vec![
            BufferDecl::storage(node_kinds, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(n_nodes),
            BufferDecl::storage(node_var, 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(n_nodes),
            BufferDecl::storage(child_offsets, 2, BufferAccess::ReadOnly, DataType::U32)
                .with_count(n_nodes),
            BufferDecl::storage(child_counts, 3, BufferAccess::ReadOnly, DataType::U32)
                .with_count(n_nodes),
            BufferDecl::storage(children, 4, BufferAccess::ReadOnly, DataType::U32)
                .with_count(n_children.max(1)),
            BufferDecl::storage(var_assignments, 5, BufferAccess::ReadOnly, DataType::U32)
                .with_count(n_vars),
            BufferDecl::storage(out, 6, BufferAccess::ReadWrite, DataType::U32).with_count(n_nodes),
        ],
        DDNNF_EVALUATE_WORKGROUP_SIZE,
        vec![Node::Region {
            generator: Ident::from(OP_ID),
            source_region: None,
            body: Arc::new(body),
        }],
    ))
}

/// CPU helper: evaluate a d-DNNF compiled circuit under a partial
/// variable assignment. Returns the model count weighted by node
/// types (the canonical KC inference query).
///
/// `var_assignments[var_id] = 0/1/u32::MAX` (unknown).
/// `nodes[i] = (kind, child_offset, child_count)` row-major.
/// `node_var[i]` = variable id (only meaningful for literal nodes).
/// `topo_order` is the bottom-up evaluation order.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn ddnnf_evaluate_cpu(
    nodes: &[(u32, u32, u32)],
    node_var: &[u32],
    children: &[u32],
    var_assignments: &[u32],
    topo_order: &[u32],
) -> Vec<u32> {
    match try_ddnnf_evaluate_cpu(nodes, node_var, children, var_assignments, topo_order) {
        Ok(out) => out,
        // Returning empty on failure makes a GPU-vs-CPU parity assertion pass on
        // empty==empty, silently masking a divergence (Law 10 / Law 6). Fail
        // loud; callers use try_ddnnf_evaluate_cpu.
        Err(error) => panic!("vyre-primitives d-DNNF evaluate CPU reference failed: {error}"),
    }
}

/// CPU helper with checked compiled-circuit indexing and arithmetic.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_ddnnf_evaluate_cpu(
    nodes: &[(u32, u32, u32)],
    node_var: &[u32],
    children: &[u32],
    var_assignments: &[u32],
    topo_order: &[u32],
) -> Result<Vec<u32>, String> {
    let mut out = Vec::new();
    try_ddnnf_evaluate_cpu_into(
        nodes,
        node_var,
        children,
        var_assignments,
        topo_order,
        &mut out,
    )?;
    Ok(out)
}

/// CPU helper with checked indexing/arithmetic and caller-owned output storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_ddnnf_evaluate_cpu_into(
    nodes: &[(u32, u32, u32)],
    node_var: &[u32],
    children: &[u32],
    var_assignments: &[u32],
    topo_order: &[u32],
    out: &mut Vec<u32>,
) -> Result<(), String> {
    let mut scratch = DdnnfCpuScratch::default();
    try_ddnnf_evaluate_cpu_into_with_scratch(
        nodes,
        node_var,
        children,
        var_assignments,
        topo_order,
        out,
        &mut scratch,
    )
}

/// Caller-owned workspace for d-DNNF CPU evaluation.
#[cfg(any(test, feature = "cpu-parity"))]
#[derive(Debug, Default, Clone)]
pub struct DdnnfCpuScratch {
    /// Transactional value buffer populated before committing to caller output.
    pub values: Vec<u32>,
}

#[cfg(any(test, feature = "cpu-parity"))]
impl DdnnfCpuScratch {
    /// Create an empty reusable d-DNNF evaluation workspace.
    pub fn new() -> Self {
        Self::default()
    }
}

/// CPU helper with caller-owned output and transactional scratch storage.
///
/// Malformed compiled circuits are rejected before `out` or `scratch` are
/// cleared. Arithmetic failures leave `out` unchanged because all writes happen
/// in the scratch buffer until the full evaluation succeeds.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_ddnnf_evaluate_cpu_into_with_scratch(
    nodes: &[(u32, u32, u32)],
    node_var: &[u32],
    children: &[u32],
    var_assignments: &[u32],
    topo_order: &[u32],
    out: &mut Vec<u32>,
    scratch: &mut DdnnfCpuScratch,
) -> Result<(), String> {
    validate_ddnnf_evaluate_inputs(nodes, node_var, children, var_assignments, topo_order)?;
    let n_nodes = nodes.len();
    scratch.values.clear();
    resize_ddnnf_cpu_vec(
        &mut scratch.values,
        n_nodes,
        0u32,
        "ddnnf_evaluate CPU scratch",
    )?;
    for &node in topo_order {
        let i = node as usize;
        let (kind, co, cc) = nodes[i];
        match kind {
            LITERAL_TRUE => {
                let assigned = var_assignments[node_var[i] as usize];
                scratch.values[i] = if assigned == 1 || assigned == u32::MAX {
                    1
                } else {
                    0
                };
            }
            LITERAL_FALSE => {
                let assigned = var_assignments[node_var[i] as usize];
                scratch.values[i] = if assigned == 0 || assigned == u32::MAX {
                    1
                } else {
                    0
                };
            }
            AND_NODE => {
                let mut acc = 1u32;
                for k in 0..cc as usize {
                    let child_index = co as usize + k;
                    let cn = children[child_index] as usize;
                    let child_value = scratch.values[cn];
                    acc = acc.checked_mul(child_value).ok_or_else(|| {
                        format!(
                            "ddnnf_evaluate CPU oracle AND node {i} model count overflowed u32. Fix: shard or widen model-count accumulation."
                        )
                    })?;
                }
                scratch.values[i] = acc;
            }
            OR_NODE => {
                let mut acc = 0u32;
                for k in 0..cc as usize {
                    let child_index = co as usize + k;
                    let cn = children[child_index] as usize;
                    let child_value = scratch.values[cn];
                    acc = acc.checked_add(child_value).ok_or_else(|| {
                        format!(
                            "ddnnf_evaluate CPU oracle OR node {i} model count overflowed u32. Fix: shard or widen model-count accumulation."
                        )
                    })?;
                }
                scratch.values[i] = acc;
            }
            _ => {
                scratch.values[i] = 0;
            }
        }
    }
    if n_nodes > out.len() {
        crate::graph::scratch::reserve_graph_items(
            out,
            n_nodes - out.len(),
            "d-DNNF CPU oracle",
            "ddnnf_evaluate CPU output",
        )?;
    }
    out.clear();
    out.extend_from_slice(&scratch.values);
    Ok(())
}

#[cfg(any(test, feature = "cpu-parity"))]
fn validate_ddnnf_evaluate_inputs(
    nodes: &[(u32, u32, u32)],
    node_var: &[u32],
    children: &[u32],
    var_assignments: &[u32],
    topo_order: &[u32],
) -> Result<(), String> {
    let n_nodes = nodes.len();
    if node_var.len() != n_nodes {
        return Err(format!(
            "ddnnf_evaluate CPU oracle received node_var_len={} for node_count={n_nodes}. Fix: pass one variable slot per compiled node.",
            node_var.len()
        ));
    }
    for &node in topo_order {
        let i = node as usize;
        let Some(&(kind, co, cc)) = nodes.get(i) else {
            return Err(format!(
                "ddnnf_evaluate CPU oracle topo node {node} is outside node_count={n_nodes}. Fix: rebuild the compiled-circuit topological order."
            ));
        };
        match kind {
            LITERAL_TRUE | LITERAL_FALSE => {
                let v = node_var[i] as usize;
                if v >= var_assignments.len() {
                    return Err(format!(
                        "ddnnf_evaluate CPU oracle literal node {i} references var {v} outside assignment_count={}. Fix: pass a complete assignment vector.",
                        var_assignments.len()
                    ));
                }
            }
            AND_NODE | OR_NODE => {
                let co = co as usize;
                let cc = cc as usize;
                let end = co.checked_add(cc).ok_or_else(|| {
                    format!(
                        "ddnnf_evaluate CPU oracle child offset overflow at node {i}. Fix: rebuild child_offsets before parity comparison."
                    )
                })?;
                if end > children.len() {
                    return Err(format!(
                        "ddnnf_evaluate CPU oracle node {i} child range {co}..{end} exceeds child_count={}. Fix: pass a complete child list.",
                        children.len()
                    ));
                }
                for child_index in co..end {
                    let cn = children[child_index] as usize;
                    if cn >= n_nodes {
                        return Err(format!(
                            "ddnnf_evaluate CPU oracle node {i} references child node {cn} outside node_count={n_nodes}. Fix: rebuild compiled child ids."
                        ));
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(any(test, feature = "cpu-parity"))]

fn resize_ddnnf_cpu_vec<T: Clone>(
    out: &mut Vec<T>,
    len: usize,
    value: T,
    context: &str,
) -> Result<(), String> {
    if len > out.len() {
        crate::graph::scratch::reserve_graph_items(
            out,
            len - out.len(),
            "d-DNNF CPU oracle",
            context,
        )?;
    }
    out.resize(len, value);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_single_true_literal_with_assigned_var() {
        // 1 node, kind=LITERAL_TRUE, var 0 assigned to 1 → out = 1.
        let nodes = vec![(LITERAL_TRUE, 0, 0)];
        let node_var = vec![0];
        let children = vec![];
        let assigns = vec![1];
        let order = vec![0];
        let out = ddnnf_evaluate_cpu(&nodes, &node_var, &children, &assigns, &order);
        assert_eq!(out[0], 1);
    }

    #[test]
    fn cpu_single_true_literal_with_unset_var() {
        // var 0 unknown → output 1 (counts both true assignments).
        let nodes = vec![(LITERAL_TRUE, 0, 0)];
        let node_var = vec![0];
        let children = vec![];
        let assigns = vec![u32::MAX];
        let order = vec![0];
        let out = ddnnf_evaluate_cpu(&nodes, &node_var, &children, &assigns, &order);
        assert_eq!(out[0], 1);
    }

    #[test]
    fn cpu_and_of_two_literals() {
        // (x_0=true) AND (x_1=true), both unknown → mc = 1
        let nodes = vec![(LITERAL_TRUE, 0, 0), (LITERAL_TRUE, 0, 0), (AND_NODE, 0, 2)];
        let node_var = vec![0, 1, 0];
        let children = vec![0, 1];
        let assigns = vec![u32::MAX; 2];
        let order = vec![0, 1, 2];
        let out = ddnnf_evaluate_cpu(&nodes, &node_var, &children, &assigns, &order);
        assert_eq!(out[2], 1);
    }

    #[test]
    fn cpu_or_of_two_literals_counts_both() {
        // (x_0=true) OR (x_1=true), both unknown → mc = 2
        let nodes = vec![(LITERAL_TRUE, 0, 0), (LITERAL_TRUE, 0, 0), (OR_NODE, 0, 2)];
        let node_var = vec![0, 1, 0];
        let children = vec![0, 1];
        let assigns = vec![u32::MAX; 2];
        let order = vec![0, 1, 2];
        let out = ddnnf_evaluate_cpu(&nodes, &node_var, &children, &assigns, &order);
        assert_eq!(out[2], 2);
    }

    #[test]
    fn cpu_partial_assignment_constrains_count() {
        // With var 0 fixed to true, the OR (x_0 OR x_1) becomes
        // mc = 1 (x_0 satisfied) for any x_1.
        let nodes = vec![(LITERAL_TRUE, 0, 0), (LITERAL_TRUE, 0, 0), (OR_NODE, 0, 2)];
        let node_var = vec![0, 1, 0];
        let children = vec![0, 1];
        let assigns = vec![1, 0]; // x_0 = true, x_1 = false
        let order = vec![0, 1, 2];
        let out = ddnnf_evaluate_cpu(&nodes, &node_var, &children, &assigns, &order);
        // out[0] = 1 (x_0 = true literal evaluates to 1)
        // out[1] = 0 (x_1 = true literal but x_1 is assigned false)
        // out[2] = 1 + 0 = 1
        assert_eq!(out[2], 1);
    }

    #[test]
    fn checked_cpu_oracle_rejects_missing_assignment() {
        let nodes = vec![(LITERAL_TRUE, 0, 0)];
        let node_var = vec![7];
        let children = vec![];
        let assigns = vec![u32::MAX];
        let order = vec![0];
        let error = try_ddnnf_evaluate_cpu(&nodes, &node_var, &children, &assigns, &order)
            .expect_err("checked d-DNNF oracle must reject missing variable assignments");

        assert!(
            error.contains("outside assignment_count"),
            "error should describe the missing assignment: {error}"
        );
    }

    #[test]
    fn checked_cpu_oracle_rejects_missing_child() {
        let nodes = vec![(LITERAL_TRUE, 0, 0), (AND_NODE, 0, 1)];
        let node_var = vec![0, 0];
        let children = vec![];
        let assigns = vec![u32::MAX];
        let order = vec![0, 1];
        let error = try_ddnnf_evaluate_cpu(&nodes, &node_var, &children, &assigns, &order)
            .expect_err("checked d-DNNF oracle must reject missing child list entries");

        assert!(
            error.contains("exceeds child_count"),
            "error should describe the missing child entry: {error}"
        );
    }

    #[test]
    fn scratch_cpu_oracle_rejects_bad_child_without_clobbering_storage() {
        let nodes = vec![(LITERAL_TRUE, 0, 0), (AND_NODE, 0, 1)];
        let node_var = vec![0, 0];
        let children = vec![9];
        let assigns = vec![u32::MAX];
        let order = vec![0, 1];
        let mut out = vec![0xDEAD_BEEF, 0xCAFE_BABE];
        let mut scratch = DdnnfCpuScratch {
            values: vec![7, 8, 9],
        };

        let error = try_ddnnf_evaluate_cpu_into_with_scratch(
            &nodes,
            &node_var,
            &children,
            &assigns,
            &order,
            &mut out,
            &mut scratch,
        )
        .expect_err("checked d-DNNF oracle must reject out-of-range child ids");

        assert!(
            error.contains("outside node_count"),
            "error should describe the invalid child node: {error}"
        );
        assert_eq!(
            out,
            vec![0xDEAD_BEEF, 0xCAFE_BABE],
            "Fix: malformed d-DNNF inputs must not clobber caller output."
        );
        assert_eq!(
            scratch.values,
            vec![7, 8, 9],
            "Fix: structural validation failures must not clear reusable scratch."
        );
    }

    #[test]
    fn scratch_cpu_oracle_reuses_values_and_clears_stale_tail() {
        let nodes = vec![(LITERAL_TRUE, 0, 0), (LITERAL_FALSE, 0, 0), (OR_NODE, 0, 2)];
        let node_var = vec![0, 1, 0];
        let children = vec![0, 1];
        let assigns = vec![1, 0];
        let order = vec![0, 1, 2];
        let mut out = Vec::with_capacity(8);
        out.extend_from_slice(&[99, 98, 97, 96]);
        let mut scratch = DdnnfCpuScratch {
            values: Vec::with_capacity(8),
        };
        scratch.values.extend_from_slice(&[11, 12, 13, 14, 15]);
        let out_capacity = out.capacity();
        let scratch_capacity = scratch.values.capacity();

        try_ddnnf_evaluate_cpu_into_with_scratch(
            &nodes,
            &node_var,
            &children,
            &assigns,
            &order,
            &mut out,
            &mut scratch,
        )
        .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - valid d-DNNF circuit must evaluate into reusable storage");

        assert_eq!(out, vec![1, 1, 2]);
        assert_eq!(scratch.values, vec![1, 1, 2]);
        assert_eq!(out.capacity(), out_capacity);
        assert_eq!(scratch.values.capacity(), scratch_capacity);

        try_ddnnf_evaluate_cpu_into_with_scratch(
            &nodes[..1],
            &node_var[..1],
            &[],
            &assigns,
            &[0],
            &mut out,
            &mut scratch,
        )
        .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - smaller d-DNNF circuit must reuse and truncate storage");

        assert_eq!(out, vec![1]);
        assert_eq!(scratch.values, vec![1]);
        assert_eq!(out.capacity(), out_capacity);
        assert_eq!(scratch.values.capacity(), scratch_capacity);
    }

    #[test]
    fn generated_cpu_oracle_matches_independent_ddnnf_evaluator() {
        let mut out = Vec::new();
        let mut scratch = DdnnfCpuScratch::new();
        for case in 0..4096usize {
            let n_literals = match case {
                1 => 256,
                2 => 257,
                3 => 1025,
                _ => 1 + case % 6,
            };
            let n_nodes = n_literals + 4;
            let n_vars = 1 + (case / 7) % 6;
            let mut nodes = Vec::new();
            let mut node_var = Vec::new();
            let mut children = Vec::new();

            for idx in 0..n_literals {
                let kind = if (case + idx) % 2 == 0 {
                    LITERAL_TRUE
                } else {
                    LITERAL_FALSE
                };
                nodes.push((kind, 0, 0));
                node_var.push((idx % n_vars) as u32);
            }

            for op_idx in 0..4usize {
                let child_count = 1 + ((case + op_idx) % n_literals);
                let offset = children.len() as u32;
                for child in 0..child_count {
                    children.push(((child + op_idx) % (n_literals + op_idx)) as u32);
                }
                let kind = if op_idx % 2 == 0 { AND_NODE } else { OR_NODE };
                nodes.push((kind, offset, child_count as u32));
                node_var.push(0);
            }

            let assignments: Vec<u32> = (0..n_vars)
                .map(|idx| match (case + idx) % 3 {
                    0 => 0,
                    1 => 1,
                    _ => u32::MAX,
                })
                .collect();
            let topo_order: Vec<u32> = (0..n_nodes as u32).collect();

            try_ddnnf_evaluate_cpu_into_with_scratch(
                &nodes,
                &node_var,
                &children,
                &assignments,
                &topo_order,
                &mut out,
                &mut scratch,
            )
            .expect("Fix: caller must pre-size buffers; use fallible reserve or return ResourceExhausted - generated d-DNNF CPU oracle should reserve and evaluate");
            let expected =
                independent_ddnnf_evaluate(&nodes, &node_var, &children, &assignments, &topo_order);

            assert_eq!(out, expected, "case {case}: d-DNNF evaluation mismatch");
        }
    }

    fn independent_ddnnf_evaluate(
        nodes: &[(u32, u32, u32)],
        node_var: &[u32],
        children: &[u32],
        assignments: &[u32],
        topo_order: &[u32],
    ) -> Vec<u32> {
        let mut out = Vec::new();
        out.resize(nodes.len(), 0);
        for &node in topo_order {
            let i = node as usize;
            let (kind, offset, count) = nodes[i];
            out[i] = match kind {
                LITERAL_TRUE => {
                    let assigned = assignments[node_var[i] as usize];
                    u32::from(assigned == 1 || assigned == u32::MAX)
                }
                LITERAL_FALSE => {
                    let assigned = assignments[node_var[i] as usize];
                    u32::from(assigned == 0 || assigned == u32::MAX)
                }
                AND_NODE => {
                    let mut acc = 1u32;
                    for k in 0..count as usize {
                        acc *= out[children[offset as usize + k] as usize];
                    }
                    acc
                }
                OR_NODE => {
                    let mut acc = 0u32;
                    for k in 0..count as usize {
                        acc += out[children[offset as usize + k] as usize];
                    }
                    acc
                }
                _ => 0,
            };
        }
        out
    }

    #[test]
    fn gpu_program_builder_exposes_ddnnf_buffers() {
        let program = ddnnf_evaluate(
            "kinds",
            "node_var",
            "child_offsets",
            "child_counts",
            "children",
            "assignments",
            "out",
            3,
            2,
            2,
        );
        assert_eq!(program.buffers().len(), 7);
        assert_eq!(program.workgroup_size(), DDNNF_EVALUATE_WORKGROUP_SIZE);
        assert!(
            program
                .entry()
                .iter()
                .any(|node| matches!(node, vyre_foundation::ir::Node::Region { generator, .. } if generator.as_str() == OP_ID))
        );
    }

    #[test]
    fn dispatch_grid_packs_ddnnf_nodes_into_workgroups() {
        assert_eq!(ddnnf_evaluate_dispatch_grid(0), [1, 1, 1]);
        assert_eq!(ddnnf_evaluate_dispatch_grid(1), [1, 1, 1]);
        assert_eq!(ddnnf_evaluate_dispatch_grid(256), [1, 1, 1]);
        assert_eq!(ddnnf_evaluate_dispatch_grid(257), [2, 1, 1]);
        assert_eq!(ddnnf_evaluate_dispatch_grid(1025), [5, 1, 1]);
    }

    #[test]
    fn gpu_program_builder_rejects_empty_node_count_with_trap_program() {
        let program = ddnnf_evaluate(
            "kinds",
            "node_var",
            "child_offsets",
            "child_counts",
            "children",
            "assignments",
            "out",
            0,
            0,
            1,
        );
        assert_eq!(program.buffers().len(), 1);
        assert!(
            program
                .entry()
                .iter()
                .any(|node| matches!(node, vyre_foundation::ir::Node::Region { body, .. } if body.iter().any(|inner| matches!(inner, vyre_foundation::ir::Node::Trap { .. }))))
        );
    }

    #[test]
    fn checked_gpu_builder_rejects_empty_var_domain() {
        let error = try_ddnnf_evaluate(
            "kinds",
            "node_var",
            "child_offsets",
            "child_counts",
            "children",
            "assignments",
            "out",
            1,
            0,
            0,
        )
        .expect_err("checked d-DNNF builder must reject empty variable domains");

        assert!(
            error.contains("requires n_vars > 0"),
            "error should describe the invalid variable domain: {error}"
        );
    }

    #[test]
    fn gpu_builder_source_has_checked_api_without_panics() {
        let source = include_str!("knowledge_compile.rs");
        let builder_source = source
            .split("pub fn ddnnf_evaluate(")
            .nth(1)
            .expect("Fix: d-DNNF GPU builder source must be present")
            .split("/// CPU helper:")
            .next()
            .expect("Fix: d-DNNF GPU builder source must precede CPU oracle");

        assert!(
            builder_source.contains("pub fn try_ddnnf_evaluate(")
                && !builder_source.contains(concat!("panic", "!("))
                && !builder_source.contains(".unwrap_or_else("),
            "Fix: ddnnf_evaluate must expose a checked release API and avoid production panics."
        );
    }

    #[test]
    fn ddnnf_cpu_source_uses_fallible_reusable_output() {
        let source = include_str!("knowledge_compile.rs");
        let cpu_source = source
            .split("/// CPU helper:")
            .nth(1)
            .expect("Fix: d-DNNF CPU source must be present")
            .split("#[cfg(test)]")
            .next()
            .expect("Fix: d-DNNF CPU source must precede tests");

        assert!(
            cpu_source.contains("try_ddnnf_evaluate_cpu_into")
                && cpu_source.contains("resize_ddnnf_cpu_vec")
                && cpu_source.contains("crate::graph::scratch::reserve_graph_items")
                && !cpu_source.contains("fn reserve_ddnnf_cpu_vec")
                && !cpu_source.contains("vec![0u32; n_nodes]")
                && !cpu_source.contains("Vec::with_capacity")
                && !cpu_source.contains(".reserve("),
            "Fix: d-DNNF CPU oracle must use fallible caller-owned output storage."
        );
    }
}
