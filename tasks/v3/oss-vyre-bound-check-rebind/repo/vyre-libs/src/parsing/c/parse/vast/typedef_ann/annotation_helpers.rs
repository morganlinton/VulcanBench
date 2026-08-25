use super::*;

#[must_use]
pub fn c11_precompute_vast_decl_prefix_starts(
    vast_nodes: &str,
    num_nodes: Expr,
    out_decl_contexts: &str,
) -> Program {
    let t = Expr::InvocationId { axis: 0 };
    let n = node_count(&num_nodes).max(1);
    const PARALLEL_BACKSCAN_MAX_NODES: u32 = 16_384;
    if n > PARALLEL_BACKSCAN_MAX_NODES {
        let row_context_base = Expr::mul(
            Expr::var("decl_prefix_row"),
            Expr::u32(VAST_DECL_CONTEXT_STRIDE_U32),
        );
        let row_body = vec![
            Node::let_bind(
                "decl_prefix_row_base",
                Expr::mul(
                    Expr::var("decl_prefix_row"),
                    Expr::u32(VAST_NODE_STRIDE_U32),
                ),
            ),
            Node::let_bind(
                "decl_prefix_kind",
                Expr::load(vast_nodes, Expr::var("decl_prefix_row_base")),
            ),
            Node::store(
                out_decl_contexts,
                Expr::add(
                    row_context_base.clone(),
                    Expr::u32(VAST_DECL_CONTEXT_PREFIX_START_FIELD),
                ),
                Expr::var("decl_prefix_start"),
            ),
            Node::store(
                out_decl_contexts,
                Expr::add(
                    row_context_base.clone(),
                    Expr::u32(VAST_DECL_CONTEXT_PREV_BUCKET_LINK_FIELD),
                ),
                Expr::u32(SENTINEL),
            ),
            Node::store(
                out_decl_contexts,
                Expr::add(
                    row_context_base.clone(),
                    Expr::u32(VAST_DECL_CONTEXT_PREV_DECL_LINK_FIELD),
                ),
                Expr::u32(SENTINEL),
            ),
            Node::store(
                out_decl_contexts,
                Expr::add(
                    row_context_base,
                    Expr::u32(VAST_DECL_CONTEXT_PREV_DECL_CHAIN_LEN_FIELD),
                ),
                Expr::u32(0),
            ),
            Node::if_then(
                is_decl_prefix_reset_token(Expr::var("decl_prefix_kind")),
                vec![Node::assign(
                    "decl_prefix_start",
                    Expr::add(Expr::var("decl_prefix_row"), Expr::u32(1)),
                )],
            ),
        ];
        let body = vec![
            Node::let_bind("decl_prefix_start", Expr::u32(0)),
            Node::loop_for("decl_prefix_row", Expr::u32(0), num_nodes.clone(), row_body),
        ];
        return Program::wrapped(
            vec![
                BufferDecl::storage(vast_nodes, 0, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(n.saturating_mul(VAST_NODE_STRIDE_U32)),
                BufferDecl::storage(out_decl_contexts, 1, BufferAccess::ReadWrite, DataType::U32)
                    .with_count(n.saturating_mul(VAST_DECL_CONTEXT_STRIDE_U32)),
            ],
            [1, 1, 1],
            vec![wrap_anonymous(
                PRECOMPUTE_VAST_DECL_PREFIX_STARTS_OP_ID,
                vec![Node::if_then(Expr::eq(t, Expr::u32(0)), body)],
            )],
        )
        .with_entry_op_id(PRECOMPUTE_VAST_DECL_PREFIX_STARTS_OP_ID)
        .with_non_composable_with_self(true);
    }

    let row_context_base = Expr::mul(t.clone(), Expr::u32(VAST_DECL_CONTEXT_STRIDE_U32));
    let body = vec![
        Node::let_bind("decl_prefix_start", Expr::u32(0)),
        Node::let_bind("decl_prefix_done", Expr::u32(0)),
        Node::loop_for(
            "decl_prefix_back_scan",
            Expr::u32(0),
            t.clone(),
            vec![Node::if_then(
                Expr::eq(Expr::var("decl_prefix_done"), Expr::u32(0)),
                vec![
                    Node::let_bind(
                        "decl_prefix_scan_idx",
                        Expr::sub(
                            Expr::sub(t.clone(), Expr::u32(1)),
                            Expr::var("decl_prefix_back_scan"),
                        ),
                    ),
                    Node::let_bind(
                        "decl_prefix_scan_base",
                        Expr::mul(
                            Expr::var("decl_prefix_scan_idx"),
                            Expr::u32(VAST_NODE_STRIDE_U32),
                        ),
                    ),
                    Node::let_bind(
                        "decl_prefix_scan_kind",
                        Expr::load(vast_nodes, Expr::var("decl_prefix_scan_base")),
                    ),
                    Node::if_then(
                        is_decl_prefix_reset_token(Expr::var("decl_prefix_scan_kind")),
                        vec![
                            Node::assign(
                                "decl_prefix_start",
                                Expr::add(Expr::var("decl_prefix_scan_idx"), Expr::u32(1)),
                            ),
                            Node::assign("decl_prefix_done", Expr::u32(1)),
                        ],
                    ),
                ],
            )],
        ),
        Node::let_bind("decl_prefix_context_base", row_context_base),
        Node::store(
            out_decl_contexts,
            Expr::add(
                Expr::var("decl_prefix_context_base"),
                Expr::u32(VAST_DECL_CONTEXT_PREFIX_START_FIELD),
            ),
            Expr::var("decl_prefix_start"),
        ),
        Node::store(
            out_decl_contexts,
            Expr::add(
                Expr::var("decl_prefix_context_base"),
                Expr::u32(VAST_DECL_CONTEXT_PREV_BUCKET_LINK_FIELD),
            ),
            Expr::u32(SENTINEL),
        ),
        Node::store(
            out_decl_contexts,
            Expr::add(
                Expr::var("decl_prefix_context_base"),
                Expr::u32(VAST_DECL_CONTEXT_PREV_DECL_LINK_FIELD),
            ),
            Expr::u32(SENTINEL),
        ),
        Node::store(
            out_decl_contexts,
            Expr::add(
                Expr::var("decl_prefix_context_base"),
                Expr::u32(VAST_DECL_CONTEXT_PREV_DECL_CHAIN_LEN_FIELD),
            ),
            Expr::u32(0),
        ),
    ];
    Program::wrapped(
        vec![
            BufferDecl::storage(vast_nodes, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(n.saturating_mul(VAST_NODE_STRIDE_U32)),
            BufferDecl::storage(out_decl_contexts, 1, BufferAccess::ReadWrite, DataType::U32)
                .with_count(n.saturating_mul(VAST_DECL_CONTEXT_STRIDE_U32)),
        ],
        [256, 1, 1],
        vec![wrap_anonymous(
            PRECOMPUTE_VAST_DECL_PREFIX_STARTS_OP_ID,
            vec![Node::if_then(Expr::lt(t, num_nodes), body)],
        )],
    )
    .with_entry_op_id(PRECOMPUTE_VAST_DECL_PREFIX_STARTS_OP_ID)
    .with_non_composable_with_self(true)
}
