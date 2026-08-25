// ---------------------------------------------------------------------------
// PG lowering contract tests
// ---------------------------------------------------------------------------

#[test]
fn pg_lower_preserves_typedef_cast_vs_expr_kinds() {
    let (tok_types, tok_starts, tok_lens) = fixture_typedef_cast_vs_expr_multiply();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let pg = reference_ast_to_pg_nodes(&typed);

    // (T) cast expr must survive lowering.
    assert_eq!(
        pg_word_at(&pg, 10, 0),
        C_AST_KIND_CAST_EXPR,
        "PG must preserve cast expr kind for typedef paren"
    );
    assert_eq!(
        pg_word_at(&pg, 10, 1),
        tok_starts[10],
        "PG cast expr span start must survive"
    );

    // (x) must NOT survive as cast expr in PG.
    assert_ne!(
        pg_word_at(&pg, 16, 0),
        C_AST_KIND_CAST_EXPR,
        "PG must not invent cast expr for variable parenthesisation"
    );
}

#[test]
fn pg_lower_preserves_declarator_context_kinds() {
    let (tok_types, tok_starts, tok_lens) = fixture_declarator_contexts();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let pg = reference_ast_to_pg_nodes(&typed);

    // int *f(int);  -  function decl and function declarator must survive.
    assert_eq!(
        pg_word_at(&pg, 24, 0),
        node_kind::FUNCTION_DECL,
        "PG must preserve FUNCTION_DECL for declarator identifier"
    );
    assert_eq!(
        pg_word_at(&pg, 25, 0),
        C_AST_KIND_FUNCTION_DECLARATOR,
        "PG must preserve FUNCTION_DECLARATOR for parameter list"
    );

    // Pointer declarators must survive.
    assert_eq!(
        pg_word_at(&pg, 7, 0),
        C_AST_KIND_POINTER_DECL,
        "PG must preserve POINTER_DECL"
    );
    assert_eq!(
        pg_word_at(&pg, 15, 0),
        C_AST_KIND_POINTER_DECL,
        "PG must preserve parenthesised POINTER_DECL"
    );
}

// ---------------------------------------------------------------------------
// GPU parity tests (CPU reference == GPU dispatch)
// ---------------------------------------------------------------------------

#[test]
fn gpu_parity_typedef_cast_vs_expr_multiply_classifier() {
    let (tok_types, tok_starts, tok_lens) = fixture_typedef_cast_vs_expr_multiply();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let expected = reference_c11_classify_vast_node_kinds(&raw);
    let gpu = run_gpu_classifier(&raw, tok_types.len() as u32);

    assert_eq!(
        gpu, expected,
        "GPU classifier must match CPU for typedef cast vs expr multiply"
    );
    assert_kind(&gpu, 10, C_AST_KIND_CAST_EXPR);
    assert_kind(&gpu, 13, C_AST_KIND_UNARY_EXPR);
}

#[test]
fn gpu_parity_typedef_shadowing_nested_classifier() {
    let (tok_types, tok_starts, tok_lens) = fixture_typedef_shadowing_nested();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let expected = reference_c11_classify_vast_node_kinds(&raw);
    let gpu = run_gpu_classifier(&raw, tok_types.len() as u32);

    assert_eq!(
        gpu, expected,
        "GPU classifier must match CPU for typedef shadowing"
    );
    // Spot-check that GPU agrees with CPU (even if CPU is contract-broken).
    assert_eq!(
        word_at(&gpu, 15 * VAST_STRIDE_U32),
        word_at(&expected, 15 * VAST_STRIDE_U32),
        "GPU/CPU shadowed-star kind must match"
    );
}

#[test]
fn gpu_parity_struct_tag_vs_typedef_classifier() {
    let (tok_types, tok_starts, tok_lens) = fixture_struct_tag_vs_typedef();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let expected = reference_c11_classify_vast_node_kinds(&raw);
    let gpu = run_gpu_classifier(&raw, tok_types.len() as u32);

    assert_eq!(
        gpu, expected,
        "GPU classifier must match CPU for struct tag vs typedef"
    );
    assert_kind(&gpu, 21, C_AST_KIND_POINTER_DECL);
    assert_kind(&gpu, 25, C_AST_KIND_POINTER_DECL);
}

#[test]
fn gpu_parity_declarator_contexts_classifier() {
    let (tok_types, tok_starts, tok_lens) = fixture_declarator_contexts();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let expected = reference_c11_classify_vast_node_kinds(&raw);
    let gpu = run_gpu_classifier(&raw, tok_types.len() as u32);

    assert_eq!(
        gpu, expected,
        "GPU classifier must match CPU for declarator contexts"
    );
    assert_eq!(
        typed_indices(&gpu, C_AST_KIND_POINTER_DECL),
        vec![7, 15, 23, 31],
        "GPU must classify all four stars as POINTER_DECL"
    );
    assert_eq!(
        typed_indices(&gpu, C_AST_KIND_ARRAY_DECL),
        vec![9, 18],
        "GPU must classify both array brackets as ARRAY_DECL"
    );
    assert_eq!(
        typed_indices(&gpu, C_AST_KIND_FUNCTION_DECLARATOR),
        vec![2, 25, 34],
        "GPU must classify all parameter parens as FUNCTION_DECLARATOR"
    );
}

// ---------------------------------------------------------------------------
// GPU parity for VAST builder (delimiter tree)
// ---------------------------------------------------------------------------

#[test]
fn gpu_parity_vast_builder_declarator_contexts() {
    let (tok_types, tok_starts, tok_lens) = fixture_declarator_contexts();
    let expected = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let gpu = run_gpu_vast_builder(&tok_types, &tok_starts, &tok_lens);

    assert_eq!(
        gpu, expected,
        "GPU VAST builder must match CPU for declarator contexts"
    );
}

#[test]
fn gpu_parity_vast_builder_struct_tag_vs_typedef() {
    let (tok_types, tok_starts, tok_lens) = fixture_struct_tag_vs_typedef();
    let expected = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let gpu = run_gpu_vast_builder(&tok_types, &tok_starts, &tok_lens);

    assert_eq!(
        gpu, expected,
        "GPU VAST builder must match CPU for struct tag vs typedef"
    );
}

// ---------------------------------------------------------------------------
// GPU parity for PG lowering
// ---------------------------------------------------------------------------

#[test]
fn gpu_parity_pg_lower_declarator_contexts() {
    let (tok_types, tok_starts, tok_lens) = fixture_declarator_contexts();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed, tok_types.len() as u32);

    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for declarator contexts"
    );
}

#[test]
fn gpu_parity_pg_lower_struct_tag_vs_typedef() {
    let (tok_types, tok_starts, tok_lens) = fixture_struct_tag_vs_typedef();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed, tok_types.len() as u32);

    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for struct tag vs typedef"
    );
}
