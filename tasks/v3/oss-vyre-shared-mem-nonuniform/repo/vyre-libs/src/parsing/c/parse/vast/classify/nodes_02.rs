use super::*;

pub(super) fn extend(
    out: &mut Vec<Node>,
    vast_nodes: &str,
    _out_typed_vast_nodes: &str,
    _num_nodes: Expr,
    _t: Expr,
    _base: Expr,
    decl_contexts: Option<&str>,
) {
    let parent_ctx_scan_start = if let Some(decl_contexts) = decl_contexts {
        Expr::load(
            decl_contexts,
            Expr::add(
                Expr::mul(
                    Expr::var("safe_cur_parent_idx"),
                    Expr::u32(VAST_DECL_CONTEXT_STRIDE_U32),
                ),
                Expr::u32(VAST_DECL_CONTEXT_PREFIX_START_FIELD),
            ),
        )
    } else {
        Expr::u32(0)
    };
    let parent_parent_ctx_scan_start = if let Some(decl_contexts) = decl_contexts {
        Expr::load(
            decl_contexts,
            Expr::add(
                Expr::mul(
                    Expr::var("cur_parent_parent_safe_idx"),
                    Expr::u32(VAST_DECL_CONTEXT_STRIDE_U32),
                ),
                Expr::u32(VAST_DECL_CONTEXT_PREFIX_START_FIELD),
            ),
        )
    } else {
        Expr::u32(0)
    };
    out.extend(vec![
        Node::let_bind("parent_ctx_scan_start", parent_ctx_scan_start),
        Node::let_bind("parent_parent_ctx_scan_start", parent_parent_ctx_scan_start),
        Node::let_bind(
            "parent_decl_prefix_scan_needed",
            Expr::or(
                Expr::or(
                    Expr::eq(Expr::var("raw_kind"), Expr::u32(TOK_IDENTIFIER)),
                    Expr::eq(Expr::var("raw_kind"), Expr::u32(TOK_STAR)),
                ),
                Expr::or(
                    Expr::eq(Expr::var("raw_kind"), Expr::u32(TOK_LBRACKET)),
                    Expr::or(
                        Expr::eq(Expr::var("raw_kind"), Expr::u32(TOK_LPAREN)),
                        Expr::or(
                            Expr::eq(Expr::var("raw_kind"), Expr::u32(TOK_LBRACE)),
                            Expr::or(
                                Expr::eq(Expr::var("raw_kind"), Expr::u32(TOK_ASSIGN)),
                                Expr::eq(Expr::var("raw_kind"), Expr::u32(TOK_COLON)),
                            ),
                        ),
                    ),
                ),
            ),
        ),
        Node::if_then(
            Expr::var("parent_decl_prefix_scan_needed"),
            vec![Node::loop_for(
                "parent_ctx_scan",
                Expr::var("parent_ctx_scan_start"),
                Expr::var("parent_ctx_scan_limit"),
                vec![
                    Node::let_bind(
                        "parent_ctx_base",
                        Expr::mul(
                            Expr::var("parent_ctx_scan"),
                            Expr::u32(VAST_NODE_STRIDE_U32),
                        ),
                    ),
                    Node::let_bind(
                        "parent_ctx_kind",
                        Expr::load(vast_nodes, Expr::var("parent_ctx_base")),
                    ),
                    Node::let_bind(
                        "parent_ctx_typedef_flags",
                        Expr::load(
                            vast_nodes,
                            Expr::add(
                                Expr::var("parent_ctx_base"),
                                Expr::u32(VAST_TYPEDEF_FLAGS_FIELD),
                            ),
                        ),
                    ),
                    Node::let_bind(
                        "parent_ctx_symbol_hash",
                        Expr::load(
                            vast_nodes,
                            Expr::add(
                                Expr::var("parent_ctx_base"),
                                Expr::u32(VAST_TYPEDEF_SYMBOL_FIELD),
                            ),
                        ),
                    ),
                    Node::let_bind(
                        "parent_ctx_parent",
                        Expr::load(
                            vast_nodes,
                            Expr::add(Expr::var("parent_ctx_base"), Expr::u32(1)),
                        ),
                    ),
                    Node::if_then(
                        Expr::eq(
                            Expr::var("parent_ctx_parent"),
                            Expr::var("cur_parent_parent"),
                        ),
                        vec![
                            Node::let_bind(
                                "parent_ctx_aggregate_body_open",
                                is_aggregate_specifier_body_open(
                                    Expr::var("parent_ctx_kind"),
                                    Expr::var("parent_prev_kind"),
                                    Expr::var("parent_prev_prev_kind"),
                                ),
                            ),
                            Node::if_then(
                                is_decl_prefix_reset_token(Expr::var("parent_ctx_kind")),
                                vec![Node::assign("parent_has_decl_prefix", Expr::u32(0))],
                            ),
                            Node::if_then(
                                Expr::or(
                                    is_decl_prefix_token_or_gnu_type_hash(
                                        Expr::var("parent_ctx_kind"),
                                        Expr::var("parent_ctx_symbol_hash"),
                                    ),
                                    Expr::or(
                                        Expr::var("parent_ctx_aggregate_body_open"),
                                        Expr::and(
                                            Expr::eq(
                                                Expr::var("parent_ctx_kind"),
                                                Expr::u32(TOK_IDENTIFIER),
                                            ),
                                            is_typedef_name_annotation(Expr::var(
                                                "parent_ctx_typedef_flags",
                                            )),
                                        ),
                                    ),
                                ),
                                vec![Node::assign("parent_has_decl_prefix", Expr::u32(1))],
                            ),
                            Node::assign("parent_prev_prev_kind", Expr::var("parent_prev_kind")),
                            Node::assign(
                                "parent_prev_kind",
                                Expr::load(vast_nodes, Expr::var("parent_ctx_base")),
                            ),
                        ],
                    ),
                ],
            )],
        ),
        Node::let_bind("parent_parent_prev_kind", Expr::u32(SENTINEL)),
        Node::let_bind("parent_parent_prev_prev_kind", Expr::u32(SENTINEL)),
        Node::let_bind("parent_parent_has_decl_prefix", Expr::u32(0)),
        Node::if_then(
            Expr::and(
                Expr::var("parent_decl_prefix_scan_needed"),
                Expr::var("cur_parent_parent_valid"),
            ),
            vec![Node::loop_for(
                "parent_parent_ctx_scan",
                Expr::var("parent_parent_ctx_scan_start"),
                Expr::var("cur_parent_parent"),
                vec![
                    Node::let_bind(
                        "parent_parent_ctx_base",
                        Expr::mul(
                            Expr::var("parent_parent_ctx_scan"),
                            Expr::u32(VAST_NODE_STRIDE_U32),
                        ),
                    ),
                    Node::let_bind(
                        "parent_parent_ctx_kind",
                        Expr::load(vast_nodes, Expr::var("parent_parent_ctx_base")),
                    ),
                    Node::let_bind(
                        "parent_parent_ctx_typedef_flags",
                        Expr::load(
                            vast_nodes,
                            Expr::add(
                                Expr::var("parent_parent_ctx_base"),
                                Expr::u32(VAST_TYPEDEF_FLAGS_FIELD),
                            ),
                        ),
                    ),
                    Node::let_bind(
                        "parent_parent_ctx_symbol_hash",
                        Expr::load(
                            vast_nodes,
                            Expr::add(
                                Expr::var("parent_parent_ctx_base"),
                                Expr::u32(VAST_TYPEDEF_SYMBOL_FIELD),
                            ),
                        ),
                    ),
                    Node::let_bind(
                        "parent_parent_ctx_parent",
                        Expr::load(
                            vast_nodes,
                            Expr::add(Expr::var("parent_parent_ctx_base"), Expr::u32(1)),
                        ),
                    ),
                    Node::if_then(
                        Expr::eq(
                            Expr::var("parent_parent_ctx_parent"),
                            Expr::var("cur_parent_parent_parent"),
                        ),
                        vec![
                            Node::let_bind(
                                "parent_parent_ctx_aggregate_body_open",
                                is_aggregate_specifier_body_open(
                                    Expr::var("parent_parent_ctx_kind"),
                                    Expr::var("parent_parent_prev_kind"),
                                    Expr::var("parent_parent_prev_prev_kind"),
                                ),
                            ),
                            Node::if_then(
                                is_decl_prefix_reset_token(Expr::var("parent_parent_ctx_kind")),
                                vec![Node::assign("parent_parent_has_decl_prefix", Expr::u32(0))],
                            ),
                            Node::if_then(
                                Expr::or(
                                    is_decl_prefix_token_or_gnu_type_hash(
                                        Expr::var("parent_parent_ctx_kind"),
                                        Expr::var("parent_parent_ctx_symbol_hash"),
                                    ),
                                    Expr::or(
                                        Expr::var("parent_parent_ctx_aggregate_body_open"),
                                        Expr::and(
                                            Expr::eq(
                                                Expr::var("parent_parent_ctx_kind"),
                                                Expr::u32(TOK_IDENTIFIER),
                                            ),
                                            is_typedef_name_annotation(Expr::var(
                                                "parent_parent_ctx_typedef_flags",
                                            )),
                                        ),
                                    ),
                                ),
                                vec![Node::assign("parent_parent_has_decl_prefix", Expr::u32(1))],
                            ),
                            Node::assign(
                                "parent_parent_prev_prev_kind",
                                Expr::var("parent_parent_prev_kind"),
                            ),
                            Node::assign(
                                "parent_parent_prev_kind",
                                Expr::var("parent_parent_ctx_kind"),
                            ),
                        ],
                    ),
                ],
            )],
        ),
        Node::let_bind("ancestor_decl_prefix", Expr::u32(0)),
        Node::let_bind("decl_ancestor", Expr::var("cur_parent")),
        Node::let_bind(
            "decl_ancestor_active",
            Expr::select(
                Expr::var("parent_decl_prefix_scan_needed"),
                Expr::u32(1),
                Expr::u32(0),
            ),
        ),
    ]);
}
