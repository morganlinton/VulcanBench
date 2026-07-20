// Integration test module for the containing Vyre package.

use super::fixtures::*;
use super::support::{
    assert_pg_links_match_vast, assert_pg_preserves_row, assert_shape_row, row_indices,
    run_pipeline, word_at, SENTINEL, VAST_STRIDE_U32,
};
use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::parse::vast::{
    C_AST_KIND_ARRAY_DECL, C_AST_KIND_ARRAY_SUBSCRIPT_EXPR, C_AST_KIND_ASSIGN_EXPR,
    C_AST_KIND_BREAK_STMT, C_AST_KIND_CASE_STMT, C_AST_KIND_CAST_EXPR,
    C_AST_KIND_MEMBER_ACCESS_EXPR, C_AST_KIND_RANGE_DESIGNATOR_EXPR, C_AST_KIND_SWITCH_STMT,
    C_AST_KIND_UNARY_EXPR, C_EXPR_ASSOC_NONE, C_EXPR_SHAPE_NONE,
};
use vyre_primitives::predicate::node_kind;

#[test]
fn prefix_unary_operators_have_unary_expr_kind_and_postfix_inc_dec_stays_unshaped() {
    let (tok_types, tok_lens) = fixture_unary_prefix_and_postfix();
    let rows = run_pipeline(&tok_types, &tok_lens);

    // Prefix ++ at 3, postfix -- at 10, prefix & at 15, prefix * at 21,
    // prefix + at 27, prefix - at 33, prefix ~ at 39, prefix ! at 45.
    let unary_indices = vec![3usize, 15, 21, 27, 33, 39, 45];
    assert_eq!(
        row_indices(&rows.typed_vast, VAST_STRIDE_U32, C_AST_KIND_UNARY_EXPR),
        unary_indices,
        "prefix unary operators must type as UNARY_EXPR while postfix -- must not"
    );

    for &idx in &unary_indices {
        assert_pg_preserves_row(&rows, idx, C_AST_KIND_UNARY_EXPR);
    }

    let postfix_dec_kind = word_at(&rows.typed_vast, 10 * VAST_STRIDE_U32);
    assert_ne!(
        postfix_dec_kind, C_AST_KIND_UNARY_EXPR,
        "postfix -- must not classify as UNARY_EXPR"
    );
    assert_ne!(
        postfix_dec_kind,
        node_kind::BINARY,
        "postfix -- must not classify as BINARY"
    );

    assert_shape_row(
        &rows.expr_shape,
        3,
        C_EXPR_SHAPE_NONE,
        TOK_INC,
        0,
        C_EXPR_ASSOC_NONE,
        SENTINEL,
        SENTINEL,
        SENTINEL,
    );
    assert_shape_row(
        &rows.expr_shape,
        10,
        C_EXPR_SHAPE_NONE,
        TOK_DEC,
        0,
        C_EXPR_ASSOC_NONE,
        SENTINEL,
        SENTINEL,
        SENTINEL,
    );
    assert_shape_row(
        &rows.expr_shape,
        15,
        C_EXPR_SHAPE_NONE,
        TOK_AMP,
        0,
        C_EXPR_ASSOC_NONE,
        SENTINEL,
        SENTINEL,
        SENTINEL,
    );
    assert_shape_row(
        &rows.expr_shape,
        21,
        C_EXPR_SHAPE_NONE,
        TOK_STAR,
        0,
        C_EXPR_ASSOC_NONE,
        SENTINEL,
        SENTINEL,
        SENTINEL,
    );
    assert_shape_row(
        &rows.expr_shape,
        27,
        C_EXPR_SHAPE_NONE,
        TOK_PLUS,
        0,
        C_EXPR_ASSOC_NONE,
        SENTINEL,
        SENTINEL,
        SENTINEL,
    );
    assert_shape_row(
        &rows.expr_shape,
        33,
        C_EXPR_SHAPE_NONE,
        TOK_MINUS,
        0,
        C_EXPR_ASSOC_NONE,
        SENTINEL,
        SENTINEL,
        SENTINEL,
    );
    assert_shape_row(
        &rows.expr_shape,
        39,
        C_EXPR_SHAPE_NONE,
        TOK_TILDE,
        0,
        C_EXPR_ASSOC_NONE,
        SENTINEL,
        SENTINEL,
        SENTINEL,
    );
    assert_shape_row(
        &rows.expr_shape,
        45,
        C_EXPR_SHAPE_NONE,
        TOK_BANG,
        0,
        C_EXPR_ASSOC_NONE,
        SENTINEL,
        SENTINEL,
        SENTINEL,
    );
}

