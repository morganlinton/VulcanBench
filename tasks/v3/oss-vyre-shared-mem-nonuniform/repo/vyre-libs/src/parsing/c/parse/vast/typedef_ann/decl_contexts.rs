use super::super::decl_context_common;
use super::*;

pub fn c11_precompute_vast_decl_contexts(
    vast_nodes: &str,
    num_nodes: Expr,
    out_decl_contexts: &str,
) -> Program {
    const BUCKETS: u32 = 4096;

    let t = Expr::InvocationId { axis: 0 };
    let mut body = vec![
        Node::loop_for(
            "decl_ctx_bucket_init",
            Expr::u32(0),
            Expr::u32(BUCKETS),
            vec![
                Node::store(
                    "__vast_decl_symbol_heads",
                    Expr::var("decl_ctx_bucket_init"),
                    Expr::u32(SENTINEL),
                ),
                Node::store(
                    "__vast_decl_symbol_heads",
                    Expr::add(Expr::var("decl_ctx_bucket_init"), Expr::u32(BUCKETS)),
                    Expr::u32(0),
                ),
            ],
        ),
        Node::let_bind("decl_ctx_prefix_start", Expr::u32(0)),
    ];

    let row_context_base = decl_context_common::decl_context_base(Expr::var("decl_ctx_row"));
    let row_body = vec![
        decl_context_common::bind_vast_node_base("decl_ctx_row_base", Expr::var("decl_ctx_row")),
        decl_context_common::bind_vast_node_kind("decl_ctx_kind", vast_nodes, "decl_ctx_row_base"),
        decl_context_common::bind_vast_node_field(
            "decl_ctx_hash",
            vast_nodes,
            "decl_ctx_row_base",
            VAST_TYPEDEF_SYMBOL_FIELD,
        ),
        Node::let_bind("decl_ctx_bucket_prev_encoded", Expr::u32(SENTINEL)),
        Node::let_bind("decl_ctx_exact_prev_encoded", Expr::u32(SENTINEL)),
        Node::let_bind("decl_ctx_bucket_chain_len", Expr::u32(0)),
        Node::let_bind("decl_ctx_exact_chain_len", Expr::u32(0)),
        Node::if_then(
            Expr::and(
                Expr::eq(Expr::var("decl_ctx_kind"), Expr::u32(TOK_IDENTIFIER)),
                Expr::ne(Expr::var("decl_ctx_hash"), Expr::u32(0)),
            ),
            vec![
                Node::let_bind(
                    "decl_ctx_bucket",
                    typedef_symbol_bucket(Expr::var("decl_ctx_hash"), BUCKETS),
                ),
                Node::let_bind(
                    "decl_ctx_prev",
                    Expr::load("__vast_decl_symbol_heads", Expr::var("decl_ctx_bucket")),
                ),
                Node::assign(
                    "decl_ctx_bucket_chain_len",
                    Expr::load(
                        "__vast_decl_symbol_heads",
                        Expr::add(Expr::var("decl_ctx_bucket"), Expr::u32(BUCKETS)),
                    ),
                ),
                Node::assign(
                    "decl_ctx_bucket_prev_encoded",
                    Expr::select(
                        Expr::eq(Expr::var("decl_ctx_prev"), Expr::u32(SENTINEL)),
                        Expr::u32(SENTINEL),
                        Expr::add(Expr::var("decl_ctx_prev"), Expr::u32(1)),
                    ),
                ),
                Node::let_bind("decl_ctx_exact_cursor", Expr::var("decl_ctx_prev")),
                Node::loop_for(
                    "decl_ctx_exact_scan",
                    Expr::u32(0),
                    Expr::var("decl_ctx_bucket_chain_len"),
                    vec![Node::if_then(
                        Expr::and(
                            Expr::eq(
                                Expr::var("decl_ctx_exact_prev_encoded"),
                                Expr::u32(SENTINEL),
                            ),
                            Expr::lt(Expr::var("decl_ctx_exact_cursor"), num_nodes.clone()),
                        ),
                        vec![
                            decl_context_common::bind_vast_node_base(
                                "decl_ctx_exact_cursor_base",
                                Expr::var("decl_ctx_exact_cursor"),
                            ),
                            decl_context_common::bind_vast_node_field(
                                "decl_ctx_exact_cursor_hash",
                                vast_nodes,
                                "decl_ctx_exact_cursor_base",
                                VAST_TYPEDEF_SYMBOL_FIELD,
                            ),
                            Node::let_bind(
                                "decl_ctx_exact_cursor_context_base",
                                decl_context_common::decl_context_base(Expr::var(
                                    "decl_ctx_exact_cursor",
                                )),
                            ),
                            Node::if_then(
                                Expr::eq(
                                    Expr::var("decl_ctx_exact_cursor_hash"),
                                    Expr::var("decl_ctx_hash"),
                                ),
                                vec![
                                    Node::assign(
                                        "decl_ctx_exact_prev_encoded",
                                        Expr::add(Expr::var("decl_ctx_exact_cursor"), Expr::u32(1)),
                                    ),
                                    Node::assign(
                                        "decl_ctx_exact_chain_len",
                                        Expr::add(
                                            decl_context_common::load_decl_context_field(
                                                out_decl_contexts,
                                                Expr::var("decl_ctx_exact_cursor_context_base"),
                                                VAST_DECL_CONTEXT_PREV_DECL_CHAIN_LEN_FIELD,
                                            ),
                                            Expr::u32(1),
                                        ),
                                    ),
                                ],
                            ),
                            Node::let_bind(
                                "decl_ctx_exact_cursor_bucket_link",
                                decl_context_common::load_decl_context_field(
                                    out_decl_contexts,
                                    Expr::var("decl_ctx_exact_cursor_context_base"),
                                    VAST_DECL_CONTEXT_PREV_BUCKET_LINK_FIELD,
                                ),
                            ),
                            Node::assign(
                                "decl_ctx_exact_cursor",
                                Expr::select(
                                    Expr::or(
                                        Expr::eq(
                                            Expr::var("decl_ctx_exact_cursor_bucket_link"),
                                            Expr::u32(0),
                                        ),
                                        Expr::eq(
                                            Expr::var("decl_ctx_exact_cursor_bucket_link"),
                                            Expr::u32(SENTINEL),
                                        ),
                                    ),
                                    Expr::u32(SENTINEL),
                                    Expr::sub(
                                        Expr::var("decl_ctx_exact_cursor_bucket_link"),
                                        Expr::u32(1),
                                    ),
                                ),
                            ),
                        ],
                    )],
                ),
                Node::let_bind(
                    "decl_ctx_next_idx",
                    Expr::select(
                        Expr::lt(
                            Expr::add(Expr::var("decl_ctx_row"), Expr::u32(1)),
                            num_nodes.clone(),
                        ),
                        Expr::add(Expr::var("decl_ctx_row"), Expr::u32(1)),
                        Expr::var("decl_ctx_row"),
                    ),
                ),
                Node::let_bind(
                    "decl_ctx_next_kind",
                    Expr::select(
                        Expr::lt(
                            Expr::add(Expr::var("decl_ctx_row"), Expr::u32(1)),
                            num_nodes.clone(),
                        ),
                        decl_context_common::load_vast_node_kind(
                            vast_nodes,
                            decl_context_common::vast_node_base(Expr::var("decl_ctx_next_idx")),
                        ),
                        Expr::u32(SENTINEL),
                    ),
                ),
                Node::let_bind(
                    "decl_ctx_possible_declarator",
                    is_typedef_symbol_link_follower_token(Expr::var("decl_ctx_next_kind")),
                ),
                Node::if_then(
                    Expr::var("decl_ctx_possible_declarator"),
                    vec![
                        Node::store(
                            "__vast_decl_symbol_heads",
                            Expr::var("decl_ctx_bucket"),
                            Expr::var("decl_ctx_row"),
                        ),
                        Node::store(
                            "__vast_decl_symbol_heads",
                            Expr::add(Expr::var("decl_ctx_bucket"), Expr::u32(BUCKETS)),
                            Expr::add(Expr::var("decl_ctx_bucket_chain_len"), Expr::u32(1)),
                        ),
                    ],
                ),
            ],
        ),
        decl_context_common::store_decl_context_field(
            out_decl_contexts,
            row_context_base.clone(),
            VAST_DECL_CONTEXT_PREFIX_START_FIELD,
            Expr::var("decl_ctx_prefix_start"),
        ),
        decl_context_common::store_decl_context_field(
            out_decl_contexts,
            row_context_base.clone(),
            VAST_DECL_CONTEXT_PREV_BUCKET_LINK_FIELD,
            Expr::var("decl_ctx_bucket_prev_encoded"),
        ),
        decl_context_common::store_decl_context_field(
            out_decl_contexts,
            row_context_base.clone(),
            VAST_DECL_CONTEXT_PREV_DECL_LINK_FIELD,
            Expr::var("decl_ctx_exact_prev_encoded"),
        ),
        decl_context_common::store_decl_context_field(
            out_decl_contexts,
            row_context_base,
            VAST_DECL_CONTEXT_PREV_DECL_CHAIN_LEN_FIELD,
            Expr::var("decl_ctx_exact_chain_len"),
        ),
        Node::if_then(
            is_decl_prefix_reset_token(Expr::var("decl_ctx_kind")),
            vec![Node::assign(
                "decl_ctx_prefix_start",
                Expr::add(Expr::var("decl_ctx_row"), Expr::u32(1)),
            )],
        ),
    ];
    body.push(Node::loop_for(
        "decl_ctx_row",
        Expr::u32(0),
        num_nodes.clone(),
        row_body,
    ));

    let n = node_count(&num_nodes).max(1);
    Program::wrapped(
        vec![
            BufferDecl::storage(vast_nodes, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(n.saturating_mul(VAST_NODE_STRIDE_U32)),
            BufferDecl::storage(out_decl_contexts, 1, BufferAccess::ReadWrite, DataType::U32)
                .with_count(n.saturating_mul(VAST_DECL_CONTEXT_STRIDE_U32)),
            BufferDecl::workgroup("__vast_decl_symbol_heads", BUCKETS * 2, DataType::U32),
        ],
        [1, 1, 1],
        vec![wrap_anonymous(
            PRECOMPUTE_VAST_DECL_CONTEXTS_OP_ID,
            vec![Node::if_then(Expr::eq(t, Expr::u32(0)), body)],
        )],
    )
    .with_entry_op_id(PRECOMPUTE_VAST_DECL_CONTEXTS_OP_ID)
    .with_non_composable_with_self(true)
}
