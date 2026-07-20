use super::*;

#[test]
fn comma_boundary_preserves_assignment_shapes_and_lowers_to_pg() {
    let (tok_types, tok_lens) = comma_fixture();
    let rows = run_pipeline(&tok_types, &tok_lens);

    // Assignment at index 1: a = b
    assert_shape_row(
        &rows.expr_shape,
        1,
        C_EXPR_SHAPE_BINARY,
        TOK_ASSIGN,
        2,
        C_EXPR_ASSOC_RIGHT,
        0,
        2,
        SENTINEL,
    );
    // Comma at index 3 is an expression boundary, not a shape node.
    assert_shape_row(
        &rows.expr_shape,
        3,
        C_EXPR_SHAPE_NONE,
        TOK_COMMA,
        0,
        C_EXPR_ASSOC_NONE,
        SENTINEL,
        SENTINEL,
        SENTINEL,
    );
    // Assignment at index 5: c = d
    assert_shape_row(
        &rows.expr_shape,
        5,
        C_EXPR_SHAPE_BINARY,
        TOK_ASSIGN,
        2,
        C_EXPR_ASSOC_RIGHT,
        4,
        6,
        SENTINEL,
    );
    // Comma at index 7
    assert_shape_row(
        &rows.expr_shape,
        7,
        C_EXPR_SHAPE_NONE,
        TOK_COMMA,
        0,
        C_EXPR_ASSOC_NONE,
        SENTINEL,
        SENTINEL,
        SENTINEL,
    );
    // Assignment at index 9: e = f
    assert_shape_row(
        &rows.expr_shape,
        9,
        C_EXPR_SHAPE_BINARY,
        TOK_ASSIGN,
        2,
        C_EXPR_ASSOC_RIGHT,
        8,
        10,
        SENTINEL,
    );

    assert_eq!(
        row_indices(&rows.typed_vast, VAST_STRIDE_U32, C_AST_KIND_ASSIGN_EXPR),
        vec![1, 5, 9]
    );

    for idx in [1usize, 5, 9] {
        assert_pg_preserves_row(&rows, idx, C_AST_KIND_ASSIGN_EXPR);
        assert_pg_links_match_vast(&rows, idx);
    }
}

#[test]
fn assignment_chain_right_associativity_shapes_and_lowers_to_pg() {
    let (tok_types, tok_lens) = assignment_chain_fixture();
    let rows = run_pipeline(&tok_types, &tok_lens);

    // Right-associative: a = (b = (c = d))
    assert_shape_row(
        &rows.expr_shape,
        5,
        C_EXPR_SHAPE_BINARY,
        TOK_ASSIGN,
        2,
        C_EXPR_ASSOC_RIGHT,
        4,
        6,
        SENTINEL,
    );
    assert_shape_row(
        &rows.expr_shape,
        3,
        C_EXPR_SHAPE_BINARY,
        TOK_ASSIGN,
        2,
        C_EXPR_ASSOC_RIGHT,
        2,
        5,
        SENTINEL,
    );
    assert_shape_row(
        &rows.expr_shape,
        1,
        C_EXPR_SHAPE_BINARY,
        TOK_ASSIGN,
        2,
        C_EXPR_ASSOC_RIGHT,
        0,
        3,
        SENTINEL,
    );

    assert_eq!(
        row_indices(&rows.typed_vast, VAST_STRIDE_U32, C_AST_KIND_ASSIGN_EXPR),
        vec![1, 3, 5]
    );

    for idx in [1usize, 3, 5] {
        assert_pg_preserves_row(&rows, idx, C_AST_KIND_ASSIGN_EXPR);
        assert_pg_links_match_vast(&rows, idx);
    }
}

