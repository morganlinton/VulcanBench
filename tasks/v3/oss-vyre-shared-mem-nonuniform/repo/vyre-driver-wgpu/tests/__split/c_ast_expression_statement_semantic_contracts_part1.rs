use super::*;

#[test]
fn cast_simple_classifies_and_preserves_links() {
    let fix = fixture_cast_simple();
    assert_full_pipeline_parity(&fix, "cast_simple");

    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_CAST_EXPR),
        vec![5],
        "simple cast `(` must classify as CAST_EXPR"
    );

    // Structural invariant: CAST_EXPR (row 5) must have the type-name `int` (row 6)
    // as its first child in the raw VAST tree.
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    assert_first_child(&raw, 5, 6);
}

#[test]
fn cast_complex_with_pointer_classifies() {
    let fix = fixture_cast_complex();
    assert_full_pipeline_parity(&fix, "cast_complex");

    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_CAST_EXPR),
        vec![5],
        "complex cast `(` must classify as CAST_EXPR"
    );

    // The `*` inside the cast is a pointer declarator, not a dereference operator.
    assert_eq!(
        word_at(&typed, 8 * VAST_STRIDE_U32),
        0xC011_D001, // C_AST_KIND_POINTER_DECL
        "`*` inside cast type-name must be POINTER_DECL"
    );
}

#[test]
fn compound_literal_vs_cast_distinction() {
    let fix_lit = fixture_compound_literal_simple();
    let fix_cast = fixture_cast_not_compound();

    assert_full_pipeline_parity(&fix_lit, "compound_literal_simple");
    assert_full_pipeline_parity(&fix_cast, "cast_not_compound");

    let typed_lit = classify(&fix_lit);
    let typed_cast = classify(&fix_cast);

    assert_eq!(
        row_indices(&typed_lit, C_AST_KIND_COMPOUND_LITERAL_EXPR),
        vec![8],
        "compound literal introducer `(` must be COMPOUND_LITERAL_EXPR"
    );
    assert!(
        row_indices(&typed_lit, C_AST_KIND_INITIALIZER_LIST).contains(&11),
        "compound literal brace must classify as INITIALIZER_LIST"
    );

    assert_eq!(
        row_indices(&typed_cast, C_AST_KIND_CAST_EXPR),
        vec![8],
        "`(int)(1)` introducer `(` must be CAST_EXPR"
    );
    assert!(
        row_indices(&typed_cast, C_AST_KIND_COMPOUND_LITERAL_EXPR).is_empty(),
        "cast must NOT be confused with compound literal"
    );
}

#[test]
fn pg_lower_preserves_compound_literal_rows() {
    let fix_lit = fixture_compound_literal_simple();
    let typed_lit = classify(&fix_lit);
    let pg_lit = reference_ast_to_pg_nodes(&typed_lit);
    assert_pg_preserves_row(
        &typed_lit,
        &pg_lit,
        &fix_lit,
        8,
        C_AST_KIND_COMPOUND_LITERAL_EXPR,
    );
    assert_pg_preserves_row(
        &typed_lit,
        &pg_lit,
        &fix_lit,
        11,
        C_AST_KIND_INITIALIZER_LIST,
    );
}

// ---------------------------------------------------------------------------
// Tests – generic selection
// ---------------------------------------------------------------------------

#[test]
fn generic_selection_classifies_and_has_children() {
    let fix = fixture_generic_selection();
    assert_full_pipeline_parity(&fix, "generic_selection");

    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_GENERIC_SELECTION_EXPR),
        vec![5],
        "_Generic must classify as GENERIC_SELECTION_EXPR"
    );
    assert_ne!(
        word_at(&typed, 5 * VAST_STRIDE_U32),
        node_kind::CALL,
        "_Generic must not collapse into CALL"
    );
    assert_ne!(
        word_at(&typed, 5 * VAST_STRIDE_U32),
        node_kind::BINARY,
        "_Generic must not collapse into BINARY"
    );
}

#[test]
fn pg_lower_preserves_generic_selection() {
    let fix = fixture_generic_selection();
    let typed = classify(&fix);
    let pg = reference_ast_to_pg_nodes(&typed);
    assert_pg_preserves_row(&typed, &pg, &fix, 5, C_AST_KIND_GENERIC_SELECTION_EXPR);
}

// ---------------------------------------------------------------------------
// Tests – sizeof / alignof
// ---------------------------------------------------------------------------