#[test]
fn cast_expression_has_cast_expr_kind_and_shape_row() {
    let (tok_types, tok_lens) = fixture_cast_expr();
    let rows = run_pipeline(&tok_types, &tok_lens);

    assert_eq!(
        row_indices(&rows.typed_vast, VAST_STRIDE_U32, C_AST_KIND_CAST_EXPR),
        vec![3],
        "cast LPAREN must type as CAST_EXPR"
    );

    assert_pg_preserves_row(&rows, 3, C_AST_KIND_CAST_EXPR);
    assert_shape_row(
        &rows.expr_shape,
        3,
        C_EXPR_SHAPE_NONE,
        TOK_LPAREN,
        0,
        C_EXPR_ASSOC_NONE,
        SENTINEL,
        SENTINEL,
        SENTINEL,
    );
}

#[test]
fn member_access_has_member_access_expr_kind_and_shape_rows() {
    let (tok_types, tok_lens) = fixture_member_access();
    let rows = run_pipeline(&tok_types, &tok_lens);

    assert_eq!(
        row_indices(
            &rows.typed_vast,
            VAST_STRIDE_U32,
            C_AST_KIND_MEMBER_ACCESS_EXPR
        ),
        vec![4, 11],
        "dot and arrow must both type as MEMBER_ACCESS_EXPR"
    );

    assert_pg_preserves_row(&rows, 4, C_AST_KIND_MEMBER_ACCESS_EXPR);
    assert_pg_preserves_row(&rows, 11, C_AST_KIND_MEMBER_ACCESS_EXPR);

    assert_shape_row(
        &rows.expr_shape,
        4,
        C_EXPR_SHAPE_NONE,
        TOK_DOT,
        0,
        C_EXPR_ASSOC_NONE,
        SENTINEL,
        SENTINEL,
        SENTINEL,
    );
    assert_shape_row(
        &rows.expr_shape,
        11,
        C_EXPR_SHAPE_NONE,
        TOK_ARROW,
        0,
        C_EXPR_ASSOC_NONE,
        SENTINEL,
        SENTINEL,
        SENTINEL,
    );
}

#[test]
fn array_subscript_has_array_subscript_expr_kind_and_shape_row() {
    let (tok_types, tok_lens) = fixture_array_subscript();
    let rows = run_pipeline(&tok_types, &tok_lens);

    assert_eq!(
        row_indices(
            &rows.typed_vast,
            VAST_STRIDE_U32,
            C_AST_KIND_ARRAY_SUBSCRIPT_EXPR
        ),
        vec![4],
        "subscript LBRACKET must type as ARRAY_SUBSCRIPT_EXPR"
    );

    assert_pg_preserves_row(&rows, 4, C_AST_KIND_ARRAY_SUBSCRIPT_EXPR);
    assert_shape_row(
        &rows.expr_shape,
        4,
        C_EXPR_SHAPE_NONE,
        TOK_LBRACKET,
        0,
        C_EXPR_ASSOC_NONE,
        SENTINEL,
        SENTINEL,
        SENTINEL,
    );
}

#[test]
fn designated_initializer_has_explicit_typed_vast_kinds_and_shape_rows() {
    let (tok_types, tok_lens) = fixture_designated_initializer();
    let rows = run_pipeline(&tok_types, &tok_lens);

    // Dot designator .x = 1
    assert_eq!(
        row_indices(
            &rows.typed_vast,
            VAST_STRIDE_U32,
            C_AST_KIND_MEMBER_ACCESS_EXPR
        ),
        vec![5],
        "dot designator must type as MEMBER_ACCESS_EXPR"
    );
    // Array designator [0] = 2
    assert_eq!(
        row_indices(
            &rows.typed_vast,
            VAST_STRIDE_U32,
            C_AST_KIND_ARRAY_SUBSCRIPT_EXPR
        ),
        vec![10],
        "array designator must type as ARRAY_SUBSCRIPT_EXPR"
    );
    // Assignment expressions: .x = 1 and [0] = 2. The outer
    // `struct S s = { ... }` equals token is a declaration initializer.
    let assigns = row_indices(&rows.typed_vast, VAST_STRIDE_U32, C_AST_KIND_ASSIGN_EXPR);
    assert_ne!(
        word_at(&rows.typed_vast, 3 * VAST_STRIDE_U32),
        C_AST_KIND_ASSIGN_EXPR,
        "top-level declaration initializer must not type as ASSIGN_EXPR; got {assigns:?}"
    );
    assert!(
        assigns.contains(&7),
        "dot designator value must type as ASSIGN_EXPR; got {assigns:?}"
    );
    assert!(
        assigns.contains(&13),
        "array designator value must type as ASSIGN_EXPR; got {assigns:?}"
    );

    assert_pg_preserves_row(&rows, 5, C_AST_KIND_MEMBER_ACCESS_EXPR);
    assert_pg_preserves_row(&rows, 10, C_AST_KIND_ARRAY_SUBSCRIPT_EXPR);
    assert_pg_preserves_row(&rows, 7, C_AST_KIND_ASSIGN_EXPR);
    assert_pg_preserves_row(&rows, 13, C_AST_KIND_ASSIGN_EXPR);

    assert_shape_row(
        &rows.expr_shape,
        3,
        C_EXPR_SHAPE_NONE,
        TOK_ASSIGN,
        0,
        C_EXPR_ASSOC_NONE,
        SENTINEL,
        SENTINEL,
        SENTINEL,
    );
    assert_shape_row(
        &rows.expr_shape,
        5,
        C_EXPR_SHAPE_NONE,
        TOK_DOT,
        0,
        C_EXPR_ASSOC_NONE,
        SENTINEL,
        SENTINEL,
        SENTINEL,
    );
    assert_shape_row(
        &rows.expr_shape,
        10,
        C_EXPR_SHAPE_NONE,
        TOK_LBRACKET,
        0,
        C_EXPR_ASSOC_NONE,
        SENTINEL,
        SENTINEL,
        SENTINEL,
    );
}

