use super::*;

#[must_use]
pub fn c11_annotate_global_typedef_names_fast(
    vast_nodes: &str,
    global_typedef_hashes: &str,
    num_nodes: Expr,
    num_global_typedefs: Expr,
    out_annotated_vast_nodes: &str,
) -> Program {
    let t = Expr::InvocationId { axis: 0 };
    let base = Expr::mul(t.clone(), Expr::u32(VAST_NODE_STRIDE_U32));
    let next_idx = Expr::select(
        Expr::lt(Expr::add(t.clone(), Expr::u32(1)), num_nodes.clone()),
        Expr::add(t.clone(), Expr::u32(1)),
        t.clone(),
    );
    let next_base = Expr::mul(next_idx, Expr::u32(VAST_NODE_STRIDE_U32));
    let prev_idx = Expr::select(
        Expr::gt(t.clone(), Expr::u32(0)),
        Expr::sub(t.clone(), Expr::u32(1)),
        Expr::u32(0),
    );
    let prev_base = Expr::mul(prev_idx, Expr::u32(VAST_NODE_STRIDE_U32));
    let prev_prev_idx = Expr::select(
        Expr::gt(t.clone(), Expr::u32(1)),
        Expr::sub(t.clone(), Expr::u32(2)),
        Expr::u32(0),
    );
    let prev_prev_base = Expr::mul(prev_prev_idx, Expr::u32(VAST_NODE_STRIDE_U32));
    let mut loop_body = vec![
        Node::let_bind("raw_kind", Expr::load(vast_nodes, base.clone())),
        Node::let_bind(
            "name_hash",
            Expr::load(
                vast_nodes,
                Expr::add(base.clone(), Expr::u32(VAST_TYPEDEF_SYMBOL_FIELD)),
            ),
        ),
        Node::let_bind("is_global_typedef_hash", Expr::u32(0)),
        Node::if_then(
            Expr::and(
                Expr::eq(Expr::var("raw_kind"), Expr::u32(TOK_IDENTIFIER)),
                Expr::ne(Expr::var("name_hash"), Expr::u32(0)),
            ),
            vec![Node::loop_for(
                "global_typedef_hash_scan",
                Expr::u32(0),
                num_global_typedefs.clone(),
                vec![Node::if_then(
                    Expr::eq(
                        Expr::load(global_typedef_hashes, Expr::var("global_typedef_hash_scan")),
                        Expr::var("name_hash"),
                    ),
                    vec![Node::assign("is_global_typedef_hash", Expr::u32(1))],
                )],
            )],
        ),
        Node::let_bind(
            "prev_kind",
            Expr::select(
                Expr::gt(t.clone(), Expr::u32(0)),
                Expr::load(vast_nodes, prev_base),
                Expr::u32(SENTINEL),
            ),
        ),
        Node::let_bind(
            "prev_prev_kind",
            Expr::select(
                Expr::gt(t.clone(), Expr::u32(1)),
                Expr::load(vast_nodes, prev_prev_base),
                Expr::u32(SENTINEL),
            ),
        ),
        Node::let_bind("next_kind", Expr::load(vast_nodes, next_base)),
        Node::let_bind("has_decl_prefix", Expr::u32(0)),
        Node::let_bind("has_typedef_prefix", Expr::u32(0)),
        Node::let_bind("decl_prefix_done", Expr::u32(0)),
        Node::let_bind("decl_prefix_skipped_paren_depth", Expr::u32(0)),
        Node::let_bind("decl_prefix_skipped_brace_depth", Expr::u32(0)),
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
                    Node::let_bind(
                        "decl_prefix_scan_name_hash",
                        Expr::load(
                            vast_nodes,
                            Expr::add(
                                Expr::var("decl_prefix_scan_base"),
                                Expr::u32(VAST_TYPEDEF_SYMBOL_FIELD),
                            ),
                        ),
                    ),
                    Node::let_bind("decl_prefix_scan_is_typedef_name", Expr::u32(0)),
                    Node::if_then(
                        Expr::and(
                            Expr::eq(
                                Expr::var("decl_prefix_scan_kind"),
                                Expr::u32(TOK_IDENTIFIER),
                            ),
                            Expr::ne(Expr::var("decl_prefix_scan_name_hash"), Expr::u32(0)),
                        ),
                        vec![Node::loop_for(
                            "decl_prefix_global_typedef_hash_scan",
                            Expr::u32(0),
                            num_global_typedefs.clone(),
                            vec![Node::if_then(
                                Expr::eq(
                                    Expr::load(
                                        global_typedef_hashes,
                                        Expr::var("decl_prefix_global_typedef_hash_scan"),
                                    ),
                                    Expr::var("decl_prefix_scan_name_hash"),
                                ),
                                vec![Node::assign(
                                    "decl_prefix_scan_is_typedef_name",
                                    Expr::u32(1),
                                )],
                            )],
                        )],
                    ),
                    Node::let_bind(
                        "decl_prefix_in_skipped_paren",
                        Expr::or(
                            Expr::gt(Expr::var("decl_prefix_skipped_paren_depth"), Expr::u32(0)),
                            Expr::eq(Expr::var("decl_prefix_scan_kind"), Expr::u32(TOK_RPAREN)),
                        ),
                    ),
                    Node::let_bind(
                        "decl_prefix_in_skipped_brace",
                        Expr::or(
                            Expr::gt(Expr::var("decl_prefix_skipped_brace_depth"), Expr::u32(0)),
                            Expr::eq(Expr::var("decl_prefix_scan_kind"), Expr::u32(TOK_RBRACE)),
                        ),
                    ),
                    Node::if_then(
                        Expr::eq(Expr::var("decl_prefix_scan_kind"), Expr::u32(TOK_RBRACE)),
                        vec![Node::assign(
                            "decl_prefix_skipped_brace_depth",
                            Expr::add(Expr::var("decl_prefix_skipped_brace_depth"), Expr::u32(1)),
                        )],
                    ),
                    Node::if_then(
                        Expr::and(
                            Expr::gt(Expr::var("decl_prefix_skipped_brace_depth"), Expr::u32(0)),
                            Expr::eq(Expr::var("decl_prefix_scan_kind"), Expr::u32(TOK_LBRACE)),
                        ),
                        vec![Node::assign(
                            "decl_prefix_skipped_brace_depth",
                            Expr::sub(Expr::var("decl_prefix_skipped_brace_depth"), Expr::u32(1)),
                        )],
                    ),
                    Node::if_then(
                        Expr::eq(Expr::var("decl_prefix_scan_kind"), Expr::u32(TOK_RPAREN)),
                        vec![Node::assign(
                            "decl_prefix_skipped_paren_depth",
                            Expr::add(Expr::var("decl_prefix_skipped_paren_depth"), Expr::u32(1)),
                        )],
                    ),
                    Node::if_then(
                        Expr::and(
                            Expr::gt(Expr::var("decl_prefix_skipped_paren_depth"), Expr::u32(0)),
                            Expr::eq(Expr::var("decl_prefix_scan_kind"), Expr::u32(TOK_LPAREN)),
                        ),
                        vec![Node::assign(
                            "decl_prefix_skipped_paren_depth",
                            Expr::sub(Expr::var("decl_prefix_skipped_paren_depth"), Expr::u32(1)),
                        )],
                    ),
                    Node::if_then(
                        Expr::not(Expr::or(
                            Expr::var("decl_prefix_in_skipped_brace"),
                            Expr::var("decl_prefix_in_skipped_paren"),
                        )),
                        vec![
                            Node::if_then(
                                Expr::eq(
                                    Expr::var("decl_prefix_scan_kind"),
                                    Expr::u32(TOK_TYPEDEF),
                                ),
                                vec![Node::assign("has_typedef_prefix", Expr::u32(1))],
                            ),
                            Node::if_then(
                                Expr::or(
                                    Expr::eq(
                                        Expr::var("decl_prefix_scan_is_typedef_name"),
                                        Expr::u32(1),
                                    ),
                                    any_token_eq(
                                        Expr::var("decl_prefix_scan_kind"),
                                        &[
                                            TOK_TYPEDEF,
                                            TOK_INT,
                                            TOK_CHAR_KW,
                                            TOK_VOID,
                                            TOK_DOUBLE,
                                            TOK_FLOAT_KW,
                                            TOK_LONG,
                                            TOK_SHORT,
                                            TOK_SIGNED,
                                            TOK_UNSIGNED,
                                            TOK_BOOL,
                                            TOK_STRUCT,
                                            TOK_UNION,
                                            TOK_ENUM,
                                            TOK_AUTO,
                                            TOK_CONST,
                                            TOK_VOLATILE,
                                            TOK_STATIC,
                                            TOK_EXTERN,
                                            TOK_REGISTER,
                                            TOK_RESTRICT,
                                            TOK_INLINE,
                                            TOK_ALIGNAS,
                                            TOK_ATOMIC,
                                            TOK_GNU_AUTO_TYPE,
                                            TOK_GNU_TYPEOF,
                                            TOK_GNU_TYPEOF_UNQUAL,
                                            TOK_GNU_INT128,
                                            TOK_GNU_BUILTIN_VA_LIST,
                                            TOK_FLOAT16_KW,
                                            TOK_FLOAT32_KW,
                                            TOK_FLOAT64_KW,
                                            TOK_FLOAT128_KW,
                                            TOK_GNU_FLOAT128_KW,
                                            TOK_GNU_BF16_KW,
                                            TOK_GNU_FP16_KW,
                                        ],
                                    ),
                                ),
                                vec![Node::assign("has_decl_prefix", Expr::u32(1))],
                            ),
                            Node::if_then(
                                is_decl_prefix_reset_token(Expr::var("decl_prefix_scan_kind")),
                                vec![Node::assign("decl_prefix_done", Expr::u32(1))],
                            ),
                        ],
                    ),
                ],
            )],
        ),
        Node::let_bind(
            "scope_open",
            Expr::load(
                vast_nodes,
                Expr::add(base.clone(), Expr::u32(VAST_TYPEDEF_SCOPE_FIELD)),
            ),
        ),
        Node::let_bind(
            "scope_has_prev",
            Expr::and(
                Expr::ne(Expr::var("scope_open"), Expr::u32(SENTINEL)),
                Expr::gt(Expr::var("scope_open"), Expr::u32(0)),
            ),
        ),
        Node::let_bind(
            "scope_prev_kind",
            Expr::select(
                Expr::var("scope_has_prev"),
                Expr::load(
                    vast_nodes,
                    Expr::mul(
                        Expr::sub(Expr::var("scope_open"), Expr::u32(1)),
                        Expr::u32(VAST_NODE_STRIDE_U32),
                    ),
                ),
                Expr::u32(SENTINEL),
            ),
        ),
        Node::let_bind(
            "scope_has_prev_prev",
            Expr::and(
                Expr::ne(Expr::var("scope_open"), Expr::u32(SENTINEL)),
                Expr::gt(Expr::var("scope_open"), Expr::u32(1)),
            ),
        ),
        Node::let_bind(
            "scope_prev_prev_kind",
            Expr::select(
                Expr::var("scope_has_prev_prev"),
                Expr::load(
                    vast_nodes,
                    Expr::mul(
                        Expr::sub(Expr::var("scope_open"), Expr::u32(2)),
                        Expr::u32(VAST_NODE_STRIDE_U32),
                    ),
                ),
                Expr::u32(SENTINEL),
            ),
        ),
        Node::let_bind("in_aggregate_body", Expr::bool(false)),
        Node::let_bind("aggregate_scan_done", Expr::bool(false)),
        Node::loop_for(
            "aggregate_scope_back_scan",
            Expr::u32(0),
            Expr::select(
                Expr::ne(Expr::var("scope_open"), Expr::u32(SENTINEL)),
                Expr::var("scope_open"),
                Expr::u32(0),
            ),
            vec![Node::if_then(
                Expr::not(Expr::var("aggregate_scan_done")),
                vec![
                    Node::let_bind(
                        "aggregate_scan_idx",
                        Expr::sub(
                            Expr::sub(Expr::var("scope_open"), Expr::u32(1)),
                            Expr::var("aggregate_scope_back_scan"),
                        ),
                    ),
                    Node::let_bind(
                        "aggregate_scan_kind",
                        Expr::load(
                            vast_nodes,
                            Expr::mul(
                                Expr::var("aggregate_scan_idx"),
                                Expr::u32(VAST_NODE_STRIDE_U32),
                            ),
                        ),
                    ),
                    Node::if_then(
                        any_token_eq(
                            Expr::var("aggregate_scan_kind"),
                            &[TOK_STRUCT, TOK_UNION, TOK_ENUM],
                        ),
                        vec![
                            Node::assign("in_aggregate_body", Expr::bool(true)),
                            Node::assign("aggregate_scan_done", Expr::bool(true)),
                        ],
                    ),
                    Node::if_then(
                        is_decl_prefix_reset_token(Expr::var("aggregate_scan_kind")),
                        vec![Node::assign("aggregate_scan_done", Expr::bool(true))],
                    ),
                ],
            )],
        ),
        Node::let_bind(
            "possible_declarator",
            any_token_eq(
                Expr::var("next_kind"),
                &[
                    TOK_SEMICOLON,
                    TOK_COMMA,
                    TOK_ASSIGN,
                    TOK_LPAREN,
                    TOK_LBRACKET,
                    TOK_COLON,
                    TOK_RPAREN,
                    TOK_RBRACKET,
                ],
            ),
        ),
        Node::let_bind(
            "declaration_candidate",
            Expr::and(
                Expr::var("possible_declarator"),
                Expr::and(
                    Expr::not(any_token_eq(
                        Expr::var("prev_kind"),
                        &[
                            TOK_STRUCT, TOK_UNION, TOK_ENUM, TOK_DOT, TOK_ARROW, TOK_GOTO,
                        ],
                    )),
                    Expr::and(
                        Expr::ne(Expr::var("next_kind"), Expr::u32(TOK_COLON)),
                        Expr::and(
                            Expr::not(Expr::and(
                                Expr::eq(Expr::var("prev_kind"), Expr::u32(TOK_STAR)),
                                Expr::eq(Expr::var("prev_prev_kind"), Expr::u32(TOK_RPAREN)),
                            )),
                            Expr::and(
                                Expr::eq(Expr::var("has_decl_prefix"), Expr::u32(1)),
                                Expr::not(Expr::var("in_aggregate_body")),
                            ),
                        ),
                    ),
                ),
            ),
        ),
        Node::let_bind(
            "typedef_name_context",
            Expr::and(
                Expr::not(any_token_eq(
                    Expr::var("prev_kind"),
                    &[
                        TOK_STRUCT, TOK_UNION, TOK_ENUM, TOK_DOT, TOK_ARROW, TOK_GOTO,
                    ],
                )),
                Expr::ne(Expr::var("next_kind"), Expr::u32(TOK_COLON)),
            ),
        ),
        Node::let_bind("has_prior_same_hash", Expr::u32(0)),
        Node::let_bind("prior_same_hash_done", Expr::u32(0)),
        Node::loop_for(
            "prior_typedef_hash_scan",
            Expr::u32(0),
            t.clone(),
            vec![Node::if_then(
                Expr::eq(Expr::var("prior_same_hash_done"), Expr::u32(0)),
                vec![
                    Node::let_bind(
                        "prior_typedef_hash_idx",
                        Expr::sub(
                            Expr::sub(t.clone(), Expr::u32(1)),
                            Expr::var("prior_typedef_hash_scan"),
                        ),
                    ),
                    Node::let_bind(
                        "prior_typedef_hash_base",
                        Expr::mul(
                            Expr::var("prior_typedef_hash_idx"),
                            Expr::u32(VAST_NODE_STRIDE_U32),
                        ),
                    ),
                    Node::let_bind(
                        "prior_typedef_hash_kind",
                        Expr::load(vast_nodes, Expr::var("prior_typedef_hash_base")),
                    ),
                    Node::if_then(
                        Expr::and(
                            Expr::eq(
                                Expr::var("prior_typedef_hash_kind"),
                                Expr::u32(TOK_IDENTIFIER),
                            ),
                            Expr::eq(
                                Expr::load(
                                    vast_nodes,
                                    Expr::add(
                                        Expr::var("prior_typedef_hash_base"),
                                        Expr::u32(VAST_TYPEDEF_SYMBOL_FIELD),
                                    ),
                                ),
                                Expr::var("name_hash"),
                            ),
                        ),
                        vec![
                            Node::assign("prior_same_hash_done", Expr::u32(1)),
                            Node::let_bind("prior_same_hash_has_typedef", Expr::u32(0)),
                            Node::let_bind("prior_same_hash_prefix_done", Expr::u32(0)),
                            Node::let_bind("prior_same_hash_skipped_paren_depth", Expr::u32(0)),
                            Node::let_bind("prior_same_hash_skipped_brace_depth", Expr::u32(0)),
                            Node::loop_for(
                                "prior_same_hash_prefix_scan",
                                Expr::u32(0),
                                Expr::var("prior_typedef_hash_idx"),
                                vec![Node::if_then(
                                    Expr::eq(
                                        Expr::var("prior_same_hash_prefix_done"),
                                        Expr::u32(0),
                                    ),
                                    vec![
                                        Node::let_bind(
                                            "prior_same_hash_prefix_idx",
                                            Expr::sub(
                                                Expr::sub(
                                                    Expr::var("prior_typedef_hash_idx"),
                                                    Expr::u32(1),
                                                ),
                                                Expr::var("prior_same_hash_prefix_scan"),
                                            ),
                                        ),
                                        Node::let_bind(
                                            "prior_same_hash_prefix_kind",
                                            Expr::load(
                                                vast_nodes,
                                                Expr::mul(
                                                    Expr::var("prior_same_hash_prefix_idx"),
                                                    Expr::u32(VAST_NODE_STRIDE_U32),
                                                ),
                                            ),
                                        ),
                                        Node::let_bind(
                                            "prior_same_hash_in_skipped_paren",
                                            Expr::or(
                                                Expr::gt(
                                                    Expr::var(
                                                        "prior_same_hash_skipped_paren_depth",
                                                    ),
                                                    Expr::u32(0),
                                                ),
                                                Expr::eq(
                                                    Expr::var("prior_same_hash_prefix_kind"),
                                                    Expr::u32(TOK_RPAREN),
                                                ),
                                            ),
                                        ),
                                        Node::let_bind(
                                            "prior_same_hash_in_skipped_brace",
                                            Expr::or(
                                                Expr::gt(
                                                    Expr::var(
                                                        "prior_same_hash_skipped_brace_depth",
                                                    ),
                                                    Expr::u32(0),
                                                ),
                                                Expr::eq(
                                                    Expr::var("prior_same_hash_prefix_kind"),
                                                    Expr::u32(TOK_RBRACE),
                                                ),
                                            ),
                                        ),
                                        Node::if_then(
                                            Expr::eq(
                                                Expr::var("prior_same_hash_prefix_kind"),
                                                Expr::u32(TOK_RBRACE),
                                            ),
                                            vec![Node::assign(
                                                "prior_same_hash_skipped_brace_depth",
                                                Expr::add(
                                                    Expr::var(
                                                        "prior_same_hash_skipped_brace_depth",
                                                    ),
                                                    Expr::u32(1),
                                                ),
                                            )],
                                        ),
                                        Node::if_then(
                                            Expr::and(
                                                Expr::gt(
                                                    Expr::var(
                                                        "prior_same_hash_skipped_brace_depth",
                                                    ),
                                                    Expr::u32(0),
                                                ),
                                                Expr::eq(
                                                    Expr::var("prior_same_hash_prefix_kind"),
                                                    Expr::u32(TOK_LBRACE),
                                                ),
                                            ),
                                            vec![Node::assign(
                                                "prior_same_hash_skipped_brace_depth",
                                                Expr::sub(
                                                    Expr::var(
                                                        "prior_same_hash_skipped_brace_depth",
                                                    ),
                                                    Expr::u32(1),
                                                ),
                                            )],
                                        ),
                                        Node::if_then(
                                            Expr::eq(
                                                Expr::var("prior_same_hash_prefix_kind"),
                                                Expr::u32(TOK_RPAREN),
                                            ),
                                            vec![Node::assign(
                                                "prior_same_hash_skipped_paren_depth",
                                                Expr::add(
                                                    Expr::var(
                                                        "prior_same_hash_skipped_paren_depth",
                                                    ),
                                                    Expr::u32(1),
                                                ),
                                            )],
                                        ),
                                        Node::if_then(
                                            Expr::and(
                                                Expr::gt(
                                                    Expr::var(
                                                        "prior_same_hash_skipped_paren_depth",
                                                    ),
                                                    Expr::u32(0),
                                                ),
                                                Expr::eq(
                                                    Expr::var("prior_same_hash_prefix_kind"),
                                                    Expr::u32(TOK_LPAREN),
                                                ),
                                            ),
                                            vec![Node::assign(
                                                "prior_same_hash_skipped_paren_depth",
                                                Expr::sub(
                                                    Expr::var(
                                                        "prior_same_hash_skipped_paren_depth",
                                                    ),
                                                    Expr::u32(1),
                                                ),
                                            )],
                                        ),
                                        Node::if_then(
                                            Expr::not(Expr::or(
                                                Expr::var("prior_same_hash_in_skipped_brace"),
                                                Expr::var("prior_same_hash_in_skipped_paren"),
                                            )),
                                            vec![
                                                Node::if_then(
                                                    Expr::eq(
                                                        Expr::var("prior_same_hash_prefix_kind"),
                                                        Expr::u32(TOK_TYPEDEF),
                                                    ),
                                                    vec![
                                                        Node::assign(
                                                            "prior_same_hash_has_typedef",
                                                            Expr::u32(1),
                                                        ),
                                                        Node::assign(
                                                            "prior_same_hash_prefix_done",
                                                            Expr::u32(1),
                                                        ),
                                                    ],
                                                ),
                                                Node::if_then(
                                                    is_decl_prefix_reset_token(Expr::var(
                                                        "prior_same_hash_prefix_kind",
                                                    )),
                                                    vec![Node::assign(
                                                        "prior_same_hash_prefix_done",
                                                        Expr::u32(1),
                                                    )],
                                                ),
                                            ],
                                        ),
                                    ],
                                )],
                            ),
                            Node::assign(
                                "has_prior_same_hash",
                                Expr::var("prior_same_hash_has_typedef"),
                            ),
                        ],
                    ),
                ],
            )],
        ),
        Node::let_bind("typedef_flags", Expr::u32(0)),
        Node::if_then(
            Expr::and(
                Expr::eq(Expr::var("raw_kind"), Expr::u32(TOK_IDENTIFIER)),
                Expr::and(
                    Expr::eq(Expr::var("is_global_typedef_hash"), Expr::u32(1)),
                    Expr::eq(Expr::var("has_prior_same_hash"), Expr::u32(1)),
                ),
            ),
            vec![Node::assign(
                "typedef_flags",
                Expr::bitor(
                    Expr::var("typedef_flags"),
                    Expr::u32(C_TYPEDEF_FLAG_VISIBLE_TYPEDEF_NAME),
                ),
            )],
        ),
        Node::if_then(
            Expr::and(
                Expr::eq(Expr::var("raw_kind"), Expr::u32(TOK_IDENTIFIER)),
                Expr::var("declaration_candidate"),
            ),
            vec![Node::assign(
                "typedef_flags",
                Expr::select(
                    Expr::eq(Expr::var("has_typedef_prefix"), Expr::u32(1)),
                    Expr::u32(C_TYPEDEF_FLAG_TYPEDEF_DECLARATOR),
                    Expr::u32(C_TYPEDEF_FLAG_ORDINARY_DECLARATOR),
                ),
            )],
        ),
    ];
    for field in 0..VAST_NODE_STRIDE_U32 {
        let value = match field {
            VAST_TYPEDEF_FLAGS_FIELD => Expr::var("typedef_flags"),
            _ => Expr::load(vast_nodes, Expr::add(base.clone(), Expr::u32(field))),
        };
        loop_body.push(Node::store(
            out_annotated_vast_nodes,
            Expr::add(base.clone(), Expr::u32(field)),
            value,
        ));
    }
    let n = node_count(&num_nodes).max(1);
    let typedef_count = node_count(&num_global_typedefs).max(1);
    Program::wrapped(
        vec![
            BufferDecl::storage(vast_nodes, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(n.saturating_mul(VAST_NODE_STRIDE_U32)),
            BufferDecl::storage(
                global_typedef_hashes,
                1,
                BufferAccess::ReadOnly,
                DataType::U32,
            )
            .with_count(typedef_count),
            BufferDecl::storage(
                out_annotated_vast_nodes,
                2,
                BufferAccess::ReadWrite,
                DataType::U32,
            )
            .with_count(n.saturating_mul(VAST_NODE_STRIDE_U32)),
        ],
        [256, 1, 1],
        vec![wrap_anonymous(
            ANNOTATE_TYPEDEF_OP_ID,
            vec![Node::if_then(Expr::lt(t, num_nodes), loop_body)],
        )],
    )
    .with_entry_op_id(ANNOTATE_TYPEDEF_OP_ID)
}
