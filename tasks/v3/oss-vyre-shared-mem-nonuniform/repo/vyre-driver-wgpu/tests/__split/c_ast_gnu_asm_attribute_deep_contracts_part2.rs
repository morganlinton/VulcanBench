use super::*;

#[test]
fn non_attribute_identifiers_must_not_leak_attribute_kinds_gpu_cpu_parity() {
    let fix = fixture_non_attribute_identifiers();
    assert_full_pipeline_parity(&fix, "non_attribute_identifiers");

    let typed = classify(&fix);

    let attr_kinds = [
        C_AST_KIND_ATTRIBUTE_SECTION,
        C_AST_KIND_ATTRIBUTE_WEAK,
        C_AST_KIND_ATTRIBUTE_ALIAS,
        C_AST_KIND_ATTRIBUTE_ALIGNED,
        C_AST_KIND_ATTRIBUTE_USED,
        C_AST_KIND_ATTRIBUTE_UNUSED,
        C_AST_KIND_ATTRIBUTE_NAKED,
        C_AST_KIND_ATTRIBUTE_VISIBILITY,
        C_AST_KIND_ATTRIBUTE_CLEANUP,
        C_AST_KIND_ATTRIBUTE_CONSTRUCTOR,
        C_AST_KIND_ATTRIBUTE_DESTRUCTOR,
        C_AST_KIND_ATTRIBUTE_MODE,
        C_AST_KIND_ATTRIBUTE_PACKED,
    ];

    for kind in attr_kinds {
        assert!(
            row_indices(&typed, kind).is_empty(),
            "identifier in non-attribute context must not classify as attribute kind {kind:#010x}"
        );
    }

    assert_eq!(
        word_at(&typed, VAST_STRIDE_U32),
        node_kind::FUNCTION_DECL,
        "cleanup as function name must classify as FUNCTION_DECL"
    );
    assert_eq!(
        word_at(&typed, 4 * VAST_STRIDE_U32),
        node_kind::VARIABLE,
        "section as parameter name must classify as VARIABLE"
    );
}