#[test]
fn array_range_designator_has_range_designator_and_subscript_kinds() {
    let (tok_types, tok_lens) = fixture_array_range_designator();
    let rows = run_pipeline(&tok_types, &tok_lens);

    // The declarator bracket `[]` is an array decl, not a designator.
    assert_eq!(
        row_indices(&rows.typed_vast, VAST_STRIDE_U32, C_AST_KIND_ARRAY_DECL),
        vec![2],
        "declarator brackets must type as ARRAY_DECL"
    );
    // The designator bracket `[0 ... 1]` inside the initializer.
    assert_eq!(
        row_indices(
            &rows.typed_vast,
            VAST_STRIDE_U32,
            C_AST_KIND_ARRAY_SUBSCRIPT_EXPR
        ),
        vec![6],
        "range designator bracket must type as ARRAY_SUBSCRIPT_EXPR"
    );
    // The ellipsis `...` must be a range designator expression.
    assert_eq!(
        row_indices(
            &rows.typed_vast,
            VAST_STRIDE_U32,
            C_AST_KIND_RANGE_DESIGNATOR_EXPR
        ),
        vec![8],
        "ellipsis in range designator must type as RANGE_DESIGNATOR_EXPR"
    );

    assert_pg_preserves_row(&rows, 6, C_AST_KIND_ARRAY_SUBSCRIPT_EXPR);
    assert_pg_preserves_row(&rows, 8, C_AST_KIND_RANGE_DESIGNATOR_EXPR);

    assert_shape_row(
        &rows.expr_shape,
        6,
        C_EXPR_SHAPE_NONE,
        TOK_LBRACKET,
        0,
        C_EXPR_ASSOC_NONE,
        SENTINEL,
        SENTINEL,
        SENTINEL,
    );
    assert_shape_row(
        &rows.expr_shape,
        8,
        C_EXPR_SHAPE_NONE,
        TOK_ELLIPSIS,
        0,
        C_EXPR_ASSOC_NONE,
        SENTINEL,
        SENTINEL,
        SENTINEL,
    );
}

#[test]
fn gnu_case_range_has_case_stmt_and_range_designator_kinds() {
    let (tok_types, tok_lens) = fixture_gnu_case_range();
    let rows = run_pipeline(&tok_types, &tok_lens);

    assert_eq!(
        row_indices(&rows.typed_vast, VAST_STRIDE_U32, C_AST_KIND_SWITCH_STMT),
        vec![0],
        "switch must type as SWITCH_STMT"
    );
    assert_eq!(
        row_indices(&rows.typed_vast, VAST_STRIDE_U32, C_AST_KIND_CASE_STMT),
        vec![5],
        "case must type as CASE_STMT"
    );
    assert_eq!(
        row_indices(
            &rows.typed_vast,
            VAST_STRIDE_U32,
            C_AST_KIND_RANGE_DESIGNATOR_EXPR
        ),
        vec![7],
        "ellipsis in case range must type as RANGE_DESIGNATOR_EXPR"
    );
    assert_eq!(
        row_indices(&rows.typed_vast, VAST_STRIDE_U32, C_AST_KIND_BREAK_STMT),
        vec![10],
        "break must type as BREAK_STMT"
    );

    assert_pg_preserves_row(&rows, 0, C_AST_KIND_SWITCH_STMT);
    assert_pg_preserves_row(&rows, 5, C_AST_KIND_CASE_STMT);
    assert_pg_preserves_row(&rows, 7, C_AST_KIND_RANGE_DESIGNATOR_EXPR);
    assert_pg_preserves_row(&rows, 10, C_AST_KIND_BREAK_STMT);
    for idx in [0usize, 5, 7, 10] {
        assert_pg_links_match_vast(&rows, idx);
    }

    // Case statement and range designator are not expression-shape nodes,
    // but they still receive shape rows (NONE) in the buffer.
    assert_shape_row(
        &rows.expr_shape,
        5,
        C_EXPR_SHAPE_NONE,
        TOK_CASE,
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
        TOK_ELLIPSIS,
        0,
        C_EXPR_ASSOC_NONE,
        SENTINEL,
        SENTINEL,
        SENTINEL,
    );
}
