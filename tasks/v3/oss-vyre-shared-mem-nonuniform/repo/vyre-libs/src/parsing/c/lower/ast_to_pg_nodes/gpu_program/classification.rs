use super::*;

pub(super) fn expr_is_kind(kind: Expr, expected: u32) -> Expr {
    Expr::eq(kind, Expr::u32(expected))
}

pub(super) fn kind_is(expected: u32) -> Expr {
    expr_is_kind(Expr::var("kind"), expected)
}

pub(super) fn kind_in_range(start: u32, end: u32) -> Expr {
    Expr::and(
        Expr::ge(Expr::var("kind"), Expr::u32(start)),
        Expr::le(Expr::var("kind"), Expr::u32(end)),
    )
}

pub(super) fn kind_in_set(kinds: &[u32]) -> Expr {
    kinds
        .iter()
        .copied()
        .map(kind_is)
        .reduce(Expr::or)
        .unwrap_or_else(|| Expr::bool(false))
}

pub(super) fn category_lookup_expr() -> Expr {
    let is_control = Expr::or(
        kind_in_range(C_AST_KIND_IF_STMT, C_AST_KIND_GOTO_STMT),
        kind_in_set(&[C_AST_KIND_LABEL_STMT, C_AST_KIND_BUILTIN_UNREACHABLE_STMT]),
    );
    let is_expression = Expr::or(
        kind_in_range(C_AST_KIND_ASSIGN_EXPR, C_AST_KIND_ARRAY_SUBSCRIPT_EXPR),
        Expr::or(
            kind_in_set(&[
                C_AST_KIND_GNU_STATEMENT_EXPR,
                C_AST_KIND_ALIGNOF_EXPR,
                C_AST_KIND_GENERIC_SELECTION_EXPR,
                C_AST_KIND_RANGE_DESIGNATOR_EXPR,
                C_AST_KIND_CAST_EXPR,
                C_AST_KIND_COMPOUND_LITERAL_EXPR,
                C_AST_KIND_INITIALIZER_LIST,
            ]),
            Expr::bool(false),
        ),
    );
    let is_declaration = Expr::or(
        kind_in_range(C_AST_KIND_POINTER_DECL, C_AST_KIND_FUNCTION_DECLARATOR),
        Expr::or(
            kind_in_range(C_AST_KIND_STRUCT_DECL, C_AST_KIND_STATIC_ASSERT_DECL),
            kind_in_set(&[
                C_AST_KIND_FIELD_DECL,
                C_AST_KIND_ENUMERATOR_DECL,
                vyre_primitives::predicate::node_kind::FUNCTION_DECL,
            ]),
        ),
    );
    let is_gnu = Expr::or(
        kind_in_range(C_AST_KIND_INLINE_ASM, C_AST_KIND_ASM_QUALIFIER),
        Expr::or(
            kind_in_range(C_AST_KIND_GNU_ATTRIBUTE, C_AST_KIND_ATTRIBUTE_FALLTHROUGH),
            Expr::or(
                kind_is(C_AST_KIND_GNU_LABEL_ADDRESS_EXPR),
                Expr::or(
                    kind_in_range(
                        C_AST_KIND_BUILTIN_CONSTANT_P_EXPR,
                        C_AST_KIND_BUILTIN_TYPES_COMPATIBLE_P_EXPR,
                    ),
                    Expr::or(
                        kind_in_range(
                            C_AST_KIND_BUILTIN_EXPECT_EXPR,
                            C_AST_KIND_BUILTIN_PREFETCH_EXPR,
                        ),
                        kind_in_range(
                            C_AST_KIND_BUILTIN_OVERFLOW_EXPR,
                            C_AST_KIND_BUILTIN_CLASSIFY_TYPE_EXPR,
                        ),
                    ),
                ),
            ),
        ),
    );

    Expr::select(
        is_gnu,
        Expr::u32(C_AST_PG_CATEGORY_GNU),
        Expr::select(
            is_declaration,
            Expr::u32(C_AST_PG_CATEGORY_DECLARATION),
            Expr::select(
                is_expression,
                Expr::u32(C_AST_PG_CATEGORY_EXPRESSION),
                Expr::select(
                    is_control,
                    Expr::u32(C_AST_PG_CATEGORY_CONTROL),
                    Expr::u32(C_AST_PG_CATEGORY_NONE),
                ),
            ),
        ),
    )
}

