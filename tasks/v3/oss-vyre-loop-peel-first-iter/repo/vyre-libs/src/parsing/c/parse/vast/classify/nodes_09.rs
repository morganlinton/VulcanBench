use super::*;

pub(super) fn extend(
    out: &mut Vec<Node>,
    _vast_nodes: &str,
    out_typed_vast_nodes: &str,
    _num_nodes: Expr,
    _t: Expr,
    base: Expr,
) {
    out.extend(vec![
        Node::let_bind(
            "attribute_name_context",
            Expr::and(
                Expr::and(
                    any_token_eq(Expr::var("raw_kind"), &[TOK_IDENTIFIER, TOK_CONST]),
                    Expr::eq(Expr::var("cur_parent_kind"), Expr::u32(TOK_LPAREN)),
                ),
                Expr::and(
                    Expr::eq(Expr::var("cur_parent_parent_kind"), Expr::u32(TOK_LPAREN)),
                    Expr::or(
                        Expr::eq(
                            Expr::var("cur_grandparent_prev_sibling_kind"),
                            Expr::u32(TOK_GNU_ATTRIBUTE),
                        ),
                        Expr::eq(
                            Expr::var("cur_parent_parent_prev_adjacent_kind"),
                            Expr::u32(TOK_GNU_ATTRIBUTE),
                        ),
                    ),
                ),
            ),
        ),
        Node::let_bind(
            "attribute_kind",
            Expr::select(
                Expr::var("attribute_name_context"),
                Expr::select(
                    Expr::eq(Expr::var("raw_kind"), Expr::u32(TOK_CONST)),
                    Expr::u32(C_AST_KIND_ATTRIBUTE_CONST),
                    c_attribute_kind_from_hash(Expr::var("current_symbol_hash")),
                ),
                Expr::u32(0),
            ),
        ),
        Node::let_bind(
            "direct_attribute_kind",
            Expr::select(
                Expr::and(
                    Expr::and(
                        any_token_eq(Expr::var("raw_kind"), &[TOK_IDENTIFIER, TOK_CONST]),
                        Expr::eq(Expr::var("cur_parent_kind"), Expr::u32(TOK_LPAREN)),
                    ),
                    Expr::and(
                        Expr::eq(Expr::var("cur_parent_parent_kind"), Expr::u32(TOK_LPAREN)),
                        Expr::eq(
                            Expr::var("cur_parent_parent_prev_adjacent_kind"),
                            Expr::u32(TOK_GNU_ATTRIBUTE),
                        ),
                    ),
                ),
                Expr::select(
                    Expr::eq(Expr::var("raw_kind"), Expr::u32(TOK_CONST)),
                    Expr::u32(C_AST_KIND_ATTRIBUTE_CONST),
                    c_attribute_kind_from_hash(Expr::var("current_symbol_hash")),
                ),
                Expr::u32(0),
            ),
        ),
        Node::let_bind(
            "statement_kind",
            Expr::select(
                Expr::var("is_asm_goto_qualifier"),
                Expr::u32(0),
                c_statement_kind(Expr::var("raw_kind")),
            ),
        ),
        Node::let_bind(
            "expression_kind",
            Expr::select(
                Expr::var("is_declaration_initializer_assign"),
                Expr::u32(0),
                c_expression_operator_kind(
                    Expr::var("raw_kind"),
                    Expr::var("prev_sibling_kind"),
                    Expr::var("prev_prev_sibling_kind"),
                ),
            ),
        ),
        Node::let_bind("builtin_expression_kind", {
            let token_kind = c_builtin_expression_kind(Expr::var("raw_kind"));
            Expr::select(
                Expr::ne(token_kind.clone(), Expr::u32(0)),
                token_kind,
                c_builtin_identifier_expression_kind(
                    Expr::var("raw_kind"),
                    Expr::var("current_symbol_hash"),
                    Expr::var("next_kind"),
                ),
            )
        }),
        Node::let_bind(
            "is_gnu_label_address_expr",
            Expr::and(
                Expr::and(
                    Expr::eq(Expr::var("raw_kind"), Expr::u32(TOK_AND)),
                    c_unary_context(Expr::var("prev_sibling_kind")),
                ),
                Expr::eq(Expr::var("next_kind"), Expr::u32(TOK_IDENTIFIER)),
            ),
        ),
        Node::let_bind("typed_kind", {
            let mut kind = Expr::u32(0);
            kind = Expr::select(
                Expr::and(
                    Expr::eq(Expr::var("raw_kind"), Expr::u32(TOK_IDENTIFIER)),
                    Expr::not(is_gnu_auto_type_symbol_hash(Expr::var(
                        "current_symbol_hash",
                    ))),
                ),
                Expr::u32(node_kind::VARIABLE),
                kind,
            );
            kind = Expr::select(
                is_c_literal_token(Expr::var("raw_kind")),
                Expr::u32(node_kind::LITERAL),
                kind,
            );
            kind = Expr::select(
                Expr::eq(Expr::var("raw_kind"), Expr::u32(TOK_GNU_ATTRIBUTE)),
                Expr::u32(C_AST_KIND_GNU_ATTRIBUTE),
                kind,
            );
            kind = Expr::select(
                Expr::eq(Expr::var("raw_kind"), Expr::u32(TOK_GNU_ASM)),
                Expr::u32(C_AST_KIND_INLINE_ASM),
                kind,
            );
            kind = Expr::select(
                Expr::ne(Expr::var("expression_kind"), Expr::u32(0)),
                Expr::var("expression_kind"),
                kind,
            );
            kind = Expr::select(
                Expr::ne(Expr::var("builtin_expression_kind"), Expr::u32(0)),
                Expr::var("builtin_expression_kind"),
                kind,
            );
            kind = Expr::select(
                Expr::ne(Expr::var("attribute_kind"), Expr::u32(0)),
                Expr::var("attribute_kind"),
                kind,
            );
            kind = Expr::select(
                Expr::var("is_asm_goto_label"),
                Expr::u32(C_AST_KIND_ASM_GOTO_LABELS),
                kind,
            );
            kind = Expr::select(
                Expr::var("is_asm_clobbers_list"),
                Expr::u32(C_AST_KIND_ASM_CLOBBERS_LIST),
                kind,
            );
            kind = Expr::select(
                Expr::var("is_asm_input_operand"),
                Expr::u32(C_AST_KIND_ASM_INPUT_OPERAND),
                kind,
            );
            kind = Expr::select(
                Expr::var("is_asm_output_operand"),
                Expr::u32(C_AST_KIND_ASM_OUTPUT_OPERAND),
                kind,
            );
            kind = Expr::select(
                Expr::var("is_asm_template"),
                Expr::u32(C_AST_KIND_ASM_TEMPLATE),
                kind,
            );
            kind = Expr::select(
                Expr::var("is_gnu_label_address_expr"),
                Expr::u32(C_AST_KIND_GNU_LABEL_ADDRESS_EXPR),
                kind,
            );
            kind = Expr::select(
                Expr::var("star_after_parenthesized_identifier_expr"),
                Expr::u32(node_kind::BINARY),
                kind,
            );
            kind = Expr::select(
                Expr::ne(Expr::var("statement_kind"), Expr::u32(0)),
                Expr::var("statement_kind"),
                kind,
            );
            kind = Expr::select(
                Expr::eq(Expr::var("raw_kind"), Expr::u32(TOK_LBRACE)),
                Expr::u32(node_kind::BASIC_BLOCK),
                kind,
            );
            kind = Expr::select(
                Expr::ne(Expr::var("direct_attribute_kind"), Expr::u32(0)),
                Expr::var("direct_attribute_kind"),
                kind,
            );
            kind = Expr::select(Expr::var("is_call"), Expr::u32(node_kind::CALL), kind);
            kind = Expr::select(
                Expr::ne(Expr::var("attribute_kind"), Expr::u32(0)),
                Expr::var("attribute_kind"),
                kind,
            );
            kind = Expr::select(
                Expr::var("is_enumerator_decl"),
                Expr::u32(C_AST_KIND_ENUMERATOR_DECL),
                kind,
            );
            kind = Expr::select(
                Expr::var("is_field_decl"),
                Expr::u32(C_AST_KIND_FIELD_DECL),
                kind,
            );
            kind = Expr::select(
                Expr::var("is_initializer_list"),
                Expr::u32(C_AST_KIND_INITIALIZER_LIST),
                kind,
            );
            kind = Expr::select(
                Expr::var("is_compound_literal"),
                Expr::u32(C_AST_KIND_COMPOUND_LITERAL_EXPR),
                kind,
            );
            kind = Expr::select(
                Expr::var("is_cast_expr"),
                Expr::u32(C_AST_KIND_CAST_EXPR),
                kind,
            );
            kind = Expr::select(
                Expr::var("is_array_designator_expr"),
                Expr::u32(C_AST_KIND_ARRAY_SUBSCRIPT_EXPR),
                kind,
            );
            kind = Expr::select(
                Expr::var("is_array_decl"),
                Expr::u32(C_AST_KIND_ARRAY_DECL),
                kind,
            );
            kind = Expr::select(
                Expr::var("is_pointer_decl"),
                Expr::u32(C_AST_KIND_POINTER_DECL),
                kind,
            );
            kind = Expr::select(
                Expr::var("is_function_declarator"),
                Expr::u32(C_AST_KIND_FUNCTION_DECLARATOR),
                kind,
            );
            Expr::select(
                Expr::var("is_function_decl"),
                Expr::u32(node_kind::FUNCTION_DECL),
                kind,
            )
        }),
        Node::let_bind(
            "final_typed_kind",
            Expr::select(
                Expr::var("is_function_definition"),
                Expr::u32(C_AST_KIND_FUNCTION_DEFINITION),
                Expr::select(
                    Expr::ne(Expr::var("aggregate_decl_kind"), Expr::u32(0)),
                    Expr::var("aggregate_decl_kind"),
                    Expr::select(
                        Expr::var("is_typedef_decl"),
                        Expr::u32(C_AST_KIND_TYPEDEF_DECL),
                        Expr::select(
                            Expr::var("is_static_assert_decl"),
                            Expr::u32(C_AST_KIND_STATIC_ASSERT_DECL),
                            Expr::select(
                                Expr::eq(Expr::var("raw_kind"), Expr::u32(TOK_GNU_LABEL)),
                                Expr::u32(C_AST_KIND_GNU_LOCAL_LABEL_DECL),
                                Expr::select(
                                    Expr::or(
                                        Expr::var("is_bit_field_decl"),
                                        Expr::var("is_anonymous_bit_field_decl"),
                                    ),
                                    Expr::u32(C_AST_KIND_BIT_FIELD_DECL),
                                    Expr::select(
                                        Expr::var("is_label_stmt"),
                                        Expr::u32(C_AST_KIND_LABEL_STMT),
                                        Expr::select(
                                            Expr::var("is_gnu_statement_expr"),
                                            Expr::u32(C_AST_KIND_GNU_STATEMENT_EXPR),
                                            Expr::select(
                                                Expr::or(
                                                    Expr::var("is_asm_goto_qualifier"),
                                                    Expr::var("is_asm_volatile_qualifier"),
                                                ),
                                                Expr::u32(C_AST_KIND_ASM_QUALIFIER),
                                                Expr::select(
                                                    Expr::ne(
                                                        Expr::var("builtin_expression_kind"),
                                                        Expr::u32(0),
                                                    ),
                                                    Expr::var("builtin_expression_kind"),
                                                    Expr::var("typed_kind"),
                                                ),
                                            ),
                                        ),
                                    ),
                                ),
                            ),
                        ),
                    ),
                ),
            ),
        ),
        Node::if_then(
            Expr::ne(Expr::var("direct_attribute_kind"), Expr::u32(0)),
            vec![Node::assign(
                "final_typed_kind",
                Expr::var("direct_attribute_kind"),
            )],
        ),
        Node::store(
            out_typed_vast_nodes,
            base.clone(),
            Expr::var("final_typed_kind"),
        ),
    ]);
}