#[test]
fn ternary_nesting_right_associativity_shapes_and_lowers_to_pg() {
    let (tok_types, tok_lens) = ternary_nesting_fixture();
    let rows = run_pipeline(&tok_types, &tok_lens);

    // Inner conditional: b ? c : d
    assert_shape_row(
        &rows.expr_shape,
        3,
        C_EXPR_SHAPE_CONDITIONAL,
        TOK_QUESTION,
        3,
        C_EXPR_ASSOC_RIGHT,
        2,
        4,
        6,
    );
    // Outer conditional: a ? (inner) : e
    assert_shape_row(
        &rows.expr_shape,
        1,
        C_EXPR_SHAPE_CONDITIONAL,
        TOK_QUESTION,
        3,
        C_EXPR_ASSOC_RIGHT,
        0,
        3,
        8,
    );
    // Colons are boundaries, not shape nodes.
    assert_shape_row(
        &rows.expr_shape,
        5,
        C_EXPR_SHAPE_NONE,
        TOK_COLON,
        0,
        C_EXPR_ASSOC_NONE,
        SENTINEL,
        SENTINEL,
        SENTINEL,
    );
    assert_shape_row(
        &rows.expr_shape,
        7,
        C_EXPR_SHAPE_NONE,
        TOK_COLON,
        0,
        C_EXPR_ASSOC_NONE,
        SENTINEL,
        SENTINEL,
        SENTINEL,
    );

    assert_eq!(
        row_indices(
            &rows.typed_vast,
            VAST_STRIDE_U32,
            C_AST_KIND_CONDITIONAL_EXPR
        ),
        vec![1, 3]
    );

    for idx in [1usize, 3] {
        assert_pg_preserves_row(&rows, idx, C_AST_KIND_CONDITIONAL_EXPR);
        assert_pg_links_match_vast(&rows, idx);
    }
}

#[test]
fn logical_and_bitwise_precedence_shapes_and_lowers_to_pg() {
    let (tok_types, tok_lens) = logical_bitwise_fixture();
    let rows = run_pipeline(&tok_types, &tok_lens);

    // Precedence ladder (tightest to loosest): * > + > < > == > & > ^ > | > && > ||
    assert_shape_row(
        &rows.expr_shape,
        17,
        C_EXPR_SHAPE_BINARY,
        TOK_STAR,
        13,
        C_EXPR_ASSOC_LEFT,
        16,
        18,
        SENTINEL,
    );
    assert_shape_row(
        &rows.expr_shape,
        15,
        C_EXPR_SHAPE_BINARY,
        TOK_PLUS,
        12,
        C_EXPR_ASSOC_LEFT,
        14,
        17,
        SENTINEL,
    );
    assert_shape_row(
        &rows.expr_shape,
        13,
        C_EXPR_SHAPE_BINARY,
        TOK_LT,
        10,
        C_EXPR_ASSOC_LEFT,
        12,
        15,
        SENTINEL,
    );
    assert_shape_row(
        &rows.expr_shape,
        11,
        C_EXPR_SHAPE_BINARY,
        TOK_EQ,
        9,
        C_EXPR_ASSOC_LEFT,
        10,
        13,
        SENTINEL,
    );
    assert_shape_row(
        &rows.expr_shape,
        9,
        C_EXPR_SHAPE_BINARY,
        TOK_AMP,
        8,
        C_EXPR_ASSOC_LEFT,
        8,
        11,
        SENTINEL,
    );
    assert_shape_row(
        &rows.expr_shape,
        7,
        C_EXPR_SHAPE_BINARY,
        TOK_CARET,
        7,
        C_EXPR_ASSOC_LEFT,
        6,
        9,
        SENTINEL,
    );
    assert_shape_row(
        &rows.expr_shape,
        5,
        C_EXPR_SHAPE_BINARY,
        TOK_PIPE,
        6,
        C_EXPR_ASSOC_LEFT,
        4,
        7,
        SENTINEL,
    );
    assert_shape_row(
        &rows.expr_shape,
        3,
        C_EXPR_SHAPE_BINARY,
        TOK_AND,
        5,
        C_EXPR_ASSOC_LEFT,
        2,
        5,
        SENTINEL,
    );
    assert_shape_row(
        &rows.expr_shape,
        1,
        C_EXPR_SHAPE_BINARY,
        TOK_OR,
        4,
        C_EXPR_ASSOC_LEFT,
        0,
        3,
        SENTINEL,
    );

    assert_eq!(
        row_indices(&rows.typed_vast, VAST_STRIDE_U32, node_kind::BINARY),
        vec![1, 3, 5, 7, 9, 11, 13, 15, 17]
    );

    for idx in [1usize, 3, 5, 7, 9, 11, 13, 15, 17] {
        assert_pg_preserves_row(&rows, idx, node_kind::BINARY);
        assert_pg_links_match_vast(&rows, idx);
    }
}

