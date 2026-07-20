use super::*;

#[test]
fn gpu_parity_classifier_attribute_and_asm() {
    let (tok_types, tok_starts, tok_lens) = fixture_attribute_and_asm();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let expected = reference_c11_classify_vast_node_kinds(&raw);
    let gpu = run_gpu_classifier(&raw, tok_types.len() as u32);
    assert_eq!(
        gpu, expected,
        "GPU classifier must match CPU for attribute and asm"
    );
    assert_kind(&gpu, 0, C_AST_KIND_GNU_ATTRIBUTE);
    assert_kind(&gpu, 12, C_AST_KIND_INLINE_ASM);
    assert_kind(&gpu, 7, C_AST_KIND_FUNCTION_DEFINITION);
}

#[test]
fn gpu_parity_classifier_enum_values() {
    let (tok_types, tok_starts, tok_lens) = fixture_enum_values();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let expected = reference_c11_classify_vast_node_kinds(&raw);
    let gpu = run_gpu_classifier(&raw, tok_types.len() as u32);
    assert_eq!(
        gpu, expected,
        "GPU classifier must match CPU for enum values"
    );
    assert_eq!(
        typed_indices(&gpu, C_AST_KIND_ENUMERATOR_DECL),
        vec![3, 7, 11, 13, 17]
    );
}

#[test]
fn gpu_parity_classifier_sizeof_type_vs_expr() {
    let (tok_types, tok_starts, tok_lens) = fixture_sizeof_type_vs_expr();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let expected = reference_c11_classify_vast_node_kinds(&raw);
    let gpu = run_gpu_classifier(&raw, tok_types.len() as u32);
    assert_eq!(
        gpu, expected,
        "GPU classifier must match CPU for sizeof type-vs-expr"
    );
    assert_kind(&gpu, 9, C_AST_KIND_SIZEOF_EXPR);
    assert_kind(&gpu, 17, C_AST_KIND_SIZEOF_EXPR);
    assert_kind(&gpu, 25, C_AST_KIND_SIZEOF_EXPR);
    assert_kind(&gpu, 28, node_kind::BINARY);
}

