use super::*;

#[test]
fn cpu_reference_classifies_hostile_cast_vs_declaration_patterns() {
    let (tok_types, tok_starts, tok_lens) = cast_vs_declaration_fixture();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert!(
        typed_indices(&typed, C_AST_KIND_CAST_EXPR).len() >= 2,
        "ambiguous scalar casts ((T)*p and (T)-1) must classify as cast expressions"
    );
    assert!(
        typed_indices(&typed, C_AST_KIND_POINTER_DECL).contains(&17),
        "T *q declarator must classify as pointer declaration"
    );
    assert!(
        typed_indices(&typed, C_AST_KIND_COMPOUND_LITERAL_EXPR).contains(&30),
        "typed compound literal introducer must classify as compound literal"
    );
    assert!(
        typed_indices(&typed, C_AST_KIND_INITIALIZER_LIST).contains(&33),
        "compound literal body must classify as initializer list"
    );
    assert!(
        typed_indices(&typed, C_AST_KIND_ARRAY_SUBSCRIPT_EXPR).contains(&39),
        "array designator [2] must surface as array-subscript-like AST node"
    );
}

#[test]
fn cpu_reference_classifies_hostile_nested_designated_initializers() {
    let (tok_types, tok_starts, tok_lens) = nested_designated_initializer_fixture();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    let initializer_lists = typed_indices(&typed, C_AST_KIND_INITIALIZER_LIST);
    assert!(
        initializer_lists.len() >= 2,
        "outer and nested designated initializer lists must both be materialized"
    );
    assert!(
        typed_indices(&typed, C_AST_KIND_ARRAY_SUBSCRIPT_EXPR).len() >= 2,
        "multiple array designators ([1], [0]) must classify as array-subscript-like nodes"
    );
    assert_eq!(
        word_at(&typed, 24 * VAST_STRIDE_U32 + 5),
        tok_starts[24],
        "nested initializer list span start must survive classification"
    );
}

#[test]
fn cpu_reference_classifies_storage_and_attribute_qualified_pointer_arrays() {
    let (tok_types, tok_starts, tok_lens) = qualified_function_pointer_array_fixture();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        typed_indices(&typed, C_AST_KIND_GNU_ATTRIBUTE),
        vec![15],
        "GNU attribute qualifier must survive around nested declarators"
    );
    assert_eq!(
        typed_indices(&typed, C_AST_KIND_POINTER_DECL),
        vec![4, 23],
        "both qualified function-pointer arrays must retain pointer declarator nodes"
    );
    assert_eq!(
        typed_indices(&typed, C_AST_KIND_ARRAY_DECL),
        vec![6, 25],
        "both qualified function-pointer arrays must retain array declarator nodes"
    );
    assert_eq!(
        typed_indices(&typed, C_AST_KIND_FUNCTION_DECLARATOR),
        vec![10, 29],
        "both qualified function-pointer arrays must retain function declarator nodes"
    );
    for idx in [4usize, 6, 10, 15, 23, 25, 29] {
        assert_eq!(
            word_at(&typed, idx * VAST_STRIDE_U32 + 5),
            tok_starts[idx],
            "qualified declarator row {idx} must preserve source start"
        );
    }
}

#[test]
fn cpu_reference_keeps_anonymous_aggregate_type_context_for_declarators() {
    let (tok_types, tok_starts, tok_lens) = anonymous_aggregate_declarator_fixture();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        typed_indices(&typed, C_AST_KIND_FIELD_DECL),
        vec![6, 28, 33, 36, 39],
        "anonymous struct/enum/union specifiers must keep declaration context for following fields"
    );
    assert_eq!(
        typed_indices(&typed, C_AST_KIND_POINTER_DECL),
        vec![10],
        "pointer declarator after anonymous aggregate specifier must not degrade to unary expr"
    );
    assert_eq!(
        typed_indices(&typed, C_AST_KIND_ARRAY_DECL),
        vec![12],
        "array declarator nested under anonymous aggregate function pointer must survive"
    );
    assert_eq!(
        typed_indices(&typed, C_AST_KIND_FUNCTION_DECLARATOR),
        vec![16],
        "function declarator after anonymous aggregate function pointer must survive"
    );
    assert_eq!(
        typed_indices(&typed, C_AST_KIND_ENUMERATOR_DECL),
        vec![22, 24],
        "enumerators inside anonymous enum must remain enumerator declarations"
    );

    for idx in [6usize, 10, 12, 16, 22, 24, 28, 33, 36, 39] {
        assert_eq!(
            word_at(&typed, idx * VAST_STRIDE_U32 + 5),
            tok_starts[idx],
            "anonymous aggregate row {idx} must preserve source start"
        );
    }
}

