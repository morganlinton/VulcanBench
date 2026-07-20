use super::*;

/// Lower C VAST rows into semantic Program-Graph node and edge witnesses.
///
/// The first six semantic-node fields intentionally match
/// [`c_lower_ast_to_pg_nodes`]. Fields 6-9 add stable downstream witnesses:
/// `(category, role, attr_off, attr_len)`. The edge buffer emits five rows
/// per AST node: parent, first-child, next-sibling, and two resolved semantic
/// slots for `goto` targets plus `switch` selector/case/default relations.
/// Missing edges are explicit `C_AST_PG_EDGE_NONE` rows with sentinel
/// endpoints so downstream GPU passes can consume a fixed-stride table without
/// compaction.
pub fn c_lower_ast_to_pg_semantic_graph(
    vast_nodes: &str,
    num_nodes: Expr,
    out_pg_nodes: &str,
    out_pg_edges: &str,
) -> Program {
    c_lower_ast_to_pg_semantic_graph_impl(
        vast_nodes,
        num_nodes,
        out_pg_nodes,
        out_pg_edges,
        None,
        true,
    )
}

/// Lower C VAST rows into both the plain structural ProgramGraph rows and the
/// richer semantic ProgramGraph rows/edges in one GPU dispatch.
#[must_use]
pub fn c_lower_ast_to_pg_semantic_graph_with_pg(
    vast_nodes: &str,
    num_nodes: Expr,
    out_plain_pg_nodes: &str,
    out_pg_nodes: &str,
    out_pg_edges: &str,
) -> Program {
    c_lower_ast_to_pg_semantic_graph_impl(
        vast_nodes,
        num_nodes,
        out_pg_nodes,
        out_pg_edges,
        Some(out_plain_pg_nodes),
        true,
    )
}

/// Lower C VAST rows into plain and semantic ProgramGraph rows without the
/// expensive goto/switch/case/default target-resolution scans.
///
/// This is only correct when the token stream has already proven that no
/// control-flow target constructs requiring resolved semantic edge slots are
/// present. Parent/child/sibling structural edges and semantic node roles are
/// still emitted normally.
#[must_use]
pub fn c_lower_ast_to_pg_semantic_graph_with_pg_no_control_resolution(
    vast_nodes: &str,
    num_nodes: Expr,
    out_plain_pg_nodes: &str,
    out_pg_nodes: &str,
    out_pg_edges: &str,
) -> Program {
    c_lower_ast_to_pg_semantic_graph_impl(
        vast_nodes,
        num_nodes,
        out_pg_nodes,
        out_pg_edges,
        Some(out_plain_pg_nodes),
        false,
    )
}

