#[test]
fn cpu_switch_case_with_compound_literal_classifies() {
    let fix = fixture_switch_case_with_compound_literal();
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
    assert!(
        !row_indices(&typed, C_AST_KIND_COMPOUND_LITERAL_EXPR).is_empty(),
        "compound literal in case body must classify"
    );
}

#[test]
fn cpu_switch_case_with_designated_init_classifies() {
    let fix = fixture_switch_case_with_designated_init();
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
    assert!(
        !row_indices(&typed, C_AST_KIND_INITIALIZER_LIST).is_empty(),
        "initializer list in case body must classify"
    );
    assert!(
        !row_indices(&typed, C_AST_KIND_ARRAY_SUBSCRIPT_EXPR).is_empty(),
        "array designators in case body must classify"
    );
}

#[test]
fn cpu_duffs_device_interleaved_classifies() {
    let fix = fixture_duffs_device_interleaved();
    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_SWITCH_STMT),
        vec![10],
        "switch must classify as SWITCH_STMT"
    );
    assert!(
        !row_indices(&typed, C_AST_KIND_DO_STMT).is_empty(),
        "do must classify as DO_STMT"
    );
    assert!(
        row_indices(&typed, C_AST_KIND_CASE_STMT).len() >= 2,
        "both case labels must classify"
    );
}

#[test]
fn cpu_nested_switch_inside_statement_expr_classifies() {
    let fix = fixture_nested_switch_inside_statement_expr();
    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_GNU_STATEMENT_EXPR),
        vec![6],
        "outer statement expression must classify"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_SWITCH_STMT),
        vec![8],
        "switch inside statement expression must classify"
    );
    assert!(
        !row_indices(&typed, C_AST_KIND_CASE_STMT).is_empty(),
        "case inside statement expression must classify"
    );
    assert!(
        !row_indices(&typed, C_AST_KIND_DEFAULT_STMT).is_empty(),
        "default inside statement expression must classify"
    );
}

#[test]
fn cpu_default_with_user_label_classifies() {
    let fix = fixture_default_with_user_label();
    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_DEFAULT_STMT),
        vec![12],
        "default must classify as DEFAULT_STMT"
    );
    assert!(
        !row_indices(&typed, C_AST_KIND_LABEL_STMT).is_empty(),
        "shared label must classify as LABEL_STMT"
    );
    assert!(
        !row_indices(&typed, C_AST_KIND_BREAK_STMT).is_empty(),
        "break must classify as BREAK_STMT"
    );
}

// ---------------------------------------------------------------------------
// PG lowering preservation contracts
// ---------------------------------------------------------------------------

#[test]
fn pg_lower_preserves_switch_case_with_statement_expr() {
    let fix = fixture_switch_case_with_statement_expr();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let pg = reference_ast_to_pg_nodes(&typed);

    assert_pg_preserves_row(&typed, &pg, &fix, 7, C_AST_KIND_SWITCH_STMT);
    assert_pg_preserves_row(&typed, &pg, &fix, 12, C_AST_KIND_CASE_STMT);
    for idx in row_indices(&typed, C_AST_KIND_GNU_STATEMENT_EXPR) {
        assert_pg_preserves_row(&typed, &pg, &fix, idx, C_AST_KIND_GNU_STATEMENT_EXPR);
    }
    for idx in row_indices(&typed, C_AST_KIND_BREAK_STMT) {
        assert_pg_preserves_row(&typed, &pg, &fix, idx, C_AST_KIND_BREAK_STMT);
    }
}

#[test]
fn pg_lower_preserves_switch_case_with_compound_literal() {
    let fix = fixture_switch_case_with_compound_literal();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let pg = reference_ast_to_pg_nodes(&typed);

    assert_pg_preserves_row(&typed, &pg, &fix, 7, C_AST_KIND_SWITCH_STMT);
    assert_pg_preserves_row(&typed, &pg, &fix, 12, C_AST_KIND_CASE_STMT);
    for idx in row_indices(&typed, C_AST_KIND_COMPOUND_LITERAL_EXPR) {
        assert_pg_preserves_row(&typed, &pg, &fix, idx, C_AST_KIND_COMPOUND_LITERAL_EXPR);
    }
}

#[test]
fn pg_lower_preserves_switch_case_with_designated_init() {
    let fix = fixture_switch_case_with_designated_init();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let pg = reference_ast_to_pg_nodes(&typed);

    assert_pg_preserves_row(&typed, &pg, &fix, 7, C_AST_KIND_SWITCH_STMT);
    assert_pg_preserves_row(&typed, &pg, &fix, 12, C_AST_KIND_CASE_STMT);
    for idx in row_indices(&typed, C_AST_KIND_INITIALIZER_LIST) {
        assert_pg_preserves_row(&typed, &pg, &fix, idx, C_AST_KIND_INITIALIZER_LIST);
    }
}