#[test]
fn sizeof_typename_classifies_and_inner_paren_is_not_cast() {
    let fix = fixture_sizeof_typename();
    assert_full_pipeline_parity(&fix, "sizeof_typename");

    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_SIZEOF_EXPR),
        vec![5],
        "sizeof must classify as SIZEOF_EXPR"
    );
    assert_eq!(
        word_at(&typed, 6 * VAST_STRIDE_U32),
        0,
        "`(` after sizeof must NOT be classified as CAST_EXPR"
    );
}

#[test]
fn sizeof_expr_classifies() {
    let fix = fixture_sizeof_expr();
    assert_full_pipeline_parity(&fix, "sizeof_expr");

    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_SIZEOF_EXPR),
        vec![5],
        "sizeof expr form must classify as SIZEOF_EXPR"
    );
}

#[test]
fn alignof_typename_classifies_and_inner_paren_is_not_cast() {
    let fix = fixture_alignof_typename();
    assert_full_pipeline_parity(&fix, "alignof_typename");

    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ALIGNOF_EXPR),
        vec![5],
        "_Alignof must classify as ALIGNOF_EXPR"
    );
    assert_eq!(
        word_at(&typed, 6 * VAST_STRIDE_U32),
        0,
        "`(` after _Alignof must NOT be classified as CAST_EXPR"
    );
}

#[test]
fn pg_lower_preserves_sizeof_alignof() {
    let fix_s = fixture_sizeof_typename();
    let typed_s = classify(&fix_s);
    let pg_s = reference_ast_to_pg_nodes(&typed_s);
    assert_pg_preserves_row(&typed_s, &pg_s, &fix_s, 5, C_AST_KIND_SIZEOF_EXPR);

    let fix_a = fixture_alignof_typename();
    let typed_a = classify(&fix_a);
    let pg_a = reference_ast_to_pg_nodes(&typed_a);
    assert_pg_preserves_row(&typed_a, &pg_a, &fix_a, 5, C_AST_KIND_ALIGNOF_EXPR);
}

// ---------------------------------------------------------------------------
// Tests – conditional expressions
// ---------------------------------------------------------------------------

#[test]
fn conditional_simple_classifies_and_shape() {
    let fix = fixture_conditional_simple();
    assert_full_pipeline_parity(&fix, "conditional_simple");

    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        row_indices(&typed, C_AST_KIND_CONDITIONAL_EXPR),
        vec![6],
        "`?` must classify as CONDITIONAL_EXPR"
    );
    assert_eq!(
        word_at(&typed, 8 * VAST_STRIDE_U32),
        0,
        "`:` must NOT receive a separate expression kind"
    );
}

#[test]
fn conditional_nested_classifies_both_question_marks() {
    let fix = fixture_conditional_nested();
    assert_full_pipeline_parity(&fix, "conditional_nested");

    let typed = classify(&fix);
    let questions = row_indices(&typed, C_AST_KIND_CONDITIONAL_EXPR);
    assert_eq!(
        questions,
        vec![6, 8],
        "nested ternary must classify both `?` as CONDITIONAL_EXPR"
    );
}

#[test]
fn pg_lower_preserves_conditional_expr() {
    let fix = fixture_conditional_simple();
    let typed = classify(&fix);
    let pg = reference_ast_to_pg_nodes(&typed);
    assert_pg_preserves_row(&typed, &pg, &fix, 6, C_AST_KIND_CONDITIONAL_EXPR);
}

// ---------------------------------------------------------------------------
// Tests – comma expressions
// ---------------------------------------------------------------------------

#[test]
fn comma_in_return_is_boundary_not_binary_shape() {
    let fix = fixture_comma_in_return();
    assert_full_pipeline_parity(&fix, "comma_in_return");

    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let expr_shape = vyre_libs::parsing::c::parse::vast::reference_c11_build_expression_shape_nodes(
        &raw, &typed,
    );

    // Comma tokens must be expression boundaries (shape NONE), not binary operators.
    assert_shape_none(&expr_shape, 7, TOK_COMMA);
    assert_shape_none(&expr_shape, 9, TOK_COMMA);

    // The return token must also be a boundary.
    assert_shape_none(&expr_shape, 5, TOK_RETURN);

    // The identifiers should not be shaped either (they are leaves).
    assert_shape_none(&expr_shape, 6, TOK_IDENTIFIER);
    assert_shape_none(&expr_shape, 8, TOK_IDENTIFIER);
    assert_shape_none(&expr_shape, 10, TOK_IDENTIFIER);
}

