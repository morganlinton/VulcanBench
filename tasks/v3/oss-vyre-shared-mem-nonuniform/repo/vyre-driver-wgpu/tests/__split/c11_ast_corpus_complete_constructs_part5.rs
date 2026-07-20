use super::*;

#[test]
fn gpu_parity_vast_builder_nested_designated_init() {
    let (tok_types, tok_starts, tok_lens) = fixture_nested_designated_init();
    let expected = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let gpu = run_gpu_vast_builder(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(
        gpu, expected,
        "GPU VAST builder must match CPU for nested designated init"
    );
}

#[test]
fn gpu_parity_vast_builder_stmt_expr_nesting() {
    let (tok_types, tok_starts, tok_lens) = fixture_stmt_expr_nesting();
    let expected = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let gpu = run_gpu_vast_builder(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(
        gpu, expected,
        "GPU VAST builder must match CPU for statement/expression nesting"
    );
}

// ---------------------------------------------------------------------------
// GPU parity  -  classifier
// ---------------------------------------------------------------------------

#[test]
fn gpu_parity_classifier_macro_shaped_decl() {
    let (tok_types, tok_starts, tok_lens) = fixture_macro_shaped_decl_after_preproc();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let expected = reference_c11_classify_vast_node_kinds(&raw);
    let gpu = run_gpu_classifier(&raw, tok_types.len() as u32);
    assert_eq!(
        gpu, expected,
        "GPU classifier must match CPU for macro-shaped decl"
    );
    assert_kind(&gpu, 1, C_AST_KIND_GNU_ATTRIBUTE);
    assert_kind(&gpu, 12, node_kind::FUNCTION_DECL);
}

