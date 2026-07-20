use super::*;

#[test]
fn gpu_parity_classifier_stmt_expr_nesting() {
    let (tok_types, tok_starts, tok_lens) = fixture_stmt_expr_nesting();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let expected = reference_c11_classify_vast_node_kinds(&raw);
    let gpu = run_gpu_classifier(&raw, tok_types.len() as u32);
    assert_eq!(
        gpu, expected,
        "GPU classifier must match CPU for statement/expression nesting"
    );
    assert_kind(&gpu, 1, C_AST_KIND_FUNCTION_DEFINITION);
    assert_kind(&gpu, 7, C_AST_KIND_RETURN_STMT);
    assert_kind(&gpu, 16, C_AST_KIND_IF_STMT);
    assert_kind(&gpu, 13, C_AST_KIND_CONDITIONAL_EXPR);
}

// ---------------------------------------------------------------------------
// GPU parity  -  PG lowering
// ---------------------------------------------------------------------------

#[test]
fn gpu_parity_pg_lower_macro_shaped_decl() {
    let (tok_types, tok_starts, tok_lens) = fixture_macro_shaped_decl_after_preproc();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed, tok_types.len() as u32);
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for macro-shaped decl"
    );
}

#[test]
fn gpu_parity_pg_lower_nested_anonymous_aggregates() {
    let (tok_types, tok_starts, tok_lens) = fixture_nested_anonymous_aggregates();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed, tok_types.len() as u32);
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for nested anonymous aggregates"
    );
}