#[test]
fn pg_lower_preserves_duffs_device_interleaved() {
    let fix = fixture_duffs_device_interleaved();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let pg = reference_ast_to_pg_nodes(&typed);

    assert_pg_preserves_row(&typed, &pg, &fix, 10, C_AST_KIND_SWITCH_STMT);
    for idx in row_indices(&typed, C_AST_KIND_DO_STMT) {
        assert_pg_preserves_row(&typed, &pg, &fix, idx, C_AST_KIND_DO_STMT);
    }
    for idx in row_indices(&typed, C_AST_KIND_CASE_STMT) {
        assert_pg_preserves_row(&typed, &pg, &fix, idx, C_AST_KIND_CASE_STMT);
    }
}

#[test]
fn pg_lower_preserves_nested_switch_inside_statement_expr() {
    let fix = fixture_nested_switch_inside_statement_expr();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let pg = reference_ast_to_pg_nodes(&typed);

    assert_pg_preserves_row(&typed, &pg, &fix, 6, C_AST_KIND_GNU_STATEMENT_EXPR);
    assert_pg_preserves_row(&typed, &pg, &fix, 8, C_AST_KIND_SWITCH_STMT);
    for idx in row_indices(&typed, C_AST_KIND_CASE_STMT) {
        assert_pg_preserves_row(&typed, &pg, &fix, idx, C_AST_KIND_CASE_STMT);
    }
    for idx in row_indices(&typed, C_AST_KIND_DEFAULT_STMT) {
        assert_pg_preserves_row(&typed, &pg, &fix, idx, C_AST_KIND_DEFAULT_STMT);
    }
}

#[test]
fn pg_lower_preserves_default_with_user_label() {
    let fix = fixture_default_with_user_label();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let pg = reference_ast_to_pg_nodes(&typed);

    assert_pg_preserves_row(&typed, &pg, &fix, 12, C_AST_KIND_DEFAULT_STMT);
    for idx in row_indices(&typed, C_AST_KIND_LABEL_STMT) {
        assert_pg_preserves_row(&typed, &pg, &fix, idx, C_AST_KIND_LABEL_STMT);
    }
    for idx in row_indices(&typed, C_AST_KIND_BREAK_STMT) {
        assert_pg_preserves_row(&typed, &pg, &fix, idx, C_AST_KIND_BREAK_STMT);
    }
}

// ---------------------------------------------------------------------------
// GPU/CPU parity contracts
// ---------------------------------------------------------------------------

#[test]
fn gpu_parity_switch_case_with_statement_expr() {
    let fix = fixture_switch_case_with_statement_expr();
    assert_full_pipeline_parity(&fix, "switch_case_with_statement_expr");
}

#[test]
fn gpu_parity_switch_case_with_compound_literal() {
    let fix = fixture_switch_case_with_compound_literal();
    assert_full_pipeline_parity(&fix, "switch_case_with_compound_literal");
}

#[test]
fn gpu_parity_switch_case_with_designated_init() {
    let fix = fixture_switch_case_with_designated_init();
    assert_full_pipeline_parity(&fix, "switch_case_with_designated_init");
}

#[test]
fn gpu_parity_duffs_device_interleaved() {
    let fix = fixture_duffs_device_interleaved();
    assert_full_pipeline_parity(&fix, "duffs_device_interleaved");
}

#[test]
fn gpu_parity_nested_switch_inside_statement_expr() {
    let fix = fixture_nested_switch_inside_statement_expr();
    assert_full_pipeline_parity(&fix, "nested_switch_inside_statement_expr");
}

#[test]
fn gpu_parity_default_with_user_label() {
    let fix = fixture_default_with_user_label();
    assert_full_pipeline_parity(&fix, "default_with_user_label");
}

// ---------------------------------------------------------------------------
// GPU PG lowering parity contracts
// ---------------------------------------------------------------------------

#[test]
fn gpu_parity_pg_lower_switch_case_with_statement_expr() {
    let fix = fixture_switch_case_with_statement_expr();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed);
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for switch_case_with_statement_expr"
    );
}

#[test]
fn gpu_parity_pg_lower_switch_case_with_compound_literal() {
    let fix = fixture_switch_case_with_compound_literal();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed);
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for switch_case_with_compound_literal"
    );
}

#[test]
fn gpu_parity_pg_lower_switch_case_with_designated_init() {
    let fix = fixture_switch_case_with_designated_init();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed);
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for switch_case_with_designated_init"
    );
}

#[test]
fn gpu_parity_pg_lower_duffs_device_interleaved() {
    let fix = fixture_duffs_device_interleaved();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed);
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for duffs_device_interleaved"
    );
}

#[test]
fn gpu_parity_pg_lower_nested_switch_inside_statement_expr() {
    let fix = fixture_nested_switch_inside_statement_expr();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed);
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for nested_switch_inside_statement_expr"
    );
}

#[test]
fn gpu_parity_pg_lower_default_with_user_label() {
    let fix = fixture_default_with_user_label();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed);
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for default_with_user_label"
    );
}