#[test]
fn cast_vs_parenthesized_expression_typing_and_pg_lower() {
    let (tok_types, tok_lens) = cast_vs_paren_fixture();
    let rows = run_pipeline(&tok_types, &tok_lens);

    // (int)a;  -> cast
    assert_eq!(
        word_at(&rows.typed_vast, 0 * VAST_STRIDE_U32),
        C_AST_KIND_CAST_EXPR,
        "Fix: (int) must classify as cast expression"
    );
    assert_shape_row(
        &rows.expr_shape,
        0,
        C_EXPR_SHAPE_NONE,
        TOK_LPAREN,
        0,
        C_EXPR_ASSOC_NONE,
        SENTINEL,
        SENTINEL,
        SENTINEL,
    );
    assert_pg_preserves_row(&rows, 0, C_AST_KIND_CAST_EXPR);

    // (b + c); -> parenthesized expression, LPAREN stays raw.
    assert_eq!(
        word_at(&rows.typed_vast, 5 * VAST_STRIDE_U32),
        0,
        "Fix: (b + c) must NOT classify as cast"
    );
    assert_shape_row(
        &rows.expr_shape,
        5,
        C_EXPR_SHAPE_NONE,
        TOK_LPAREN,
        0,
        C_EXPR_ASSOC_NONE,
        SENTINEL,
        SENTINEL,
        SENTINEL,
    );

    // Plus inside parentheses.
    assert_shape_row(
        &rows.expr_shape,
        7,
        C_EXPR_SHAPE_BINARY,
        TOK_PLUS,
        12,
        C_EXPR_ASSOC_LEFT,
        6,
        8,
        SENTINEL,
    );
    assert_pg_preserves_row(&rows, 7, node_kind::BINARY);
    assert_pg_links_match_vast(&rows, 7);
}

#[test]
fn postfix_call_index_member_shapes_and_lowers_to_pg() {
    let (tok_types, tok_lens) = postfix_fixture();
    let rows = run_pipeline(&tok_types, &tok_lens);

    // Postfix operators do not receive expression-shape nodes.
    for idx in [0usize, 6, 11, 15] {
        assert_shape_row(
            &rows.expr_shape,
            idx,
            C_EXPR_SHAPE_NONE,
            tok_types[idx],
            0,
            C_EXPR_ASSOC_NONE,
            SENTINEL,
            SENTINEL,
            SENTINEL,
        );
    }

    assert_eq!(
        row_indices(&rows.typed_vast, VAST_STRIDE_U32, node_kind::CALL),
        vec![0]
    );
    assert_eq!(
        row_indices(
            &rows.typed_vast,
            VAST_STRIDE_U32,
            C_AST_KIND_ARRAY_SUBSCRIPT_EXPR
        ),
        vec![6]
    );
    assert_eq!(
        row_indices(
            &rows.typed_vast,
            VAST_STRIDE_U32,
            C_AST_KIND_MEMBER_ACCESS_EXPR
        ),
        vec![11, 15]
    );

    for idx in [0usize, 6, 11, 15] {
        let kind = word_at(&rows.typed_vast, idx * VAST_STRIDE_U32);
        assert_pg_preserves_row(&rows, idx, kind);
        assert_pg_links_match_vast(&rows, idx);
    }
}

