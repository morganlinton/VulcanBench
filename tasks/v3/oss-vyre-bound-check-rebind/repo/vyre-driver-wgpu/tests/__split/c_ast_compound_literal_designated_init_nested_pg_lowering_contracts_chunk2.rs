#[test]
fn pg_lower_preserves_designated_init_with_builtin_choose_expr() {
    let fix = fixture_designated_init_with_builtin_choose_expr();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let pg = reference_ast_to_pg_nodes(&typed);

    assert_pg_preserves_row(&typed, &pg, &fix, 8, C_AST_KIND_BUILTIN_CHOOSE_EXPR);
    for idx in row_indices(&typed, C_AST_KIND_INITIALIZER_LIST) {
        assert_pg_preserves_row(&typed, &pg, &fix, idx, C_AST_KIND_INITIALIZER_LIST);
    }
}

#[test]
fn pg_lower_preserves_array_of_compound_literals() {
    let fix = fixture_array_of_compound_literals();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let pg = reference_ast_to_pg_nodes(&typed);

    for idx in row_indices(&typed, C_AST_KIND_COMPOUND_LITERAL_EXPR) {
        assert_pg_preserves_row(&typed, &pg, &fix, idx, C_AST_KIND_COMPOUND_LITERAL_EXPR);
    }
    for idx in row_indices(&typed, C_AST_KIND_INITIALIZER_LIST) {
        assert_pg_preserves_row(&typed, &pg, &fix, idx, C_AST_KIND_INITIALIZER_LIST);
    }
}

#[test]
fn pg_lower_preserves_compound_literal_in_ternary() {
    let fix = fixture_compound_literal_in_ternary();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let pg = reference_ast_to_pg_nodes(&typed);

    assert_pg_preserves_row(&typed, &pg, &fix, 6, C_AST_KIND_CONDITIONAL_EXPR);
    for idx in row_indices(&typed, C_AST_KIND_COMPOUND_LITERAL_EXPR) {
        assert_pg_preserves_row(&typed, &pg, &fix, idx, C_AST_KIND_COMPOUND_LITERAL_EXPR);
    }
}

// ---------------------------------------------------------------------------
// GPU/CPU parity contracts
// ---------------------------------------------------------------------------

#[test]
fn gpu_parity_compound_literal_nested_designated() {
    let fix = fixture_compound_literal_nested_designated();
    assert_full_pipeline_parity(&fix, "compound_literal_nested_designated");
}

#[test]
fn gpu_parity_compound_literal_inside_statement_expr() {
    let fix = fixture_compound_literal_inside_statement_expr();
    assert_full_pipeline_parity(&fix, "compound_literal_inside_statement_expr");
}

#[test]
fn gpu_parity_designated_init_with_builtin_choose_expr() {
    let fix = fixture_designated_init_with_builtin_choose_expr();
    assert_full_pipeline_parity(&fix, "designated_init_with_builtin_choose_expr");
}

#[test]
fn gpu_parity_array_of_compound_literals() {
    let fix = fixture_array_of_compound_literals();
    assert_full_pipeline_parity(&fix, "array_of_compound_literals");
}

#[test]
fn gpu_parity_compound_literal_in_ternary() {
    let fix = fixture_compound_literal_in_ternary();
    assert_full_pipeline_parity(&fix, "compound_literal_in_ternary");
}

// ---------------------------------------------------------------------------
// GPU PG lowering parity contracts
// ---------------------------------------------------------------------------

#[test]
fn gpu_parity_pg_lower_compound_literal_nested_designated() {
    let fix = fixture_compound_literal_nested_designated();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed);
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for compound_literal_nested_designated"
    );
}

#[test]
fn gpu_parity_pg_lower_compound_literal_inside_statement_expr() {
    let fix = fixture_compound_literal_inside_statement_expr();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed);
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for compound_literal_inside_statement_expr"
    );
}

#[test]
fn gpu_parity_pg_lower_designated_init_with_builtin_choose_expr() {
    let fix = fixture_designated_init_with_builtin_choose_expr();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed);
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for designated_init_with_builtin_choose_expr"
    );
}

#[test]
fn gpu_parity_pg_lower_array_of_compound_literals() {
    let fix = fixture_array_of_compound_literals();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed);
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for array_of_compound_literals"
    );
}

#[test]
fn gpu_parity_pg_lower_compound_literal_in_ternary() {
    let fix = fixture_compound_literal_in_ternary();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed);
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for compound_literal_in_ternary"
    );
}
