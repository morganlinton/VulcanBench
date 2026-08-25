use super::*;

pub(super) fn extend(
    out: &mut Vec<Node>,
    vast_nodes: &str,
    _out_typed_vast_nodes: &str,
    _num_nodes: Expr,
    t: Expr,
    _base: Expr,
) {
    out.extend(vec![
        Node::let_bind(
            "type_name_paren",
            Expr::and(
                Expr::and(
                    Expr::var("raw_lparen"),
                    Expr::not(Expr::or(
                        any_token_eq(
                            Expr::var("prev_sibling_kind"),
                            &[TOK_SIZEOF, TOK_ALIGNOF, TOK_ATOMIC],
                        ),
                        is_typeof_operator_token(
                            Expr::var("prev_sibling_kind"),
                            Expr::var("prev_sibling_symbol_hash"),
                        ),
                    )),
                ),
                Expr::or(
                    Expr::or(
                        is_type_name_start_token(Expr::var("first_child_kind")),
                        is_typeof_operator_token(
                            Expr::var("first_child_kind"),
                            Expr::var("first_child_symbol_hash"),
                        ),
                    ),
                    Expr::and(
                        Expr::or(
                            is_type_name_identifier(
                                Expr::var("first_child_typedef_flags"),
                                Expr::var("fallback_has_prior_typedef"),
                            ),
                            Expr::var("flat_identifier_type_name_paren"),
                        ),
                        Expr::or(
                            Expr::var("identifier_type_name_paren"),
                            Expr::var("flat_identifier_type_name_paren"),
                        ),
                    ),
                ),
            ),
        ),
        Node::let_bind(
            "is_return_function_suffix",
            Expr::and(
                Expr::and(
                    Expr::and(Expr::var("raw_lparen"), Expr::var("type_name_paren")),
                    Expr::var("function_boundary"),
                ),
                Expr::and(
                    Expr::eq(Expr::var("effective_has_decl_prefix"), Expr::u32(1)),
                    Expr::eq(Expr::var("prev_sibling_kind"), Expr::u32(TOK_LPAREN)),
                ),
            ),
        ),
        Node::let_bind(
            "is_function_declarator",
            Expr::or(
                Expr::or(
                    Expr::and(
                        Expr::and(Expr::var("raw_lparen"), Expr::var("function_boundary")),
                        Expr::and(
                            Expr::eq(Expr::var("effective_has_decl_prefix"), Expr::u32(1)),
                            any_token_eq(
                                Expr::var("prev_sibling_kind"),
                                &[TOK_IDENTIFIER, TOK_LPAREN, TOK_RPAREN],
                            ),
                        ),
                    ),
                    Expr::and(
                        Expr::and(
                            Expr::var("raw_lparen"),
                            Expr::or(
                                Expr::var("type_name_paren"),
                                is_type_name_start_token(Expr::var("first_child_kind")),
                            ),
                        ),
                        any_token_eq(Expr::var("prev_sibling_kind"), &[TOK_LPAREN, TOK_RPAREN]),
                    ),
                ),
                Expr::var("is_return_function_suffix"),
            ),
        ),
        Node::let_bind(
            "prev_sibling_is_attribute_lparen",
            Expr::and(
                Expr::eq(Expr::var("prev_sibling_kind"), Expr::u32(TOK_LPAREN)),
                Expr::eq(
                    Expr::var("prev_prev_sibling_kind"),
                    Expr::u32(TOK_GNU_ATTRIBUTE),
                ),
            ),
        ),
        Node::let_bind(
            "is_function_decl",
            Expr::and(
                Expr::and(
                    Expr::var("identifier_then_paren"),
                    Expr::var("function_boundary"),
                ),
                Expr::and(
                    Expr::eq(Expr::var("effective_has_decl_prefix"), Expr::u32(1)),
                    Expr::or(
                        Expr::ne(Expr::var("prev_sibling_kind"), Expr::u32(TOK_LPAREN)),
                        Expr::var("prev_sibling_is_attribute_lparen"),
                    ),
                ),
            ),
        ),
        Node::let_bind(
            "is_function_definition",
            Expr::and(
                Expr::var("is_function_decl"),
                Expr::eq(Expr::var("suffix_boundary_kind"), Expr::u32(TOK_LBRACE)),
            ),
        ),
        Node::let_bind(
            "aggregate_decl_kind",
            Expr::select(
                Expr::eq(Expr::var("raw_kind"), Expr::u32(TOK_STRUCT)),
                Expr::u32(C_AST_KIND_STRUCT_DECL),
                Expr::select(
                    Expr::eq(Expr::var("raw_kind"), Expr::u32(TOK_UNION)),
                    Expr::u32(C_AST_KIND_UNION_DECL),
                    Expr::select(
                        Expr::eq(Expr::var("raw_kind"), Expr::u32(TOK_ENUM)),
                        Expr::u32(C_AST_KIND_ENUM_DECL),
                        Expr::u32(0),
                    ),
                ),
            ),
        ),
        Node::let_bind(
            "is_typedef_decl",
            Expr::eq(Expr::var("raw_kind"), Expr::u32(TOK_TYPEDEF)),
        ),
        Node::let_bind(
            "is_static_assert_decl",
            Expr::eq(Expr::var("raw_kind"), Expr::u32(TOK_STATIC_ASSERT)),
        ),
        Node::let_bind(
            "is_call",
            Expr::and(
                Expr::var("identifier_then_paren"),
                Expr::not(Expr::var("is_function_decl")),
            ),
        ),
        Node::let_bind(
            "inside_gnu_statement_expr_body",
            Expr::and(
                Expr::eq(Expr::var("cur_parent_kind"), Expr::u32(TOK_LBRACE)),
                Expr::eq(Expr::var("cur_parent_parent_kind"), Expr::u32(TOK_LPAREN)),
            ),
        ),
        Node::let_bind(
            "c99_for_init_statement_assign",
            Expr::and(
                Expr::and(
                    Expr::eq(Expr::var("raw_kind"), Expr::u32(TOK_ASSIGN)),
                    Expr::and(
                        Expr::eq(Expr::var("cur_parent_kind"), Expr::u32(TOK_LPAREN)),
                        Expr::eq(Expr::var("effective_has_decl_prefix"), Expr::u32(1)),
                    ),
                ),
                Expr::or(
                    Expr::eq(
                        Expr::var("cur_parent_prev_sibling_kind"),
                        Expr::u32(TOK_FOR),
                    ),
                    Expr::eq(Expr::var("cur_parent_parent_kind"), Expr::u32(TOK_FOR)),
                ),
            ),
        ),
        Node::let_bind("declaration_initializer_prefix", Expr::u32(0)),
        Node::if_then(
            Expr::eq(Expr::var("raw_kind"), Expr::u32(TOK_ASSIGN)),
            vec![Node::loop_for(
                "decl_init_scan",
                Expr::u32(0),
                t.clone(),
                vec![
                    Node::let_bind(
                        "decl_init_base",
                        Expr::mul(Expr::var("decl_init_scan"), Expr::u32(VAST_NODE_STRIDE_U32)),
                    ),
                    Node::let_bind(
                        "decl_init_parent",
                        Expr::load(
                            vast_nodes,
                            Expr::add(Expr::var("decl_init_base"), Expr::u32(1)),
                        ),
                    ),
                    Node::if_then(
                        Expr::eq(Expr::var("decl_init_parent"), Expr::var("cur_parent")),
                        vec![
                            Node::let_bind(
                                "decl_init_kind",
                                Expr::load(vast_nodes, Expr::var("decl_init_base")),
                            ),
                            Node::let_bind(
                                "decl_init_symbol_hash",
                                Expr::load(
                                    vast_nodes,
                                    Expr::add(
                                        Expr::var("decl_init_base"),
                                        Expr::u32(VAST_TYPEDEF_SYMBOL_FIELD),
                                    ),
                                ),
                            ),
                            Node::if_then(
                                any_token_eq(
                                    Expr::var("decl_init_kind"),
                                    &[TOK_SEMICOLON, TOK_LBRACE, TOK_RBRACE],
                                ),
                                vec![Node::assign("declaration_initializer_prefix", Expr::u32(0))],
                            ),
                            Node::if_then(
                                is_decl_prefix_token_or_gnu_type_hash(
                                    Expr::var("decl_init_kind"),
                                    Expr::var("decl_init_symbol_hash"),
                                ),
                                vec![Node::assign("declaration_initializer_prefix", Expr::u32(1))],
                            ),
                        ],
                    ),
                ],
            )],
        ),
        Node::let_bind(
            "parent_is_initializer_list_context",
            Expr::and(
                Expr::eq(Expr::var("cur_parent_kind"), Expr::u32(TOK_LBRACE)),
                Expr::or(
                    any_token_eq(
                        Expr::var("cur_parent_prev_sibling_kind"),
                        &[TOK_ASSIGN, TOK_COMMA],
                    ),
                    Expr::and(
                        Expr::eq(
                            Expr::var("cur_parent_prev_sibling_kind"),
                            Expr::u32(TOK_LBRACE),
                        ),
                        Expr::and(
                            Expr::eq(Expr::var("cur_parent_parent_kind"), Expr::u32(TOK_LBRACE)),
                            any_token_eq(
                                Expr::var("cur_grandparent_prev_sibling_kind"),
                                &[TOK_ASSIGN, TOK_COMMA, TOK_LBRACE],
                            ),
                        ),
                    ),
                ),
            ),
        ),
        Node::let_bind(
            "is_array_declaration_initializer_assign",
            Expr::and(
                Expr::and(
                    Expr::eq(Expr::var("raw_kind"), Expr::u32(TOK_ASSIGN)),
                    Expr::eq(Expr::var("prev_sibling_kind"), Expr::u32(TOK_LBRACKET)),
                ),
                Expr::and(
                    Expr::eq(Expr::var("effective_has_decl_prefix"), Expr::u32(1)),
                    Expr::and(
                        Expr::not(Expr::var("parent_is_initializer_list_context")),
                        Expr::eq(Expr::var("next_kind"), Expr::u32(TOK_STRING)),
                    ),
                ),
            ),
        ),
        Node::let_bind(
            "is_declaration_initializer_assign",
            Expr::and(
                Expr::and(
                    Expr::eq(Expr::var("raw_kind"), Expr::u32(TOK_ASSIGN)),
                    Expr::or(
                        Expr::eq(Expr::var("declaration_initializer_prefix"), Expr::u32(1)),
                        Expr::or(
                            Expr::eq(Expr::var("effective_has_decl_prefix"), Expr::u32(1)),
                            Expr::var("c99_for_init_statement_assign"),
                        ),
                    ),
                ),
                Expr::and(
                    Expr::not(Expr::var("inside_gnu_statement_expr_body")),
                    Expr::not(Expr::var("is_array_declaration_initializer_assign")),
                ),
            ),
        ),
        Node::let_bind(
            "is_pointer_decl",
            Expr::and(
                Expr::eq(Expr::var("raw_kind"), Expr::u32(TOK_STAR)),
                Expr::or(
                    Expr::eq(Expr::var("effective_has_decl_prefix"), Expr::u32(1)),
                    Expr::and(
                        Expr::and(
                            is_type_name_identifier(
                                Expr::var("prev_sibling_typedef_flags"),
                                Expr::var("fallback_has_prior_typedef"),
                            ),
                            Expr::eq(Expr::var("prev_sibling_kind"), Expr::u32(TOK_IDENTIFIER)),
                        ),
                        Expr::and(
                            Expr::eq(Expr::var("next_kind"), Expr::u32(TOK_IDENTIFIER)),
                            any_token_eq(
                                Expr::var("prev_prev_sibling_kind"),
                                &[SENTINEL, TOK_LBRACE, TOK_LPAREN, TOK_SEMICOLON, TOK_COMMA],
                            ),
                        ),
                    ),
                ),
            ),
        ),
        Node::let_bind(
            "is_array_decl",
            Expr::and(
                Expr::and(
                    Expr::eq(Expr::var("raw_kind"), Expr::u32(TOK_LBRACKET)),
                    Expr::or(
                        Expr::eq(Expr::var("prev_sibling_kind"), Expr::u32(TOK_IDENTIFIER)),
                        Expr::and(
                            Expr::eq(Expr::var("prev_sibling_kind"), Expr::u32(TOK_LPAREN)),
                            any_token_eq(
                                Expr::var("prev_sibling_first_child_kind"),
                                &[TOK_STAR, TOK_IDENTIFIER, TOK_LPAREN],
                            ),
                        ),
                    ),
                ),
                Expr::eq(Expr::var("effective_has_decl_prefix"), Expr::u32(1)),
            ),
        ),
        Node::let_bind(
            "is_array_designator_expr",
            Expr::and(
                Expr::eq(Expr::var("raw_kind"), Expr::u32(TOK_LBRACKET)),
                Expr::eq(Expr::var("next_kind"), Expr::u32(TOK_ASSIGN)),
            ),
        ),
        Node::let_bind("declarator_parent_override", Expr::var("cur_parent")),
    ]);
}