pub(super) fn c_lower_ast_to_pg_semantic_graph_impl(
    vast_nodes: &str,
    num_nodes: Expr,
    out_pg_nodes: &str,
    out_pg_edges: &str,
    out_plain_pg_nodes: Option<&str>,
    resolve_control_edges: bool,
) -> Program {
    let t = Expr::InvocationId { axis: 0 };

    let vast_base = Expr::mul(t.clone(), Expr::u32(VAST_NODE_STRIDE_U32));
    let pg_base = Expr::mul(t.clone(), Expr::u32(C_AST_PG_SEMANTIC_NODE_STRIDE_U32));
    let plain_pg_base =
        out_plain_pg_nodes.map(|_| Expr::mul(t.clone(), Expr::u32(PG_NODE_STRIDE_U32)));
    let edge_base = Expr::mul(
        t.clone(),
        Expr::u32(C_AST_PG_EDGE_ROWS_PER_NODE.saturating_mul(C_AST_PG_EDGE_STRIDE_U32)),
    );

    let mut loop_body = vec![
        Node::let_bind("kind", Expr::load(vast_nodes, vast_base.clone())),
        Node::let_bind(
            "parent_idx",
            Expr::load(
                vast_nodes,
                Expr::add(vast_base.clone(), Expr::u32(IDX_PARENT as u32)),
            ),
        ),
        Node::let_bind(
            "first_child_idx",
            Expr::load(
                vast_nodes,
                Expr::add(vast_base.clone(), Expr::u32(IDX_FIRST_CHILD as u32)),
            ),
        ),
        Node::let_bind(
            "next_sibling_idx",
            Expr::load(
                vast_nodes,
                Expr::add(vast_base.clone(), Expr::u32(IDX_NEXT_SIBLING as u32)),
            ),
        ),
        Node::let_bind(
            "span_start",
            Expr::load(
                vast_nodes,
                Expr::add(vast_base.clone(), Expr::u32(IDX_SRC_BYTE_OFF as u32)),
            ),
        ),
        Node::let_bind(
            "span_len",
            Expr::load(
                vast_nodes,
                Expr::add(vast_base.clone(), Expr::u32(IDX_SRC_BYTE_LEN as u32)),
            ),
        ),
        Node::let_bind(
            "attr_off",
            Expr::load(
                vast_nodes,
                Expr::add(vast_base.clone(), Expr::u32(IDX_ATTR_OFF as u32)),
            ),
        ),
        Node::let_bind(
            "attr_len",
            Expr::load(
                vast_nodes,
                Expr::add(vast_base.clone(), Expr::u32(IDX_ATTR_LEN as u32)),
            ),
        ),
    ];
    loop_body.extend(semantic_context_bind_nodes());
    loop_body.push(Node::if_then(
        expr_is_kind(Expr::var("kind"), C_AST_KIND_POINTER_DECL),
        semantic_context_assign_nodes(vast_nodes, &num_nodes),
    ));
    loop_body.extend(semantic_classification_nodes());
    if resolve_control_edges {
        loop_body.extend(semantic_resolution_nodes(vast_nodes, &num_nodes, t.clone()));
    } else {
        loop_body.extend(unresolved_control_edge_slots());
    }
    loop_body.extend(vec![
        Node::store(out_pg_nodes, pg_base.clone(), Expr::var("kind")),
        Node::store(
            out_pg_nodes,
            Expr::add(pg_base.clone(), Expr::u32(1)),
            Expr::var("span_start"),
        ),
        Node::store(
            out_pg_nodes,
            Expr::add(pg_base.clone(), Expr::u32(2)),
            Expr::add(Expr::var("span_start"), Expr::var("span_len")),
        ),
        Node::store(
            out_pg_nodes,
            Expr::add(pg_base.clone(), Expr::u32(3)),
            Expr::var("parent_idx"),
        ),
        Node::store(
            out_pg_nodes,
            Expr::add(pg_base.clone(), Expr::u32(4)),
            Expr::var("first_child_idx"),
        ),
        Node::store(
            out_pg_nodes,
            Expr::add(pg_base.clone(), Expr::u32(5)),
            Expr::var("next_sibling_idx"),
        ),
        Node::store(
            out_pg_nodes,
            Expr::add(pg_base.clone(), Expr::u32(6)),
            Expr::var("semantic_category"),
        ),
        Node::store(
            out_pg_nodes,
            Expr::add(pg_base.clone(), Expr::u32(7)),
            Expr::var("semantic_role"),
        ),
        Node::store(
            out_pg_nodes,
            Expr::add(pg_base.clone(), Expr::u32(8)),
            Expr::var("attr_off"),
        ),
        Node::store(
            out_pg_nodes,
            Expr::add(pg_base, Expr::u32(9)),
            Expr::var("attr_len"),
        ),
    ]);
    if let (Some(out_plain_pg_nodes), Some(plain_pg_base)) = (out_plain_pg_nodes, plain_pg_base) {
        loop_body.extend(vec![
            Node::store(out_plain_pg_nodes, plain_pg_base.clone(), Expr::var("kind")),
            Node::store(
                out_plain_pg_nodes,
                Expr::add(plain_pg_base.clone(), Expr::u32(1)),
                Expr::var("span_start"),
            ),
            Node::store(
                out_plain_pg_nodes,
                Expr::add(plain_pg_base.clone(), Expr::u32(2)),
                Expr::add(Expr::var("span_start"), Expr::var("span_len")),
            ),
            Node::store(
                out_plain_pg_nodes,
                Expr::add(plain_pg_base.clone(), Expr::u32(3)),
                Expr::var("parent_idx"),
            ),
            Node::store(
                out_plain_pg_nodes,
                Expr::add(plain_pg_base.clone(), Expr::u32(4)),
                Expr::var("first_child_idx"),
            ),
            Node::store(
                out_plain_pg_nodes,
                Expr::add(plain_pg_base, Expr::u32(5)),
                Expr::var("next_sibling_idx"),
            ),
        ]);
    }

    loop_body.extend(store_semantic_edge(
        out_pg_edges,
        edge_base.clone(),
        0,
        valid_node_ref_expr(Expr::var("parent_idx"), &num_nodes),
        C_AST_PG_EDGE_PARENT,
        Expr::var("parent_idx"),
        t.clone(),
    ));
    loop_body.extend(store_semantic_edge(
        out_pg_edges,
        edge_base.clone(),
        1,
        valid_node_ref_expr(Expr::var("first_child_idx"), &num_nodes),
        C_AST_PG_EDGE_FIRST_CHILD,
        t.clone(),
        Expr::var("first_child_idx"),
    ));
    loop_body.extend(store_semantic_edge(
        out_pg_edges,
        edge_base.clone(),
        2,
        valid_node_ref_expr(Expr::var("next_sibling_idx"), &num_nodes),
        C_AST_PG_EDGE_NEXT_SIBLING,
        t.clone(),
        Expr::var("next_sibling_idx"),
    ));
    loop_body.extend(store_semantic_edge_expr(
        out_pg_edges,
        edge_base.clone(),
        3,
        Expr::var("semantic_edge3_has"),
        Expr::var("semantic_edge3_kind"),
        Expr::var("semantic_edge3_src"),
        Expr::var("semantic_edge3_dst"),
    ));
    loop_body.extend(store_semantic_edge_expr(
        out_pg_edges,
        edge_base,
        4,
        Expr::var("semantic_edge4_has"),
        Expr::var("semantic_edge4_kind"),
        Expr::var("semantic_edge4_src"),
        Expr::var("semantic_edge4_dst"),
    ));

    let in_words = infer_node_count_words(&num_nodes)
        .saturating_mul(VAST_NODE_STRIDE_U32)
        .max(1);
    let out_node_words = infer_node_count_words(&num_nodes)
        .saturating_mul(C_AST_PG_SEMANTIC_NODE_STRIDE_U32)
        .max(1);
    let out_edge_words = infer_node_count_words(&num_nodes)
        .saturating_mul(C_AST_PG_EDGE_ROWS_PER_NODE)
        .saturating_mul(C_AST_PG_EDGE_STRIDE_U32)
        .max(1);
    let out_plain_pg_words = infer_node_count_words(&num_nodes)
        .saturating_mul(PG_NODE_STRIDE_U32)
        .max(1);

    let mut buffers =
        vec![
            BufferDecl::storage(vast_nodes, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(in_words),
        ];
    let mut binding = 1;
    if let Some(out_plain_pg_nodes) = out_plain_pg_nodes {
        buffers.push(
            BufferDecl::storage(
                out_plain_pg_nodes,
                binding,
                BufferAccess::ReadWrite,
                DataType::U32,
            )
            .with_count(out_plain_pg_words)
            .with_pipeline_live_out(true),
        );
        binding += 1;
    }
    buffers
        .push(BufferDecl::output(out_pg_nodes, binding, DataType::U32).with_count(out_node_words));
    binding += 1;
    buffers.push(
        BufferDecl::storage(
            out_pg_edges,
            binding,
            BufferAccess::ReadWrite,
            DataType::U32,
        )
        .with_count(out_edge_words)
        .with_pipeline_live_out(true),
    );

    Program::wrapped(
        buffers,
        [256, 1, 1],
        vec![crate::region::wrap_anonymous(
            SEMANTIC_OP_ID,
            vec![Node::if_then(
                Expr::lt(t.clone(), num_nodes.clone()),
                vec![child_phase(
                    SEMANTIC_OP_ID,
                    "vyre-libs::parsing::c::lower::ast_to_pg_semantic_graph::node_edge_pass",
                    loop_body,
                )],
            )],
        )],
    )
    .with_entry_op_id(SEMANTIC_OP_ID)
}

pub(super) fn unresolved_control_edge_slots() -> Vec<Node> {
    vec![
        Node::let_bind("semantic_edge3_has", Expr::bool(false)),
        Node::let_bind("semantic_edge3_kind", Expr::u32(C_AST_PG_EDGE_NONE)),
        Node::let_bind("semantic_edge3_src", Expr::u32(u32::MAX)),
        Node::let_bind("semantic_edge3_dst", Expr::u32(u32::MAX)),
        Node::let_bind("semantic_edge4_has", Expr::bool(false)),
        Node::let_bind("semantic_edge4_kind", Expr::u32(C_AST_PG_EDGE_NONE)),
        Node::let_bind("semantic_edge4_src", Expr::u32(u32::MAX)),
        Node::let_bind("semantic_edge4_dst", Expr::u32(u32::MAX)),
    ]
}
