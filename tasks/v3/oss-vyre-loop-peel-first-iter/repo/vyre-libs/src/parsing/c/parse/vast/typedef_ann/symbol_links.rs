use super::*;

pub fn c11_link_vast_typedef_symbols(
    vast_nodes: &str,
    num_nodes: Expr,
    out_linked_vast_nodes: &str,
) -> Program {
    const BUCKETS: u32 = 4096;

    let t = Expr::InvocationId { axis: 0 };
    let mut body = vec![Node::loop_for(
        "link_bucket_init",
        Expr::u32(0),
        Expr::u32(BUCKETS),
        vec![Node::store(
            "__vast_typedef_symbol_heads",
            Expr::var("link_bucket_init"),
            Expr::u32(SENTINEL),
        )],
    )];

    let mut row_body = vec![
        Node::let_bind(
            "link_row_base",
            Expr::mul(Expr::var("link_row"), Expr::u32(VAST_NODE_STRIDE_U32)),
        ),
        Node::let_bind(
            "link_kind",
            Expr::load(vast_nodes, Expr::var("link_row_base")),
        ),
        Node::let_bind(
            "link_hash",
            Expr::load(
                vast_nodes,
                Expr::add(
                    Expr::var("link_row_base"),
                    Expr::u32(VAST_TYPEDEF_SYMBOL_FIELD),
                ),
            ),
        ),
        Node::let_bind("link_prev_encoded", Expr::u32(0)),
        Node::if_then(
            Expr::and(
                Expr::eq(Expr::var("link_kind"), Expr::u32(TOK_IDENTIFIER)),
                Expr::ne(Expr::var("link_hash"), Expr::u32(0)),
            ),
            vec![
                Node::let_bind(
                    "link_bucket",
                    typedef_symbol_bucket(Expr::var("link_hash"), BUCKETS),
                ),
                Node::let_bind(
                    "link_prev",
                    Expr::load("__vast_typedef_symbol_heads", Expr::var("link_bucket")),
                ),
                Node::let_bind(
                    "link_next_idx",
                    Expr::select(
                        Expr::lt(
                            Expr::add(Expr::var("link_row"), Expr::u32(1)),
                            num_nodes.clone(),
                        ),
                        Expr::add(Expr::var("link_row"), Expr::u32(1)),
                        Expr::var("link_row"),
                    ),
                ),
                Node::let_bind(
                    "link_next_kind",
                    Expr::select(
                        Expr::lt(
                            Expr::add(Expr::var("link_row"), Expr::u32(1)),
                            num_nodes.clone(),
                        ),
                        Expr::load(
                            vast_nodes,
                            Expr::mul(Expr::var("link_next_idx"), Expr::u32(VAST_NODE_STRIDE_U32)),
                        ),
                        Expr::u32(SENTINEL),
                    ),
                ),
                Node::let_bind(
                    "link_possible_declarator",
                    is_typedef_symbol_link_follower_token(Expr::var("link_next_kind")),
                ),
                Node::assign(
                    "link_prev_encoded",
                    Expr::select(
                        Expr::eq(Expr::var("link_prev"), Expr::u32(SENTINEL)),
                        Expr::u32(SENTINEL),
                        Expr::add(Expr::var("link_prev"), Expr::u32(1)),
                    ),
                ),
                Node::if_then(
                    Expr::var("link_possible_declarator"),
                    vec![Node::store(
                        "__vast_typedef_symbol_heads",
                        Expr::var("link_bucket"),
                        Expr::var("link_row"),
                    )],
                ),
            ],
        ),
    ];
    for field in 0..VAST_NODE_STRIDE_U32 {
        let value = if field == VAST_TYPEDEF_FLAGS_FIELD {
            Expr::var("link_prev_encoded")
        } else {
            Expr::load(
                vast_nodes,
                Expr::add(Expr::var("link_row_base"), Expr::u32(field)),
            )
        };
        row_body.push(Node::store(
            out_linked_vast_nodes,
            Expr::add(Expr::var("link_row_base"), Expr::u32(field)),
            value,
        ));
    }
    body.push(Node::loop_for(
        "link_row",
        Expr::u32(0),
        num_nodes.clone(),
        row_body,
    ));

    let n = node_count(&num_nodes).max(1);
    Program::wrapped(
        vec![
            BufferDecl::storage(vast_nodes, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(n.saturating_mul(VAST_NODE_STRIDE_U32)),
            BufferDecl::storage(
                out_linked_vast_nodes,
                1,
                BufferAccess::ReadWrite,
                DataType::U32,
            )
            .with_count(n.saturating_mul(VAST_NODE_STRIDE_U32)),
            BufferDecl::workgroup("__vast_typedef_symbol_heads", BUCKETS, DataType::U32),
        ],
        [1, 1, 1],
        vec![wrap_anonymous(
            LINK_VAST_TYPEDEF_SYMBOLS_OP_ID,
            vec![Node::if_then(Expr::eq(t, Expr::u32(0)), body)],
        )],
    )
    .with_entry_op_id(LINK_VAST_TYPEDEF_SYMBOLS_OP_ID)
    .with_non_composable_with_self(true)
}