pub(super) fn role_lookup_expr() -> Expr {
    let expression_role = Expr::or(
        kind_in_set(&[
            C_AST_KIND_CAST_EXPR,
            C_AST_KIND_COMPOUND_LITERAL_EXPR,
            C_AST_KIND_SIZEOF_EXPR,
            C_AST_KIND_CONDITIONAL_EXPR,
            C_AST_KIND_UNARY_EXPR,
            C_AST_KIND_GENERIC_SELECTION_EXPR,
            C_AST_KIND_GNU_LABEL_ADDRESS_EXPR,
        ]),
        Expr::or(
            kind_in_range(
                C_AST_KIND_BUILTIN_CONSTANT_P_EXPR,
                C_AST_KIND_BUILTIN_TYPES_COMPATIBLE_P_EXPR,
            ),
            Expr::or(
                kind_in_range(
                    C_AST_KIND_BUILTIN_EXPECT_EXPR,
                    C_AST_KIND_BUILTIN_PREFETCH_EXPR,
                ),
                kind_in_range(
                    C_AST_KIND_BUILTIN_OVERFLOW_EXPR,
                    C_AST_KIND_BUILTIN_CLASSIFY_TYPE_EXPR,
                ),
            ),
        ),
    );

    let mut role = Expr::select(
        expression_role,
        Expr::u32(C_AST_PG_ROLE_EXPRESSION),
        Expr::u32(C_AST_PG_ROLE_NONE),
    );
    for (condition, value) in [
        (
            kind_in_range(
                C_AST_KIND_ATTRIBUTE_SECTION,
                C_AST_KIND_ATTRIBUTE_FALLTHROUGH,
            ),
            C_AST_PG_ROLE_GNU_ATTRIBUTE_DETAIL,
        ),
        (
            kind_in_set(&[C_AST_KIND_IF_STMT, C_AST_KIND_ELSE_STMT]),
            C_AST_PG_ROLE_SELECTION,
        ),
        (
            kind_in_set(&[
                C_AST_KIND_FOR_STMT,
                C_AST_KIND_WHILE_STMT,
                C_AST_KIND_DO_STMT,
            ]),
            C_AST_PG_ROLE_LOOP,
        ),
        (
            kind_in_set(&[
                C_AST_KIND_STRUCT_DECL,
                C_AST_KIND_UNION_DECL,
                C_AST_KIND_ENUM_DECL,
            ]),
            C_AST_PG_ROLE_AGGREGATE_DECL,
        ),
    ] {
        role = Expr::select(condition, Expr::u32(value), role);
    }
    for (kind, value) in [
        (C_AST_KIND_LABEL_STMT, C_AST_PG_ROLE_LABEL),
        (C_AST_KIND_CASE_STMT, C_AST_PG_ROLE_CASE),
        (C_AST_KIND_DEFAULT_STMT, C_AST_PG_ROLE_DEFAULT),
        (C_AST_KIND_GOTO_STMT, C_AST_PG_ROLE_GOTO),
        (C_AST_KIND_SWITCH_STMT, C_AST_PG_ROLE_SWITCH),
        (C_AST_KIND_RETURN_STMT, C_AST_PG_ROLE_RETURN),
        (C_AST_KIND_BREAK_STMT, C_AST_PG_ROLE_BREAK),
        (C_AST_KIND_CONTINUE_STMT, C_AST_PG_ROLE_CONTINUE),
        (
            C_AST_KIND_BUILTIN_UNREACHABLE_STMT,
            C_AST_PG_ROLE_UNREACHABLE,
        ),
        (C_AST_KIND_GNU_STATEMENT_EXPR, C_AST_PG_ROLE_STATEMENT_EXPR),
        (C_AST_KIND_INLINE_ASM, C_AST_PG_ROLE_INLINE_ASM),
        (C_AST_KIND_ASM_TEMPLATE, C_AST_PG_ROLE_ASM_TEMPLATE),
        (C_AST_KIND_ASM_OUTPUT_OPERAND, C_AST_PG_ROLE_ASM_OUTPUT),
        (C_AST_KIND_ASM_INPUT_OPERAND, C_AST_PG_ROLE_ASM_INPUT),
        (C_AST_KIND_ASM_CLOBBERS_LIST, C_AST_PG_ROLE_ASM_CLOBBER),
        (C_AST_KIND_ASM_GOTO_LABELS, C_AST_PG_ROLE_ASM_GOTO_LABEL),
        (C_AST_KIND_ASM_QUALIFIER, C_AST_PG_ROLE_ASM_QUALIFIER),
        (C_AST_KIND_GNU_ATTRIBUTE, C_AST_PG_ROLE_GNU_ATTRIBUTE),
        (C_AST_KIND_INITIALIZER_LIST, C_AST_PG_ROLE_INITIALIZER_LIST),
        (
            C_AST_KIND_MEMBER_ACCESS_EXPR,
            C_AST_PG_ROLE_FIELD_DESIGNATOR_OR_MEMBER_ACCESS,
        ),
        (
            C_AST_KIND_ARRAY_SUBSCRIPT_EXPR,
            C_AST_PG_ROLE_ARRAY_DESIGNATOR_OR_SUBSCRIPT,
        ),
        (
            C_AST_KIND_RANGE_DESIGNATOR_EXPR,
            C_AST_PG_ROLE_RANGE_DESIGNATOR,
        ),
        (C_AST_KIND_ASSIGN_EXPR, C_AST_PG_ROLE_ASSIGNMENT),
        (
            C_AST_KIND_FUNCTION_DEFINITION,
            C_AST_PG_ROLE_FUNCTION_DEFINITION,
        ),
        (
            C_AST_KIND_FUNCTION_DECLARATOR,
            C_AST_PG_ROLE_FUNCTION_DECLARATOR,
        ),
        (C_AST_KIND_FIELD_DECL, C_AST_PG_ROLE_FIELD_DECL),
        (C_AST_KIND_TYPEDEF_DECL, C_AST_PG_ROLE_TYPEDEF_DECL),
        (C_AST_KIND_ENUMERATOR_DECL, C_AST_PG_ROLE_ENUMERATOR_DECL),
        (C_AST_KIND_POINTER_DECL, C_AST_PG_ROLE_POINTER_DECL),
        (C_AST_KIND_ARRAY_DECL, C_AST_PG_ROLE_ARRAY_DECL),
        (C_AST_KIND_BIT_FIELD_DECL, C_AST_PG_ROLE_BIT_FIELD_DECL),
        (
            C_AST_KIND_STATIC_ASSERT_DECL,
            C_AST_PG_ROLE_STATIC_ASSERT_DECL,
        ),
        (
            vyre_primitives::predicate::node_kind::FUNCTION_DECL,
            C_AST_PG_ROLE_DECLARATION,
        ),
        (C_AST_KIND_ALIGNOF_EXPR, C_AST_PG_ROLE_ALIGNOF),
        (C_AST_KIND_GNU_LOCAL_LABEL_DECL, C_AST_PG_ROLE_DECLARATION),
    ] {
        role = Expr::select(kind_is(kind), Expr::u32(value), role);
    }
    role
}

pub(super) fn semantic_classification_nodes() -> Vec<Node> {
    let mut nodes = vec![
        Node::let_bind("semantic_category", category_lookup_expr()),
        Node::let_bind("semantic_role", role_lookup_expr()),
    ];
    nodes.push(Node::if_then(
        Expr::and(
            expr_is_kind(Expr::var("kind"), C_AST_KIND_POINTER_DECL),
            Expr::or(
                expr_is_kind(Expr::var("parent_kind"), C_AST_KIND_FUNCTION_DECLARATOR),
                Expr::or(
                    expr_is_kind(
                        Expr::var("first_child_kind"),
                        C_AST_KIND_FUNCTION_DECLARATOR,
                    ),
                    expr_is_kind(
                        Expr::var("next_sibling_kind"),
                        C_AST_KIND_FUNCTION_DECLARATOR,
                    ),
                ),
            ),
        ),
        vec![Node::assign(
            "semantic_role",
            Expr::u32(C_AST_PG_ROLE_FUNCTION_POINTER_DECL),
        )],
    ));
    nodes
}
