use super::*;

/// Lower structural VAST rows (`kind`, `span`, `parent`, `payload`) into
/// packed Program-Graph rows:
/// `(kind, span_start, span_end, parent_idx, first_child_idx, next_sibling_idx)`.
///
/// `num_nodes` controls both dispatch bounds and buffer sizing so this stays
/// composable with one-thread-per-node invocation. Inputs outside the declared
/// `num_nodes` range are masked by the dispatch bound.
#[must_use]
pub fn c_lower_ast_to_pg_nodes(vast_nodes: &str, num_nodes: Expr, out_pg_nodes: &str) -> Program {
    let t = Expr::InvocationId { axis: 0 };

    let vast_base = Expr::mul(t.clone(), Expr::u32(VAST_NODE_STRIDE_U32));
    let pg_base = Expr::mul(t.clone(), Expr::u32(PG_NODE_STRIDE_U32));

    let loop_body = vec![
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
            Expr::add(pg_base, Expr::u32(5)),
            Expr::var("next_sibling_idx"),
        ),
    ];

    let in_words = infer_node_count_words(&num_nodes)
        .saturating_mul(VAST_NODE_STRIDE_U32)
        .max(1);
    let out_words = infer_node_count_words(&num_nodes)
        .saturating_mul(PG_NODE_STRIDE_U32)
        .max(1);

    Program::wrapped(
        vec![
            BufferDecl::storage(vast_nodes, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(in_words),
            BufferDecl::output(out_pg_nodes, 1, DataType::U32).with_count(out_words),
        ],
        [256, 1, 1],
        vec![crate::region::wrap_anonymous(
            OP_ID,
            vec![Node::if_then(
                Expr::lt(t.clone(), num_nodes.clone()),
                loop_body,
            )],
        )],
    )
    .with_entry_op_id(OP_ID)
}
