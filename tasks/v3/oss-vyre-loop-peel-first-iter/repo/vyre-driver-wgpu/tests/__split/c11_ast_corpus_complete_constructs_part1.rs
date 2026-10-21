use super::*;

#[test]
fn cpu_reference_all_corpus_fixtures_build_vast_without_panic() {
    for case in CORPUS_CASES {
        let (tok_types, tok_starts, tok_lens) = (case.fixture)();
        let _raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
        // Merely building must not panic; classification is checked below.
    }
}

#[test]
fn cpu_reference_macro_shaped_decl_preserves_attribute_and_function_decl() {
    let (tok_types, tok_starts, tok_lens) = fixture_macro_shaped_decl_after_preproc();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        typed_indices(&typed, C_AST_KIND_GNU_ATTRIBUTE),
        vec![1],
        "macro-shaped attribute prefix must be a first-class VAST node"
    );
    assert_eq!(
        typed_indices(&typed, node_kind::FUNCTION_DECL),
        vec![12],
        "function declarator after macro-shaped attribute must survive"
    );
    assert_eq!(
        typed_indices(&typed, C_AST_KIND_POINTER_DECL),
        vec![11],
        "return-type pointer declarator must be recognised"
    );
    assert_eq!(
        typed_indices(&typed, C_AST_KIND_FUNCTION_DECLARATOR),
        vec![13],
        "parameter list must be a function declarator"
    );

    for idx in [1usize, 11, 12, 13] {
        assert_eq!(
            word_at(&typed, idx * VAST_STRIDE_U32 + 5),
            tok_starts[idx],
            "macro-shaped decl row {idx} must preserve source start"
        );
    }
}

#[test]
fn cpu_reference_nested_anonymous_aggregates_keep_field_context() {
    let (tok_types, tok_starts, tok_lens) = fixture_nested_anonymous_aggregates();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    let fields = typed_indices(&typed, C_AST_KIND_FIELD_DECL);
    assert!(
        fields.contains(&6),
        "anonymous struct inner field `a` must be a field decl"
    );
    assert!(
        fields.contains(&9),
        "anonymous struct wrapper field must be a field decl"
    );
    assert!(
        fields.contains(&14),
        "anonymous union inner field `b` must be a field decl"
    );
    assert!(
        fields.contains(&17),
        "anonymous union inner field `c` must be a field decl"
    );
    assert!(
        fields.contains(&20),
        "anonymous union wrapper field must be a field decl"
    );
    assert!(
        fields.contains(&30),
        "anonymous enum wrapper field must be a field decl"
    );
    assert!(
        fields.len() >= 6,
        "nested anonymous aggregates must retain at least six field declarations; got {fields:?}"
    );

    let enums = typed_indices(&typed, C_AST_KIND_ENUMERATOR_DECL);
    assert!(
        enums.contains(&24),
        "enumerator `D` with explicit value must be typed"
    );
    assert!(
        enums.contains(&28),
        "enumerator `E` without explicit value must be typed"
    );

    assert_eq!(
        typed_indices(&typed, C_AST_KIND_POINTER_DECL),
        vec![34],
        "function pointer star must be a pointer declarator"
    );
    assert_eq!(
        typed_indices(&typed, C_AST_KIND_ARRAY_DECL),
        vec![36],
        "function pointer array suffix must be an array declarator"
    );
    assert_eq!(
        typed_indices(&typed, C_AST_KIND_FUNCTION_DECLARATOR),
        vec![40],
        "parameter list of function pointer must be a function declarator"
    );
}

