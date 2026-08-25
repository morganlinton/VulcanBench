#[test]
fn paren_expr_then_mul_is_binary_not_cast() {
    let fix = fixture_paren_expr_then_mul();
    assert_full_pipeline_parity(&fix, "paren_expr_then_mul");

    let typed = classify(&fix);
    assert!(
        row_indices(&typed, C_AST_KIND_CAST_EXPR).is_empty(),
        "`(x)` must NOT be classified as CAST_EXPR"
    );
    assert!(
        row_indices(&typed, node_kind::BINARY).contains(&8),
        "`*` after parenthesized expr must be BINARY"
    );
}

#[test]
fn cast_not_compound_literal_vs_paren_expr_call_like() {
    let fix_cast = fixture_cast_not_compound_literal();
    let fix_paren = fixture_paren_expr_then_call_like();

    assert_full_pipeline_parity(&fix_cast, "cast_not_compound_literal");
    assert_full_pipeline_parity(&fix_paren, "paren_expr_then_call_like");

    let typed_cast = classify(&fix_cast);
    let typed_paren = classify(&fix_paren);

    assert_eq!(
        row_indices(&typed_cast, C_AST_KIND_CAST_EXPR),
        vec![5],
        "`(int)(1)` must be CAST_EXPR"
    );
    assert!(
        row_indices(&typed_paren, C_AST_KIND_CAST_EXPR).is_empty(),
        "`(x)(1)` must NOT be CAST_EXPR"
    );
    assert!(
        row_indices(&typed_paren, node_kind::CALL).contains(&5)
            || row_indices(&typed_paren, node_kind::CALL).is_empty(),
        "`(x)(1)` is either CALL or unclassified, never CAST_EXPR"
    );
}

// ---------------------------------------------------------------------------
// Tests – nested conditional / comma
// ---------------------------------------------------------------------------

#[test]
fn nested_conditional_comma_classifies() {
    let fix = fixture_nested_conditional_comma();
    assert_full_pipeline_parity(&fix, "nested_conditional_comma");

    let typed = classify(&fix);
    let questions = row_indices(&typed, C_AST_KIND_CONDITIONAL_EXPR);
    assert_eq!(
        questions,
        vec![6, 8],
        "nested ternary must classify both `?` as CONDITIONAL_EXPR"
    );

    // Comma must not be classified as a binary operator in expression-shape.
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let expr_shape = reference_c11_build_expression_shape_nodes(&raw, &typed);
    assert_shape_none(&expr_shape, 14, TOK_COMMA);
}

// ---------------------------------------------------------------------------
// Tests – compound literal (array)
// ---------------------------------------------------------------------------

#[test]
fn array_compound_literal_classifies() {
    let fix = fixture_array_compound_literal();
    assert_full_pipeline_parity(&fix, "array_compound_literal");

    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_COMPOUND_LITERAL_EXPR),
        vec![9],
        "array compound literal introducer `(` must be COMPOUND_LITERAL_EXPR"
    );
}

// ---------------------------------------------------------------------------
// Tests – sizeof / _Alignof ambiguity
// ---------------------------------------------------------------------------

#[test]
fn sizeof_typename_then_star_preserves_sizeof() {
    let fix = fixture_sizeof_typename_then_star();
    assert_full_pipeline_parity(&fix, "sizeof_typename_then_star");

    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_CAST_EXPR).len(),
        0,
        "`(` after sizeof must NOT be confused with CAST_EXPR"
    );
    // The `*` after sizeof(int) is classified as BINARY multiply at the token
    // level (the classifier does not treat the preceding `)` as unary context).
    assert!(
        row_indices(&typed, node_kind::BINARY).contains(&9),
        "`*` after sizeof(int) must be BINARY"
    );
}

#[test]
fn alignof_typename_then_star_preserves_alignof() {
    let fix = fixture_alignof_typename_then_star();
    assert_full_pipeline_parity(&fix, "alignof_typename_then_star");

    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_CAST_EXPR).len(),
        0,
        "`(` after _Alignof must NOT be confused with CAST_EXPR"
    );
    assert!(
        row_indices(&typed, node_kind::BINARY).contains(&9),
        "`*` after _Alignof(int) must be BINARY"
    );
}

// ---------------------------------------------------------------------------
// Tests – PG lowering preservation
// ---------------------------------------------------------------------------

#[test]
fn pg_lower_preserves_member_access_rows() {
    let fix = fixture_member_access_simple();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let pg = reference_ast_to_pg_nodes(&typed);

    assert_pg_preserves_row(&typed, &pg, &fix, 6, C_AST_KIND_MEMBER_ACCESS_EXPR);
}

#[test]
fn pg_lower_preserves_ptr_member_access_rows() {
    let fix = fixture_ptr_member_access_simple();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let pg = reference_ast_to_pg_nodes(&typed);

    assert_pg_preserves_row(&typed, &pg, &fix, 6, C_AST_KIND_MEMBER_ACCESS_EXPR);
}

#[test]
fn pg_lower_preserves_cast_expr_rows() {
    let fix = fixture_cast_then_deref();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let pg = reference_ast_to_pg_nodes(&typed);

    assert_pg_preserves_row(&typed, &pg, &fix, 5, C_AST_KIND_CAST_EXPR);
}

#[test]
fn pg_lower_preserves_nested_conditional_rows() {
    let fix = fixture_nested_conditional_comma();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let pg = reference_ast_to_pg_nodes(&typed);

    assert_pg_preserves_row(&typed, &pg, &fix, 6, C_AST_KIND_CONDITIONAL_EXPR);
    assert_pg_preserves_row(&typed, &pg, &fix, 8, C_AST_KIND_CONDITIONAL_EXPR);
}

#[test]
fn pg_lower_preserves_array_compound_literal_rows() {
    let fix = fixture_array_compound_literal();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let pg = reference_ast_to_pg_nodes(&typed);

    assert_pg_preserves_row(&typed, &pg, &fix, 9, C_AST_KIND_COMPOUND_LITERAL_EXPR);
}

// ---------------------------------------------------------------------------
// Tests – GPU PG lowering parity
// ---------------------------------------------------------------------------

#[test]
fn gpu_pg_lower_matches_cpu_for_member_and_ambiguity_fixtures() {
    let fixtures: Vec<(&str, Fixture)> = vec![
        ("member_access_simple", fixture_member_access_simple()),
        (
            "ptr_member_access_simple",
            fixture_ptr_member_access_simple(),
        ),
        ("chained_member_access", fixture_chained_member_access()),
        ("cast_then_deref", fixture_cast_then_deref()),
        ("paren_expr_then_mul", fixture_paren_expr_then_mul()),
        (
            "cast_not_compound_literal",
            fixture_cast_not_compound_literal(),
        ),
        (
            "paren_expr_then_call_like",
            fixture_paren_expr_then_call_like(),
        ),
        (
            "nested_conditional_comma",
            fixture_nested_conditional_comma(),
        ),
        ("array_compound_literal", fixture_array_compound_literal()),
        (
            "sizeof_typename_then_star",
            fixture_sizeof_typename_then_star(),
        ),
        (
            "alignof_typename_then_star",
            fixture_alignof_typename_then_star(),
        ),
    ];

    for (label, fix) in fixtures {
        let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
        let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
        let typed = reference_c11_classify_vast_node_kinds(&annotated);
        let expected = reference_ast_to_pg_nodes(&typed);
        let gpu = run_gpu_pg_lower(&typed);
        assert_eq!(
            gpu, expected,
            "GPU PG lowerer must match CPU for fixture `{label}`"
        );
    }
}
