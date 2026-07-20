use super::*;

#[test]
fn cpu_reference_function_pointer_array_with_qualifiers() {
    let (tok_types, tok_starts, tok_lens) = fixture_function_pointer_array();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        typed_indices(&typed, C_AST_KIND_POINTER_DECL),
        vec![3, 12],
        "const-qualified pointer declarator and parameter pointer declarator must survive"
    );
    assert_eq!(
        typed_indices(&typed, C_AST_KIND_ARRAY_DECL),
        vec![6],
        "array declarator must survive after qualified pointer"
    );
    assert_eq!(
        typed_indices(&typed, C_AST_KIND_FUNCTION_DECLARATOR),
        vec![10],
        "function declarator parameter list must survive"
    );

    for idx in [3usize, 6, 10] {
        assert_eq!(
            word_at(&typed, idx * VAST_STRIDE_U32 + 5),
            tok_starts[idx],
            "function pointer array row {idx} must preserve source start"
        );
    }
}

#[test]
fn cpu_reference_nested_designated_init_materialises_all_lists() {
    let (tok_types, tok_starts, tok_lens) = fixture_nested_designated_init();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    let init_lists = typed_indices(&typed, C_AST_KIND_INITIALIZER_LIST);
    assert!(
        init_lists.len() >= 3,
        "outer, middle, and inner initializer lists must all materialise; got {init_lists:?}"
    );

    let designators = typed_indices(&typed, C_AST_KIND_ARRAY_SUBSCRIPT_EXPR);
    assert!(
        designators.len() >= 3,
        "array designators [0], [1], [2] must surface; got {designators:?}"
    );

    let member_access = typed_indices(&typed, C_AST_KIND_MEMBER_ACCESS_EXPR);
    assert!(
        member_access.len() >= 4,
        "dot designators .name, .dims, .x, .y must surface; got {member_access:?}"
    );
}

#[test]
fn cpu_reference_attribute_and_asm_are_first_class_nodes() {
    let (tok_types, tok_starts, tok_lens) = fixture_attribute_and_asm();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        typed_indices(&typed, C_AST_KIND_GNU_ATTRIBUTE),
        vec![0],
        "leading GNU attribute must be a first-class VAST node"
    );
    assert_eq!(
        typed_indices(&typed, C_AST_KIND_INLINE_ASM),
        vec![12],
        "inline asm statement must be a first-class VAST node"
    );
    assert_eq!(
        typed_indices(&typed, C_AST_KIND_FUNCTION_DEFINITION),
        vec![7],
        "attribute-suffixed function with a body must type as FUNCTION_DEFINITION"
    );
    assert_eq!(
        typed_indices(&typed, C_AST_KIND_RETURN_STMT),
        vec![22],
        "return statement must be a first-class node"
    );
}

