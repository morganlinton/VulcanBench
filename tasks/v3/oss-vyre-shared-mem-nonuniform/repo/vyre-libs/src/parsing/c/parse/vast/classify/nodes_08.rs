use super::*;

pub(super) fn extend(
    out: &mut Vec<Node>,
    _vast_nodes: &str,
    _out_typed_vast_nodes: &str,
    _num_nodes: Expr,
    _t: Expr,
    _base: Expr,
) {
    out.extend(vec![
        Node::if_then(
            Expr::and(
                Expr::var("is_array_decl"),
                Expr::and(
                    Expr::eq(Expr::var("prev_sibling_kind"), Expr::u32(TOK_LPAREN)),
                    Expr::eq(
                        Expr::var("prev_sibling_first_child_kind"),
                        Expr::u32(TOK_STAR),
                    ),
                ),
            ),
            vec![Node::assign(
                "declarator_parent_override",
                Expr::var("prev_sibling_first_child_idx"),
            )],
        ),
        Node::let_bind(
            "is_compound_literal",
            Expr::and(
                Expr::and(Expr::var("raw_lparen"), Expr::var("type_name_paren")),
                Expr::eq(Expr::var("next_kind"), Expr::u32(TOK_LBRACE)),
            ),
        ),
        Node::let_bind(
            "is_cast_expr",
            Expr::and(
                Expr::and(Expr::var("raw_lparen"), Expr::var("type_name_paren")),
                Expr::and(
                    Expr::not(Expr::var("is_function_declarator")),
                    Expr::not(Expr::var("is_compound_literal")),
                ),
            ),
        ),
        Node::let_bind(
            "star_after_parenthesized_identifier_expr",
            Expr::and(
                Expr::and(
                    Expr::eq(Expr::var("raw_kind"), Expr::u32(TOK_STAR)),
                    Expr::eq(Expr::var("prev_sibling_kind"), Expr::u32(TOK_LPAREN)),
                ),
                Expr::and(
                    Expr::eq(
                        Expr::var("prev_sibling_first_child_kind"),
                        Expr::u32(TOK_IDENTIFIER),
                    ),
                    Expr::or(
                        Expr::and(
                            Expr::eq(Expr::var("has_typedef_annotations"), Expr::u32(1)),
                            Expr::not(is_typedef_name_annotation(Expr::var(
                                "prev_sibling_first_child_typedef_flags",
                            ))),
                        ),
                        Expr::and(
                            Expr::eq(Expr::var("has_typedef_annotations"), Expr::u32(0)),
                            Expr::or(
                                Expr::eq(Expr::var("has_prior_typedef"), Expr::u32(0)),
                                Expr::or(
                                    Expr::eq(Expr::var("has_prior_ordinary_decl"), Expr::u32(1)),
                                    Expr::eq(
                                        Expr::var("has_prior_parenthesized_identifier_statement"),
                                        Expr::u32(1),
                                    ),
                                ),
                            ),
                        ),
                    ),
                ),
            ),
        ),
        Node::let_bind(
            "brace_after_compound_literal_type",
            Expr::and(
                Expr::and(
                    Expr::eq(Expr::var("prev_sibling_kind"), Expr::u32(TOK_LPAREN)),
                    Expr::or(
                        Expr::or(
                            is_type_name_start_token(Expr::var("prev_sibling_first_child_kind")),
                            is_typeof_operator_token(
                                Expr::var("prev_sibling_first_child_kind"),
                                Expr::var("prev_sibling_first_child_symbol_hash"),
                            ),
                        ),
                        Expr::and(
                            Expr::eq(
                                Expr::var("prev_sibling_first_child_kind"),
                                Expr::u32(TOK_IDENTIFIER),
                            ),
                            is_type_name_identifier(
                                Expr::var("prev_sibling_first_child_typedef_flags"),
                                Expr::var("fallback_has_prior_typedef"),
                            ),
                        ),
                    ),
                ),
                any_token_eq(
                    Expr::var("prev_prev_sibling_kind"),
                    &[TOK_ASSIGN, TOK_RETURN, TOK_COMMA, TOK_LPAREN],
                ),
            ),
        ),
        Node::let_bind(
            "is_initializer_list",
            Expr::and(
                Expr::eq(Expr::var("raw_kind"), Expr::u32(TOK_LBRACE)),
                Expr::or(
                    Expr::eq(Expr::var("prev_sibling_kind"), Expr::u32(TOK_ASSIGN)),
                    Expr::or(
                        Expr::var("brace_after_compound_literal_type"),
                        Expr::and(
                            any_token_eq(
                                Expr::var("prev_sibling_kind"),
                                &[SENTINEL, TOK_LBRACE, TOK_COMMA],
                            ),
                            Expr::var("parent_is_initializer_list_context"),
                        ),
                    ),
                ),
            ),
        ),
        Node::let_bind(
            "field_decl_follower",
            any_token_eq(
                Expr::var("next_kind"),
                &[
                    TOK_SEMICOLON,
                    TOK_COMMA,
                    TOK_ASSIGN,
                    TOK_LBRACKET,
                    TOK_COLON,
                ],
            ),
        ),
        Node::let_bind(
            "is_field_decl",
            Expr::and(
                Expr::and(
                    Expr::eq(Expr::var("raw_kind"), Expr::u32(TOK_IDENTIFIER)),
                    Expr::var("parent_is_record_body"),
                ),
                Expr::and(
                    Expr::eq(Expr::var("has_decl_prefix"), Expr::u32(1)),
                    Expr::var("field_decl_follower"),
                ),
            ),
        ),
        Node::let_bind(
            "is_bit_field_decl",
            Expr::and(
                Expr::var("is_field_decl"),
                Expr::eq(Expr::var("next_kind"), Expr::u32(TOK_COLON)),
            ),
        ),
        Node::let_bind(
            "is_anonymous_bit_field_decl",
            Expr::and(
                Expr::and(
                    Expr::eq(Expr::var("raw_kind"), Expr::u32(TOK_COLON)),
                    Expr::var("parent_is_record_body"),
                ),
                Expr::and(
                    Expr::eq(Expr::var("has_decl_prefix"), Expr::u32(1)),
                    Expr::ne(Expr::var("prev_sibling_kind"), Expr::u32(TOK_IDENTIFIER)),
                ),
            ),
        ),
        Node::let_bind(
            "enumerator_decl_follower",
            any_token_eq(Expr::var("next_kind"), &[TOK_COMMA, TOK_ASSIGN, TOK_RBRACE]),
        ),
        Node::let_bind(
            "is_enumerator_decl",
            Expr::and(
                Expr::and(
                    Expr::eq(Expr::var("raw_kind"), Expr::u32(TOK_IDENTIFIER)),
                    Expr::var("parent_is_enum_body"),
                ),
                Expr::and(
                    Expr::or(
                        Expr::eq(Expr::var("prev_sibling_kind"), Expr::u32(SENTINEL)),
                        Expr::eq(Expr::var("prev_sibling_kind"), Expr::u32(TOK_COMMA)),
                    ),
                    Expr::var("enumerator_decl_follower"),
                ),
            ),
        ),
        Node::let_bind(
            "is_label_stmt",
            Expr::and(
                Expr::and(
                    Expr::eq(Expr::var("raw_kind"), Expr::u32(TOK_IDENTIFIER)),
                    Expr::eq(Expr::var("next_kind"), Expr::u32(TOK_COLON)),
                ),
                Expr::and(
                    Expr::not(Expr::var("parent_is_record_body")),
                    Expr::not(Expr::var("parent_is_enum_body")),
                ),
            ),
        ),
        Node::let_bind(
            "is_gnu_statement_expr",
            Expr::and(
                Expr::eq(Expr::var("raw_kind"), Expr::u32(TOK_LPAREN)),
                Expr::eq(Expr::var("first_child_kind"), Expr::u32(TOK_LBRACE)),
            ),
        ),
        Node::let_bind(
            "asm_prefix_before_current",
            Expr::or(
                Expr::eq(Expr::var("prev_sibling_kind"), Expr::u32(TOK_GNU_ASM)),
                Expr::and(
                    any_token_eq(Expr::var("prev_sibling_kind"), &[TOK_VOLATILE, TOK_GOTO]),
                    Expr::eq(Expr::var("prev_prev_sibling_kind"), Expr::u32(TOK_GNU_ASM)),
                ),
            ),
        ),
        Node::let_bind(
            "asm_prefix_before_parent",
            Expr::or(
                Expr::eq(
                    Expr::var("cur_parent_prev_sibling_kind"),
                    Expr::u32(TOK_GNU_ASM),
                ),
                Expr::or(
                    Expr::and(
                        any_token_eq(
                            Expr::var("cur_parent_prev_sibling_kind"),
                            &[TOK_VOLATILE, TOK_GOTO],
                        ),
                        Expr::eq(
                            Expr::var("cur_parent_prev_prev_sibling_kind"),
                            Expr::u32(TOK_GNU_ASM),
                        ),
                    ),
                    Expr::and(
                        Expr::eq(
                            Expr::var("cur_parent_prev_sibling_kind"),
                            Expr::u32(TOK_GOTO),
                        ),
                        Expr::eq(
                            Expr::var("cur_parent_prev_prev_sibling_kind"),
                            Expr::u32(TOK_VOLATILE),
                        ),
                    ),
                ),
            ),
        ),
        Node::let_bind(
            "is_asm_goto_qualifier",
            Expr::and(
                Expr::eq(Expr::var("raw_kind"), Expr::u32(TOK_GOTO)),
                Expr::var("asm_prefix_before_current"),
            ),
        ),
        Node::let_bind(
            "is_asm_volatile_qualifier",
            Expr::and(
                Expr::eq(Expr::var("raw_kind"), Expr::u32(TOK_VOLATILE)),
                Expr::var("asm_prefix_before_current"),
            ),
        ),
        Node::let_bind(
            "asm_paren_context",
            Expr::and(
                Expr::eq(Expr::var("cur_parent_kind"), Expr::u32(TOK_LPAREN)),
                Expr::var("asm_prefix_before_parent"),
            ),
        ),
        Node::let_bind(
            "is_asm_template",
            Expr::and(
                Expr::and(
                    Expr::eq(Expr::var("raw_kind"), Expr::u32(TOK_STRING)),
                    Expr::var("asm_paren_context"),
                ),
                Expr::eq(Expr::var("colon_count_before"), Expr::u32(0)),
            ),
        ),
        Node::let_bind(
            "is_asm_output_operand",
            Expr::and(
                Expr::and(
                    Expr::eq(Expr::var("raw_kind"), Expr::u32(TOK_LPAREN)),
                    Expr::var("asm_paren_context"),
                ),
                Expr::and(
                    Expr::eq(Expr::var("prev_sibling_kind"), Expr::u32(TOK_STRING)),
                    Expr::eq(Expr::var("colon_count_before"), Expr::u32(1)),
                ),
            ),
        ),
        Node::let_bind(
            "is_asm_input_operand",
            Expr::and(
                Expr::and(
                    Expr::eq(Expr::var("raw_kind"), Expr::u32(TOK_LPAREN)),
                    Expr::var("asm_paren_context"),
                ),
                Expr::and(
                    Expr::eq(Expr::var("prev_sibling_kind"), Expr::u32(TOK_STRING)),
                    Expr::eq(Expr::var("colon_count_before"), Expr::u32(2)),
                ),
            ),
        ),
        Node::let_bind(
            "is_asm_clobbers_list",
            Expr::and(
                Expr::and(
                    Expr::eq(Expr::var("raw_kind"), Expr::u32(TOK_STRING)),
                    Expr::var("asm_paren_context"),
                ),
                Expr::ge(Expr::var("colon_count_before"), Expr::u32(3)),
            ),
        ),
        Node::let_bind(
            "is_asm_goto_label",
            Expr::and(
                Expr::and(
                    Expr::eq(Expr::var("raw_kind"), Expr::u32(TOK_IDENTIFIER)),
                    Expr::var("asm_paren_context"),
                ),
                Expr::and(
                    Expr::ge(Expr::var("colon_count_before"), Expr::u32(4)),
                    Expr::or(
                        Expr::eq(
                            Expr::var("cur_parent_prev_sibling_kind"),
                            Expr::u32(TOK_GOTO),
                        ),
                        Expr::eq(
                            Expr::var("cur_parent_prev_prev_sibling_kind"),
                            Expr::u32(TOK_GOTO),
                        ),
                    ),
                ),
            ),
        ),
    ]);
}