// ---------------------------------------------------------------------------
// Tests – labels
// ---------------------------------------------------------------------------

#[test]
fn multiple_consecutive_labels_classify() {
    let fix = fixture_multiple_labels();
    assert_full_pipeline_parity(&fix, "multiple_labels");

    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_LABEL_STMT),
        vec![5, 7],
        "consecutive labels must classify as LABEL_STMT"
    );
}

#[test]
fn pg_lower_preserves_label_stmt_rows() {
    let fix = fixture_multiple_labels();
    let typed = classify(&fix);
    let pg = reference_ast_to_pg_nodes(&typed);
    for idx in [5usize, 7] {
        assert_pg_preserves_row(&typed, &pg, &fix, idx, C_AST_KIND_LABEL_STMT);
    }
}

// ---------------------------------------------------------------------------
// Tests – goto
// ---------------------------------------------------------------------------

#[test]
fn goto_forward_and_backward_classify() {
    let fix_f = fixture_goto_forward();
    let fix_b = fixture_goto_backward();

    assert_full_pipeline_parity(&fix_f, "goto_forward");
    assert_full_pipeline_parity(&fix_b, "goto_backward");

    let typed_f = classify(&fix_f);
    assert_eq!(
        row_indices(&typed_f, C_AST_KIND_GOTO_STMT),
        vec![5],
        "forward goto must classify as GOTO_STMT"
    );

    let typed_b = classify(&fix_b);
    assert_eq!(
        row_indices(&typed_b, C_AST_KIND_GOTO_STMT),
        vec![9],
        "backward goto must classify as GOTO_STMT"
    );
}

#[test]
fn pg_lower_preserves_goto_stmt_rows() {
    let fix = fixture_goto_forward();
    let typed = classify(&fix);
    let pg = reference_ast_to_pg_nodes(&typed);
    assert_pg_preserves_row(&typed, &pg, &fix, 5, C_AST_KIND_GOTO_STMT);
}

// ---------------------------------------------------------------------------
// Tests – switch / case / default
// ---------------------------------------------------------------------------

#[test]
fn switch_case_fallthrough_classifies() {
    let fix = fixture_switch_case_fallthrough();
    assert_full_pipeline_parity(&fix, "switch_case_fallthrough");

    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_SWITCH_STMT),
        vec![7],
        "switch must classify as SWITCH_STMT"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_CASE_STMT),
        vec![12, 15],
        "both case labels must classify as CASE_STMT"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_DEFAULT_STMT),
        vec![20],
        "default must classify as DEFAULT_STMT"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_BREAK_STMT),
        vec![18],
        "break must classify as BREAK_STMT"
    );
}

#[test]
fn switch_case_range_classifies_with_range_designator() {
    let fix = fixture_switch_case_range();
    assert_full_pipeline_parity(&fix, "switch_case_range");

    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_SWITCH_STMT),
        vec![7],
        "switch must classify as SWITCH_STMT"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_CASE_STMT),
        vec![12],
        "case must classify as CASE_STMT"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_RANGE_DESIGNATOR_EXPR),
        vec![14],
        "GNU case range ellipsis must classify as RANGE_DESIGNATOR_EXPR"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_BREAK_STMT),
        vec![17],
        "break must classify as BREAK_STMT"
    );
}

#[test]
fn pg_lower_preserves_switch_case_default_rows() {
    let fix = fixture_switch_case_fallthrough();
    let typed = classify(&fix);
    let pg = reference_ast_to_pg_nodes(&typed);
    assert_pg_preserves_row(&typed, &pg, &fix, 7, C_AST_KIND_SWITCH_STMT);
    assert_pg_preserves_row(&typed, &pg, &fix, 12, C_AST_KIND_CASE_STMT);
    assert_pg_preserves_row(&typed, &pg, &fix, 15, C_AST_KIND_CASE_STMT);
    assert_pg_preserves_row(&typed, &pg, &fix, 18, C_AST_KIND_BREAK_STMT);
    assert_pg_preserves_row(&typed, &pg, &fix, 20, C_AST_KIND_DEFAULT_STMT);
}

// ---------------------------------------------------------------------------
// Tests – loops
// ---------------------------------------------------------------------------

