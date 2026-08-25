use super::*;

pub(super) fn extend(
    out: &mut Vec<Node>,
    vast_nodes: &str,
    _out_typed_vast_nodes: &str,
    num_nodes: Expr,
    t: Expr,
    _base: Expr,
    decl_contexts: Option<&str>,
) {
    let decl_ctx_scan_start = if let Some(decl_contexts) = decl_contexts {
        Expr::load(
            decl_contexts,
            Expr::add(
                Expr::mul(t.clone(), Expr::u32(VAST_DECL_CONTEXT_STRIDE_U32)),
                Expr::u32(VAST_DECL_CONTEXT_PREFIX_START_FIELD),
            ),
        )
    } else {
        Expr::u32(0)
    };
    out.extend(vec![
        Node::if_then(
            Expr::eq(Expr::var("decl_ancestor_active"), Expr::u32(1)),
            vec![Node::loop_for(
                "decl_ancestor_depth",
                Expr::u32(0),
                num_nodes.clone(),
                vec![Node::if_then(
                    Expr::and(
                        Expr::eq(Expr::var("decl_ancestor_active"), Expr::u32(1)),
                        Expr::lt(Expr::var("decl_ancestor"), num_nodes.clone()),
                    ),
                    vec![
                        Node::let_bind(
                            "decl_ancestor_base",
                            Expr::mul(Expr::var("decl_ancestor"), Expr::u32(VAST_NODE_STRIDE_U32)),
                        ),
                        Node::let_bind(
                            "decl_ancestor_kind",
                            Expr::load(vast_nodes, Expr::var("decl_ancestor_base")),
                        ),
                        Node::let_bind(
                            "decl_ancestor_parent",
                            Expr::load(
                                vast_nodes,
                                Expr::add(Expr::var("decl_ancestor_base"), Expr::u32(1)),
                            ),
                        ),
                        Node::if_then(
                            Expr::ne(Expr::var("decl_ancestor_kind"), Expr::u32(TOK_LPAREN)),
                            vec![Node::assign("decl_ancestor_active", Expr::u32(0))],
                        ),
                        Node::if_then(
                            Expr::and(
                                Expr::eq(Expr::var("decl_ancestor_active"), Expr::u32(1)),
                                Expr::eq(Expr::var("decl_ancestor_kind"), Expr::u32(TOK_LPAREN)),
                            ),
                            vec![
                                Node::let_bind("ancestor_prev_kind", Expr::u32(SENTINEL)),
                                Node::let_bind("ancestor_prev_prev_kind", Expr::u32(SENTINEL)),
                                Node::let_bind("ancestor_has_decl_prefix", Expr::u32(0)),
                                Node::loop_for(
                                    "ancestor_ctx_scan",
                                    Expr::u32(0),
                                    Expr::var("decl_ancestor"),
                                    vec![
                                        Node::let_bind(
                                            "ancestor_ctx_base",
                                            Expr::mul(
                                                Expr::var("ancestor_ctx_scan"),
                                                Expr::u32(VAST_NODE_STRIDE_U32),
                                            ),
                                        ),
                                        Node::let_bind(
                                            "ancestor_ctx_kind",
                                            Expr::load(vast_nodes, Expr::var("ancestor_ctx_base")),
                                        ),
                                        Node::let_bind(
                                            "ancestor_ctx_typedef_flags",
                                            Expr::load(
                                                vast_nodes,
                                                Expr::add(
                                                    Expr::var("ancestor_ctx_base"),
                                                    Expr::u32(VAST_TYPEDEF_FLAGS_FIELD),
                                                ),
                                            ),
                                        ),
                                        Node::let_bind(
                                            "ancestor_ctx_symbol_hash",
                                            Expr::load(
                                                vast_nodes,
                                                Expr::add(
                                                    Expr::var("ancestor_ctx_base"),
                                                    Expr::u32(VAST_TYPEDEF_SYMBOL_FIELD),
                                                ),
                                            ),
                                        ),
                                        Node::let_bind(
                                            "ancestor_ctx_parent",
                                            Expr::load(
                                                vast_nodes,
                                                Expr::add(
                                                    Expr::var("ancestor_ctx_base"),
                                                    Expr::u32(1),
                                                ),
                                            ),
                                        ),
                                        Node::if_then(
                                            Expr::eq(
                                                Expr::var("ancestor_ctx_parent"),
                                                Expr::var("decl_ancestor_parent"),
                                            ),
                                            vec![
                                            Node::let_bind(
                                                "ancestor_ctx_aggregate_body_open",
                                                is_aggregate_specifier_body_open(
                                                    Expr::var("ancestor_ctx_kind"),
                                                    Expr::var("ancestor_prev_kind"),
                                                    Expr::var("ancestor_prev_prev_kind"),
                                                ),
                                            ),
                                            Node::if_then(
                                                is_decl_prefix_reset_token(Expr::var(
                                                    "ancestor_ctx_kind",
                                                )),
                                                vec![Node::assign(
                                                    "ancestor_has_decl_prefix",
                                                    Expr::u32(0),
                                                )],
                                            ),
                                            Node::if_then(
                                                Expr::or(
                                                    is_decl_prefix_token_or_gnu_type_hash(
                                                        Expr::var("ancestor_ctx_kind"),
                                                        Expr::var("ancestor_ctx_symbol_hash"),
                                                    ),
                                                    Expr::or(
                                                        Expr::var(
                                                            "ancestor_ctx_aggregate_body_open",
                                                        ),
                                                        Expr::and(
                                                            Expr::eq(
                                                                Expr::var("ancestor_ctx_kind"),
                                                                Expr::u32(TOK_IDENTIFIER),
                                                            ),
                                                            is_typedef_name_annotation(Expr::var(
                                                                "ancestor_ctx_typedef_flags",
                                                            )),
                                                        ),
                                                    ),
                                                ),
                                                vec![Node::assign(
                                                    "ancestor_has_decl_prefix",
                                                    Expr::u32(1),
                                                )],
                                            ),
                                            Node::assign(
                                                "ancestor_prev_prev_kind",
                                                Expr::var("ancestor_prev_kind"),
                                            ),
                                            Node::assign(
                                                "ancestor_prev_kind",
                                                Expr::var("ancestor_ctx_kind"),
                                            ),
                                        ],
                                        ),
                                    ],
                                ),
                                Node::if_then(
                                    Expr::eq(Expr::var("ancestor_has_decl_prefix"), Expr::u32(1)),
                                    vec![Node::assign("ancestor_decl_prefix", Expr::u32(1))],
                                ),
                            ],
                        ),
                        Node::if_then(
                            Expr::eq(Expr::var("decl_ancestor_active"), Expr::u32(1)),
                            vec![Node::assign(
                                "decl_ancestor",
                                Expr::var("decl_ancestor_parent"),
                            )],
                        ),
                    ],
                )],
            )],
        ),
        Node::let_bind("parent_open_record_prefix", Expr::u32(0)),
        Node::let_bind("parent_open_enum_prefix", Expr::u32(0)),
        Node::if_then(
            Expr::and(
                Expr::var("cur_parent_valid"),
                Expr::eq(Expr::var("cur_parent_kind"), Expr::u32(TOK_LBRACE)),
            ),
            vec![
                Node::let_bind(
                    "parent_open_ctx_cursor",
                    Expr::load(
                        vast_nodes,
                        Expr::add(
                            Expr::var("cur_parent_base"),
                            Expr::u32(VAST_PREVIOUS_SIBLING_FIELD),
                        ),
                    ),
                ),
                Node::let_bind("parent_open_ctx_done", Expr::u32(0)),
                Node::loop_for(
                    "parent_open_ctx_scan",
                    Expr::u32(0),
                    Expr::var("cur_parent"),
                    vec![Node::if_then(
                        Expr::eq(Expr::var("parent_open_ctx_done"), Expr::u32(0)),
                        vec![
                            Node::let_bind(
                                "parent_open_ctx_cursor_valid",
                                Expr::lt(Expr::var("parent_open_ctx_cursor"), num_nodes.clone()),
                            ),
                            Node::let_bind(
                                "parent_open_ctx_safe_cursor",
                                Expr::select(
                                    Expr::var("parent_open_ctx_cursor_valid"),
                                    Expr::var("parent_open_ctx_cursor"),
                                    Expr::u32(0),
                                ),
                            ),
                            Node::let_bind(
                                "parent_open_ctx_base",
                                Expr::mul(
                                    Expr::var("parent_open_ctx_safe_cursor"),
                                    Expr::u32(VAST_NODE_STRIDE_U32),
                                ),
                            ),
                            Node::let_bind(
                                "parent_open_ctx_kind",
                                Expr::load(vast_nodes, Expr::var("parent_open_ctx_base")),
                            ),
                            Node::if_then(
                                Expr::var("parent_open_ctx_cursor_valid"),
                                vec![
                                    Node::if_then(
                                        any_token_eq(
                                            Expr::var("parent_open_ctx_kind"),
                                            &[TOK_SEMICOLON, TOK_ASSIGN, TOK_COMMA],
                                        ),
                                        vec![
                                            Node::assign("parent_open_record_prefix", Expr::u32(0)),
                                            Node::assign("parent_open_enum_prefix", Expr::u32(0)),
                                            Node::assign("parent_open_ctx_done", Expr::u32(1)),
                                        ],
                                    ),
                                    Node::if_then(
                                        any_token_eq(
                                            Expr::var("parent_open_ctx_kind"),
                                            &[TOK_STRUCT, TOK_UNION],
                                        ),
                                        vec![
                                            Node::assign("parent_open_record_prefix", Expr::u32(1)),
                                            Node::assign("parent_open_enum_prefix", Expr::u32(0)),
                                            Node::assign("parent_open_ctx_done", Expr::u32(1)),
                                        ],
                                    ),
                                    Node::if_then(
                                        Expr::eq(
                                            Expr::var("parent_open_ctx_kind"),
                                            Expr::u32(TOK_ENUM),
                                        ),
                                        vec![
                                            Node::assign("parent_open_record_prefix", Expr::u32(0)),
                                            Node::assign("parent_open_enum_prefix", Expr::u32(1)),
                                            Node::assign("parent_open_ctx_done", Expr::u32(1)),
                                        ],
                                    ),
                                    Node::assign(
                                        "parent_open_ctx_cursor",
                                        Expr::load(
                                            vast_nodes,
                                            Expr::add(
                                                Expr::var("parent_open_ctx_base"),
                                                Expr::u32(VAST_PREVIOUS_SIBLING_FIELD),
                                            ),
                                        ),
                                    ),
                                ],
                            ),
                            Node::if_then(
                                Expr::not(Expr::var("parent_open_ctx_cursor_valid")),
                                vec![Node::assign("parent_open_ctx_done", Expr::u32(1))],
                            ),
                        ],
                    )],
                ),
            ],
        ),
        Node::let_bind(
            "parent_is_record_body",
            Expr::and(
                Expr::and(
                    Expr::var("cur_parent_valid"),
                    Expr::eq(Expr::var("cur_parent_kind"), Expr::u32(TOK_LBRACE)),
                ),
                Expr::eq(Expr::var("parent_open_record_prefix"), Expr::u32(1)),
            ),
        ),
        Node::let_bind(
            "parent_is_enum_body",
            Expr::and(
                Expr::and(
                    Expr::var("cur_parent_valid"),
                    Expr::eq(Expr::var("cur_parent_kind"), Expr::u32(TOK_LBRACE)),
                ),
                Expr::eq(Expr::var("parent_open_enum_prefix"), Expr::u32(1)),
            ),
        ),
        Node::let_bind(
            "identifier_then_paren",
            Expr::and(
                Expr::and(
                    Expr::eq(Expr::var("raw_kind"), Expr::u32(TOK_IDENTIFIER)),
                    Expr::var("next_valid"),
                ),
                Expr::eq(Expr::var("next_kind"), Expr::u32(TOK_LPAREN)),
            ),
        ),
        Node::let_bind("has_decl_prefix", Expr::u32(0)),
        Node::let_bind("decl_ctx_leading_gnu_attribute", Expr::u32(0)),
        Node::let_bind("decl_ctx_last_reset_idx", Expr::u32(SENTINEL)),
        Node::let_bind("last_decl_ctx_kind", Expr::u32(SENTINEL)),
        Node::let_bind("prev_decl_ctx_kind", Expr::u32(SENTINEL)),
        Node::let_bind("suffix_has_gnu_attribute", Expr::u32(0)),
        Node::let_bind("suffix_boundary", Expr::u32(0)),
        Node::let_bind("suffix_boundary_kind", Expr::u32(SENTINEL)),
        Node::let_bind(
            "needs_decl_context",
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
        Node::let_bind("decl_ctx_scan_start", decl_ctx_scan_start),
    ]);
}
