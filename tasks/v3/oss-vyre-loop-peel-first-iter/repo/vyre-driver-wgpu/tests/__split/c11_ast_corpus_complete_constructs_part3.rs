use super::*;

#[test]
fn cpu_reference_enum_values_typed_correctly() {
    let (tok_types, tok_starts, tok_lens) = fixture_enum_values();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    let enums = typed_indices(&typed, C_AST_KIND_ENUMERATOR_DECL);
    assert_eq!(
        enums,
        vec![3, 7, 11, 13, 17],
        "all five enumerators must be typed, including implicit-value ones"
    );

    // Assignments inside enum
    let assigns = typed_indices(&typed, C_AST_KIND_ASSIGN_EXPR);
    assert!(assigns.contains(&4), "OK = 0 must be an assignment expr");
    assert!(assigns.contains(&8), "WARN = 1 must be an assignment expr");
    assert!(
        assigns.contains(&14),
        "FATAL = 255 must be an assignment expr"
    );

    for idx in [3usize, 7, 11, 13, 16] {
        assert_eq!(
            word_at(&typed, idx * VAST_STRIDE_U32 + 5),
            tok_starts[idx],
            "enumerator row {idx} must preserve source start"
        );
    }
}

#[test]
fn cpu_reference_sizeof_type_vs_expr_disambiguated() {
    let (tok_types, tok_starts, tok_lens) = fixture_sizeof_type_vs_expr();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    // sizeof(int)  -  the sizeof token itself is typed, the paren is NOT a cast
    assert_eq!(
        word_at(&typed, 9 * VAST_STRIDE_U32),
        C_AST_KIND_SIZEOF_EXPR,
        "sizeof(int) must be SIZEOF_EXPR"
    );
    assert_eq!(
        word_at(&typed, 10 * VAST_STRIDE_U32),
        0,
        "sizeof type operand paren must not be misclassified as cast"
    );

    // sizeof(a)  -  same pattern
    assert_eq!(
        word_at(&typed, 17 * VAST_STRIDE_U32),
        C_AST_KIND_SIZEOF_EXPR,
        "sizeof(a) must be SIZEOF_EXPR"
    );

    // sizeof(a + 1)  -  the plus is a binary expr inside the sizeof
    assert_eq!(
        word_at(&typed, 25 * VAST_STRIDE_U32),
        C_AST_KIND_SIZEOF_EXPR,
        "sizeof(a + 1) must be SIZEOF_EXPR"
    );
    assert_eq!(
        word_at(&typed, 28 * VAST_STRIDE_U32),
        node_kind::BINARY,
        "plus inside sizeof operand must be BINARY"
    );
}

#[test]
fn cpu_reference_stmt_expr_nesting_types_control_flow() {
    let (tok_types, tok_starts, tok_lens) = fixture_stmt_expr_nesting();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        typed_indices(&typed, C_AST_KIND_FUNCTION_DEFINITION),
        vec![1],
        "outer function with a body must be FUNCTION_DEFINITION"
    );
    assert_eq!(
        typed_indices(&typed, C_AST_KIND_RETURN_STMT),
        vec![7, 22],
        "both return statements must be RETURN_STMT"
    );
    assert_eq!(
        typed_indices(&typed, C_AST_KIND_IF_STMT),
        vec![16],
        "nested if inside expression must be IF_STMT"
    );
    assert_eq!(
        typed_indices(&typed, node_kind::BINARY),
        vec![10, 19],
        "relational operators must be BINARY"
    );
    assert_eq!(
        typed_indices(&typed, C_AST_KIND_CONDITIONAL_EXPR),
        vec![13],
        "ternary operator must be CONDITIONAL_EXPR"
    );
}

// ---------------------------------------------------------------------------
// PG lowering semantic contract tests
// ---------------------------------------------------------------------------

