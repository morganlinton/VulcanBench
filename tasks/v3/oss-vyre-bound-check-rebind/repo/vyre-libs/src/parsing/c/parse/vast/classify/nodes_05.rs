use super::*;

pub(super) fn extend(
    out: &mut Vec<Node>,
    vast_nodes: &str,
    _out_typed_vast_nodes: &str,
    num_nodes: Expr,
    t: Expr,
    _base: Expr,
    typedef_annotations_available: bool,
) {
    let has_typedef_annotations = if typedef_annotations_available {
        Expr::u32(1)
    } else {
        Expr::u32(0)
    };
    let prior_typedef_scan_needed = if typedef_annotations_available {
        Expr::eq(Expr::u32(1), Expr::u32(0))
    } else {
        Expr::or(
            Expr::var("raw_lparen"),
            Expr::or(
                Expr::eq(Expr::var("raw_kind"), Expr::u32(TOK_STAR)),
                Expr::eq(Expr::var("raw_kind"), Expr::u32(TOK_LBRACE)),
            ),
        )
    };
    out.extend(vec![
        Node::let_bind(
            "in_parenthesized_declarator",
            Expr::and(
                Expr::eq(Expr::var("cur_parent_kind"), Expr::u32(TOK_LPAREN)),
                Expr::or(
                    Expr::eq(Expr::var("parent_has_decl_prefix"), Expr::u32(1)),
                    Expr::or(
                        is_typeof_operator_token(
                            Expr::var("cur_parent_parent_kind"),
                            Expr::var("cur_parent_parent_symbol_hash"),
                        ),
                        Expr::and(
                            Expr::or(
                                Expr::eq(
                                    Expr::var("cur_parent_parent_kind"),
                                    Expr::u32(TOK_LPAREN),
                                ),
                                Expr::eq(Expr::var("ancestor_decl_prefix"), Expr::u32(1)),
                            ),
                            Expr::or(
                                Expr::eq(Expr::var("parent_parent_has_decl_prefix"), Expr::u32(1)),
                                Expr::eq(Expr::var("ancestor_decl_prefix"), Expr::u32(1)),
                            ),
                        ),
                    ),
                ),
            ),
        ),
        Node::let_bind(
            "effective_has_decl_prefix",
            Expr::select(
                Expr::or(
                    Expr::eq(Expr::var("has_decl_prefix"), Expr::u32(1)),
                    Expr::var("in_parenthesized_declarator"),
                ),
                Expr::u32(1),
                Expr::u32(0),
            ),
        ),
        Node::let_bind(
            "raw_lparen",
            Expr::eq(Expr::var("raw_kind"), Expr::u32(TOK_LPAREN)),
        ),
        Node::let_bind("prior_typedef_scan_needed", prior_typedef_scan_needed),
        Node::let_bind("has_typedef_annotations", has_typedef_annotations),
        Node::let_bind("has_prior_typedef", Expr::u32(0)),
        Node::let_bind("has_prior_ordinary_decl", Expr::u32(0)),
        Node::let_bind("has_prior_parenthesized_identifier_statement", Expr::u32(0)),
        Node::if_then(
            Expr::var("prior_typedef_scan_needed"),
            vec![
                Node::loop_for(
                    "typedef_annotation_scan",
                    Expr::u32(0),
                    num_nodes.clone(),
                    vec![Node::if_then(
                        Expr::ne(
                            Expr::load(
                                vast_nodes,
                                Expr::add(
                                    Expr::mul(
                                        Expr::var("typedef_annotation_scan"),
                                        Expr::u32(VAST_NODE_STRIDE_U32),
                                    ),
                                    Expr::u32(VAST_TYPEDEF_FLAGS_FIELD),
                                ),
                            ),
                            Expr::u32(0),
                        ),
                        vec![Node::assign("has_typedef_annotations", Expr::u32(1))],
                    )],
                ),
                Node::loop_for(
                    "prior_typedef_scan",
                    Expr::u32(0),
                    t.clone(),
                    vec![
                        Node::let_bind(
                            "prior_typedef_base",
                            Expr::mul(
                                Expr::var("prior_typedef_scan"),
                                Expr::u32(VAST_NODE_STRIDE_U32),
                            ),
                        ),
                        Node::if_then(
                            Expr::eq(
                                Expr::load(vast_nodes, Expr::var("prior_typedef_base")),
                                Expr::u32(TOK_TYPEDEF),
                            ),
                            vec![Node::assign("has_prior_typedef", Expr::u32(1))],
                        ),
                        Node::let_bind(
                            "prior_scan_prev_kind",
                            Expr::select(
                                Expr::gt(Expr::var("prior_typedef_scan"), Expr::u32(0)),
                                Expr::load(
                                    vast_nodes,
                                    Expr::sub(
                                        Expr::var("prior_typedef_base"),
                                        Expr::u32(VAST_NODE_STRIDE_U32),
                                    ),
                                ),
                                Expr::u32(SENTINEL),
                            ),
                        ),
                        Node::let_bind(
                            "prior_scan_prev_prev_kind",
                            Expr::select(
                                Expr::gt(Expr::var("prior_typedef_scan"), Expr::u32(1)),
                                Expr::load(
                                    vast_nodes,
                                    Expr::sub(
                                        Expr::var("prior_typedef_base"),
                                        Expr::u32(VAST_NODE_STRIDE_U32 * 2),
                                    ),
                                ),
                                Expr::u32(SENTINEL),
                            ),
                        ),
                        Node::let_bind(
                            "prior_scan_parent",
                            Expr::load(
                                vast_nodes,
                                Expr::add(Expr::var("prior_typedef_base"), Expr::u32(1)),
                            ),
                        ),
                        Node::let_bind(
                            "prior_scan_parent_kind",
                            Expr::select(
                                Expr::lt(Expr::var("prior_scan_parent"), num_nodes.clone()),
                                Expr::load(
                                    vast_nodes,
                                    Expr::mul(
                                        Expr::var("prior_scan_parent"),
                                        Expr::u32(VAST_NODE_STRIDE_U32),
                                    ),
                                ),
                                Expr::u32(SENTINEL),
                            ),
                        ),
                        Node::let_bind(
                            "prior_scan_parent_prev_kind",
                            Expr::select(
                                Expr::and(
                                    Expr::lt(Expr::var("prior_scan_parent"), num_nodes.clone()),
                                    Expr::gt(Expr::var("prior_scan_parent"), Expr::u32(0)),
                                ),
                                Expr::load(
                                    vast_nodes,
                                    Expr::mul(
                                        Expr::sub(Expr::var("prior_scan_parent"), Expr::u32(1)),
                                        Expr::u32(VAST_NODE_STRIDE_U32),
                                    ),
                                ),
                                Expr::u32(SENTINEL),
                            ),
                        ),
                        Node::let_bind(
                            "prior_scan_parent_prev_prev_kind",
                            Expr::select(
                                Expr::and(
                                    Expr::lt(Expr::var("prior_scan_parent"), num_nodes.clone()),
                                    Expr::gt(Expr::var("prior_scan_parent"), Expr::u32(1)),
                                ),
                                Expr::load(
                                    vast_nodes,
                                    Expr::mul(
                                        Expr::sub(Expr::var("prior_scan_parent"), Expr::u32(2)),
                                        Expr::u32(VAST_NODE_STRIDE_U32),
                                    ),
                                ),
                                Expr::u32(SENTINEL),
                            ),
                        ),
                        Node::let_bind(
                            "prior_scan_in_aggregate_body",
                            Expr::and(
                                Expr::eq(
                                    Expr::var("prior_scan_parent_kind"),
                                    Expr::u32(TOK_LBRACE),
                                ),
                                Expr::or(
                                    any_token_eq(
                                        Expr::var("prior_scan_parent_prev_kind"),
                                        &[TOK_STRUCT, TOK_UNION, TOK_ENUM],
                                    ),
                                    Expr::and(
                                        Expr::eq(
                                            Expr::var("prior_scan_parent_prev_kind"),
                                            Expr::u32(TOK_IDENTIFIER),
                                        ),
                                        any_token_eq(
                                            Expr::var("prior_scan_parent_prev_prev_kind"),
                                            &[TOK_STRUCT, TOK_UNION, TOK_ENUM],
                                        ),
                                    ),
                                ),
                            ),
                        ),
                        Node::let_bind(
                            "prior_scan_next_kind",
                            Expr::select(
                                Expr::lt(
                                    Expr::add(Expr::var("prior_typedef_scan"), Expr::u32(1)),
                                    num_nodes.clone(),
                                ),
                                Expr::load(
                                    vast_nodes,
                                    Expr::add(
                                        Expr::var("prior_typedef_base"),
                                        Expr::u32(VAST_NODE_STRIDE_U32),
                                    ),
                                ),
                                Expr::u32(SENTINEL),
                            ),
                        ),
                        Node::if_then(
                            Expr::and(
                                Expr::and(
                                    Expr::eq(
                                        Expr::load(vast_nodes, Expr::var("prior_typedef_base")),
                                        Expr::u32(TOK_IDENTIFIER),
                                    ),
                                    Expr::and(
                                        is_decl_prefix_token(Expr::var("prior_scan_prev_kind")),
                                        Expr::and(
                                            Expr::ne(
                                                Expr::var("prior_scan_prev_kind"),
                                                Expr::u32(TOK_TYPEDEF),
                                            ),
                                            Expr::ne(
                                                Expr::var("prior_scan_prev_prev_kind"),
                                                Expr::u32(TOK_TYPEDEF),
                                            ),
                                        ),
                                    ),
                                ),
                                Expr::and(
                                    Expr::not(Expr::var("prior_scan_in_aggregate_body")),
                                    any_token_eq(
                                        Expr::var("prior_scan_next_kind"),
                                        &[TOK_SEMICOLON, TOK_COMMA, TOK_ASSIGN, TOK_LBRACKET],
                                    ),
                                ),
                            ),
                            vec![Node::assign("has_prior_ordinary_decl", Expr::u32(1))],
                        ),
                        Node::if_then(
                            Expr::and(
                                Expr::lt(
                                    Expr::add(Expr::var("prior_typedef_scan"), Expr::u32(5)),
                                    t.clone(),
                                ),
                                Expr::and(
                                    Expr::eq(
                                        Expr::load(vast_nodes, Expr::var("prior_typedef_base")),
                                        Expr::u32(TOK_LPAREN),
                                    ),
                                    Expr::and(
                                        Expr::eq(
                                            Expr::load(
                                                vast_nodes,
                                                Expr::add(
                                                    Expr::var("prior_typedef_base"),
                                                    Expr::u32(VAST_NODE_STRIDE_U32),
                                                ),
                                            ),
                                            Expr::u32(TOK_IDENTIFIER),
                                        ),
                                        Expr::and(
                                            Expr::eq(
                                                Expr::load(
                                                    vast_nodes,
                                                    Expr::add(
                                                        Expr::var("prior_typedef_base"),
                                                        Expr::u32(VAST_NODE_STRIDE_U32 * 2),
                                                    ),
                                                ),
                                                Expr::u32(TOK_RPAREN),
                                            ),
                                            Expr::eq(
                                                Expr::load(
                                                    vast_nodes,
                                                    Expr::add(
                                                        Expr::var("prior_typedef_base"),
                                                        Expr::u32(VAST_NODE_STRIDE_U32 * 5),
                                                    ),
                                                ),
                                                Expr::u32(TOK_SEMICOLON),
                                            ),
                                        ),
                                    ),
                                ),
                            ),
                            vec![Node::assign(
                                "has_prior_parenthesized_identifier_statement",
                                Expr::u32(1),
                            )],
                        ),
                        Node::if_then(
                            Expr::and(
                                Expr::eq(Expr::var("has_typedef_annotations"), Expr::u32(0)),
                                Expr::eq(
                                    Expr::load(
                                        vast_nodes,
                                        Expr::add(
                                            Expr::var("prior_typedef_base"),
                                            Expr::u32(VAST_TYPEDEF_FLAGS_FIELD),
                                        ),
                                    ),
                                    Expr::u32(C_TYPEDEF_FLAG_ORDINARY_DECLARATOR),
                                ),
                            ),
                            vec![Node::assign("has_prior_ordinary_decl", Expr::u32(1))],
                        ),
                    ],
                ),
            ],
        ),
        Node::let_bind(
            "ambiguous_parenthesized_identifier_multiply",
            Expr::and(
                Expr::and(
                    Expr::var("raw_lparen"),
                    Expr::eq(Expr::var("next_kind"), Expr::u32(TOK_STAR)),
                ),
                Expr::eq(
                    Expr::var("has_prior_parenthesized_identifier_statement"),
                    Expr::u32(1),
                ),
            ),
        ),
        Node::let_bind(
            "fallback_has_prior_typedef",
            Expr::and(
                Expr::and(
                    Expr::eq(Expr::var("has_typedef_annotations"), Expr::u32(0)),
                    Expr::eq(Expr::var("has_prior_typedef"), Expr::u32(1)),
                ),
                Expr::and(
                    Expr::eq(Expr::var("has_prior_ordinary_decl"), Expr::u32(0)),
                    Expr::not(Expr::var("ambiguous_parenthesized_identifier_multiply")),
                ),
            ),
        ),
    ]);
}
