#[test]
fn pg_lower_preserves_attribute_aligned_on_label() {
    let fix = fixture_attribute_aligned_on_label();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let pg = reference_ast_to_pg_nodes(&typed);

    for idx in row_indices(&typed, C_AST_KIND_LABEL_STMT) {
        assert_pg_preserves_row(&typed, &pg, &fix, idx, C_AST_KIND_LABEL_STMT);
    }
    for idx in row_indices(&typed, C_AST_KIND_ATTRIBUTE_ALIGNED) {
        assert_pg_preserves_row(&typed, &pg, &fix, idx, C_AST_KIND_ATTRIBUTE_ALIGNED);
    }
}

#[test]
fn pg_lower_preserves_multiple_attributes_in_compound() {
    let fix = fixture_multiple_attributes_in_compound();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let pg = reference_ast_to_pg_nodes(&typed);

    for idx in row_indices(&typed, C_AST_KIND_GNU_ATTRIBUTE) {
        assert_pg_preserves_row(&typed, &pg, &fix, idx, C_AST_KIND_GNU_ATTRIBUTE);
    }
    for idx in row_indices(&typed, node_kind::VARIABLE) {
        assert_pg_preserves_row(&typed, &pg, &fix, idx, node_kind::VARIABLE);
    }
}

#[test]
fn pg_lower_preserves_attribute_on_if_arm_statement() {
    let fix = fixture_attribute_on_if_arm_statement();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let pg = reference_ast_to_pg_nodes(&typed);

    for idx in row_indices(&typed, C_AST_KIND_IF_STMT) {
        assert_pg_preserves_row(&typed, &pg, &fix, idx, C_AST_KIND_IF_STMT);
    }
    for idx in row_indices(&typed, C_AST_KIND_GNU_ATTRIBUTE) {
        assert_pg_preserves_row(&typed, &pg, &fix, idx, C_AST_KIND_GNU_ATTRIBUTE);
    }
}

// ---------------------------------------------------------------------------
// GPU/CPU parity contracts
// ---------------------------------------------------------------------------

#[test]
fn gpu_parity_attribute_fallthrough_in_switch() {
    let fix = fixture_attribute_fallthrough_statement();
    assert_full_pipeline_parity(&fix, "attribute_fallthrough_in_switch");
}

#[test]
fn gpu_parity_attribute_unused_in_statement_expr() {
    let fix = fixture_attribute_unused_in_statement_expr();
    assert_full_pipeline_parity(&fix, "attribute_unused_in_statement_expr");
}

#[test]
fn gpu_parity_attribute_aligned_on_label() {
    let fix = fixture_attribute_aligned_on_label();
    assert_full_pipeline_parity(&fix, "attribute_aligned_on_label");
}

#[test]
fn gpu_parity_multiple_attributes_in_compound() {
    let fix = fixture_multiple_attributes_in_compound();
    assert_full_pipeline_parity(&fix, "multiple_attributes_in_compound");
}

#[test]
fn gpu_parity_attribute_on_if_arm_statement() {
    let fix = fixture_attribute_on_if_arm_statement();
    assert_full_pipeline_parity(&fix, "attribute_on_if_arm_statement");
}

// ---------------------------------------------------------------------------
// GPU PG lowering parity contracts
// ---------------------------------------------------------------------------

#[test]
fn gpu_parity_pg_lower_attribute_fallthrough_in_switch() {
    let fix = fixture_attribute_fallthrough_statement();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed);
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for attribute_fallthrough_in_switch"
    );
}

#[test]
fn gpu_parity_pg_lower_attribute_unused_in_statement_expr() {
    let fix = fixture_attribute_unused_in_statement_expr();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed);
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for attribute_unused_in_statement_expr"
    );
}

#[test]
fn gpu_parity_pg_lower_attribute_aligned_on_label() {
    let fix = fixture_attribute_aligned_on_label();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed);
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for attribute_aligned_on_label"
    );
}

#[test]
fn gpu_parity_pg_lower_multiple_attributes_in_compound() {
    let fix = fixture_multiple_attributes_in_compound();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed);
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for multiple_attributes_in_compound"
    );
}

#[test]
fn gpu_parity_pg_lower_attribute_on_if_arm_statement() {
    let fix = fixture_attribute_on_if_arm_statement();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed);
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for attribute_on_if_arm_statement"
    );
}
