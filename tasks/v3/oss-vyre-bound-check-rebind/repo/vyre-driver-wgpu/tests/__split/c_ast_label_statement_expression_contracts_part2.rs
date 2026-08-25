use super::*;

#[test]
fn pg_lower_preserves_gnu_statement_expr_kinds() {
    let fix = fixture_statement_expression_simple();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let pg = reference_ast_to_pg_nodes(&typed);

    assert_pg_preserves_row(
        &typed,
        &pg,
        &fix.tok_starts,
        &fix.tok_lens,
        3,
        C_AST_KIND_GNU_STATEMENT_EXPR,
    );
    assert_pg_preserves_row(
        &typed,
        &pg,
        &fix.tok_starts,
        &fix.tok_lens,
        7,
        C_AST_KIND_ASSIGN_EXPR,
    );
}

#[test]
fn pg_lower_preserves_statement_expr_in_initializer_kinds() {
    let fix = fixture_statement_expression_in_array_init();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let pg = reference_ast_to_pg_nodes(&typed);

    for idx in [7usize, 14] {
        assert_pg_preserves_row(
            &typed,
            &pg,
            &fix.tok_starts,
            &fix.tok_lens,
            idx,
            C_AST_KIND_GNU_STATEMENT_EXPR,
        );
    }
    assert_pg_preserves_row(
        &typed,
        &pg,
        &fix.tok_starts,
        &fix.tok_lens,
        6,
        C_AST_KIND_INITIALIZER_LIST,
    );
}

#[test]
fn pg_lower_preserves_label_and_goto_in_nested_contexts() {
    let fix = fixture_label_inside_if_else();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let pg = reference_ast_to_pg_nodes(&typed);

    for idx in [12usize, 19] {
        assert_pg_preserves_row(
            &typed,
            &pg,
            &fix.tok_starts,
            &fix.tok_lens,
            idx,
            C_AST_KIND_LABEL_STMT,
        );
    }
    for idx in [14usize, 21] {
        assert_pg_preserves_row(
            &typed,
            &pg,
            &fix.tok_starts,
            &fix.tok_lens,
            idx,
            C_AST_KIND_RETURN_STMT,
        );
    }
}

#[test]
fn pg_lower_preserves_statement_expr_with_label_and_goto() {
    let fix = fixture_statement_expression_with_label_and_goto();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let pg = reference_ast_to_pg_nodes(&typed);

    assert_pg_preserves_row(
        &typed,
        &pg,
        &fix.tok_starts,
        &fix.tok_lens,
        3,
        C_AST_KIND_GNU_STATEMENT_EXPR,
    );
    assert_pg_preserves_row(
        &typed,
        &pg,
        &fix.tok_starts,
        &fix.tok_lens,
        5,
        C_AST_KIND_GOTO_STMT,
    );
    assert_pg_preserves_row(
        &typed,
        &pg,
        &fix.tok_starts,
        &fix.tok_lens,
        8,
        C_AST_KIND_LABEL_STMT,
    );
}

// ---------------------------------------------------------------------------
// Tests – GPU PG lowering parity contracts
// ---------------------------------------------------------------------------

#[test]
fn gpu_parity_pg_lower_label_stmt() {
    let fix = fixture_multiple_consecutive_labels();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed, node_count_from_vast(&typed));
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for label_stmt"
    );
}

#[test]
fn gpu_parity_pg_lower_gnu_statement_expr() {
    let fix = fixture_statement_expression_simple();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed, node_count_from_vast(&typed));
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for gnu_statement_expr"
    );
}

#[test]
fn gpu_parity_pg_lower_statement_expr_in_initializer() {
    let fix = fixture_statement_expression_in_array_init();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed, node_count_from_vast(&typed));
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for statement_expr_in_initializer"
    );
}

#[test]
fn gpu_parity_pg_lower_nested_statement_expr() {
    let fix = fixture_nested_statement_expression();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed, node_count_from_vast(&typed));
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for nested_statement_expr"
    );
}

#[test]
fn gpu_parity_pg_lower_statement_expr_with_label_and_goto() {
    let fix = fixture_statement_expression_with_label_and_goto();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed, node_count_from_vast(&typed));
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for statement_expr_with_label_and_goto"
    );
}
