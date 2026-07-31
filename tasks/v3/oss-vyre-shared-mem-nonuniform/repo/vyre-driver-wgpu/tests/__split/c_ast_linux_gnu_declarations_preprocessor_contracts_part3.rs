// (use super::* removed  -  parts now share crate-root scope via flat include)

#[test]
fn cpu_attribute_on_struct_definition_classifies() {
    let fix = fixture_attribute_on_struct_definition();
    let typed = classify_fixture(&fix);

    assert_eq!(
        row_indices(&typed, C_AST_KIND_GNU_ATTRIBUTE),
        vec![1],
        "__attribute__ must classify"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ATTRIBUTE_PACKED),
        vec![4],
        "packed must classify"
    );
    assert_eq!(
        word_at(&typed, 7 * VAST_STRIDE_U32),
        node_kind::VARIABLE,
        "struct tag S must be VARIABLE"
    );
    assert_eq!(
        word_at(&typed, 8 * VAST_STRIDE_U32),
        node_kind::BASIC_BLOCK,
        "struct body brace must be BASIC_BLOCK"
    );
    assert_eq!(
        word_at(&typed, 10 * VAST_STRIDE_U32),
        C_AST_KIND_FIELD_DECL,
        "field x must classify"
    );
}

#[test]
fn cpu_attribute_on_function_pointer_typedef_classifies() {
    let fix = fixture_attribute_on_function_pointer_typedef();
    let typed = classify_fixture(&fix);

    assert_eq!(
        row_indices(&typed, C_AST_KIND_GNU_ATTRIBUTE),
        vec![4],
        "__attribute__ must classify"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ATTRIBUTE_ALIGNED),
        vec![7],
        "aligned must classify"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_POINTER_DECL),
        vec![3],
        "pointer declarator must classify"
    );
    assert_eq!(
        word_at(&typed, 13 * VAST_STRIDE_U32),
        node_kind::VARIABLE,
        "fp must be VARIABLE"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_FUNCTION_DECLARATOR),
        vec![15],
        "function suffix must be FUNCTION_DECLARATOR"
    );
}

#[test]
fn gpu_parity_attribute_on_struct_definition() {
    let fix = fixture_attribute_on_struct_definition();
    assert_full_pipeline_parity(&fix, "attribute_on_struct_definition");
}

