use super::*;

#[test]
fn gpu_parity_classifier_nested_anonymous_aggregates() {
    let (tok_types, tok_starts, tok_lens) = fixture_nested_anonymous_aggregates();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let expected = reference_c11_classify_vast_node_kinds(&raw);
    let gpu = run_gpu_classifier(&raw, tok_types.len() as u32);
    assert_eq!(
        gpu, expected,
        "GPU classifier must match CPU for nested anonymous aggregates"
    );
    assert!(typed_indices(&gpu, C_AST_KIND_FIELD_DECL).len() >= 6);
    assert!(typed_indices(&gpu, C_AST_KIND_ENUMERATOR_DECL).len() >= 2);
    assert!(typed_indices(&gpu, C_AST_KIND_POINTER_DECL).contains(&34));
    assert!(typed_indices(&gpu, C_AST_KIND_ARRAY_DECL).contains(&36));
}

#[test]
fn gpu_parity_classifier_function_pointer_array() {
    let (tok_types, tok_starts, tok_lens) = fixture_function_pointer_array();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let expected = reference_c11_classify_vast_node_kinds(&raw);
    let gpu = run_gpu_classifier(&raw, tok_types.len() as u32);
    assert_eq!(
        gpu, expected,
        "GPU classifier must match CPU for function pointer array"
    );
    assert!(typed_indices(&gpu, C_AST_KIND_POINTER_DECL).contains(&3));
    assert!(typed_indices(&gpu, C_AST_KIND_ARRAY_DECL).contains(&6));
    assert!(typed_indices(&gpu, C_AST_KIND_FUNCTION_DECLARATOR).contains(&10));
}

#[test]
fn gpu_parity_classifier_nested_designated_init() {
    let (tok_types, tok_starts, tok_lens) = fixture_nested_designated_init();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let expected = reference_c11_classify_vast_node_kinds(&raw);
    let gpu = run_gpu_classifier(&raw, tok_types.len() as u32);
    assert_eq!(
        gpu, expected,
        "GPU classifier must match CPU for nested designated init"
    );
    assert!(typed_indices(&gpu, C_AST_KIND_INITIALIZER_LIST).len() >= 3);
    assert!(typed_indices(&gpu, C_AST_KIND_ARRAY_SUBSCRIPT_EXPR).len() >= 3);
}

