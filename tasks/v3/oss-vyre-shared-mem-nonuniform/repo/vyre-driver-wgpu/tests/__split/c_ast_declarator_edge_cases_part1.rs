use super::*;

#[test]
fn cpu_array_of_function_pointers_kinds() {
    let (tok_types, tok_starts, tok_lens) = fixture_array_of_function_pointers();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        typed_indices(&typed, C_AST_KIND_POINTER_DECL),
        vec![2, 10],
        "function pointer array must contain pointer declarators for outer and parameter"
    );
    assert_eq!(
        typed_indices(&typed, C_AST_KIND_ARRAY_DECL),
        vec![4],
        "function pointer array must contain array declarator"
    );
    assert_eq!(
        typed_indices(&typed, C_AST_KIND_FUNCTION_DECLARATOR),
        vec![8],
        "function pointer array must contain function declarator"
    );
}

#[test]
fn cpu_function_returning_fnptr_kinds() {
    let (tok_types, tok_starts, tok_lens) = fixture_function_returning_fnptr();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        typed_indices(&typed, C_AST_KIND_POINTER_DECL),
        vec![2],
        "fn-returning-fnptr must contain pointer declarator"
    );
    assert_eq!(
        typed_indices(&typed, C_AST_KIND_FUNCTION_DECLARATOR),
        vec![4, 8],
        "fn-returning-fnptr must contain two function declarators"
    );
}

#[test]
fn cpu_nested_qualifiers_kinds() {
    let (tok_types, tok_starts, tok_lens) = fixture_nested_qualifiers();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        typed_indices(&typed, C_AST_KIND_POINTER_DECL),
        vec![2, 4],
        "nested qualifiers must contain two pointer declarators"
    );
}

#[test]
fn cpu_parameter_typedef_shadowing() {
    let fix = fixture_parameter_typedef_shadowing();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, &fix.haystack);
    let typed = reference_c11_classify_vast_node_kinds(&annotated);

    // The parameter T (token index 8) is an ordinary identifier decl.
    let param_t_flags = word_at(&annotated, 8 * VAST_STRIDE_U32 + TYPEDEF_FLAGS_FIELD);
    assert_eq!(
        param_t_flags & ORDINARY_FLAG_DECL,
        ORDINARY_FLAG_DECL,
        "parameter T must be flagged as ordinary identifier declaration"
    );

    // T * y inside the body must be multiplication, not pointer declaration.
    assert_eq!(
        word_at(&typed, 12 * VAST_STRIDE_U32),
        node_kind::BINARY,
        "shadowed typedef T * y must classify * as binary multiplication"
    );
}

#[test]
fn cpu_abstract_declarator_cast_kinds() {
    let (tok_types, tok_starts, tok_lens) = fixture_abstract_declarator_cast();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        typed_indices(&typed, C_AST_KIND_CAST_EXPR),
        vec![0],
        "abstract declarator cast must classify only the outer type-name paren as CAST_EXPR"
    );
    assert_eq!(
        typed_indices(&typed, C_AST_KIND_POINTER_DECL),
        vec![3],
        "abstract declarator cast must contain pointer declarator"
    );
    assert_eq!(
        typed_indices(&typed, C_AST_KIND_FUNCTION_DECLARATOR),
        vec![5],
        "abstract function-pointer parameter suffix must classify as FUNCTION_DECLARATOR"
    );
}

#[test]
fn cpu_abstract_declarator_sizeof_kinds() {
    let (tok_types, tok_starts, tok_lens) = fixture_abstract_declarator_sizeof();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        typed_indices(&typed, C_AST_KIND_SIZEOF_EXPR),
        vec![0],
        "sizeof abstract declarator must classify as sizeof expression"
    );
    assert_eq!(
        typed_indices(&typed, C_AST_KIND_POINTER_DECL),
        vec![4],
        "sizeof abstract declarator must contain pointer declarator"
    );
    assert_eq!(
        typed_indices(&typed, C_AST_KIND_FUNCTION_DECLARATOR),
        vec![6],
        "sizeof abstract declarator must contain function declarator"
    );
}

#[test]
fn cpu_kr_function_kinds() {
    let (tok_types, tok_starts, tok_lens) = fixture_kr_function();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        typed_indices(&typed, C_AST_KIND_FUNCTION_DECLARATOR),
        vec![2],
        "K&R function must classify parameter list as function declarator"
    );
}

#[test]
fn cpu_deeply_parenthesised_pointer_kinds() {
    let (tok_types, tok_starts, tok_lens) = fixture_deeply_parenthesised_pointer();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        typed_indices(&typed, C_AST_KIND_POINTER_DECL),
        vec![4],
        "deeply parenthesised pointer must classify inner * as pointer declarator"
    );
}

#[test]
fn cpu_qualified_pointer_array_kinds() {
    let (tok_types, tok_starts, tok_lens) = fixture_qualified_pointer_array();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        typed_indices(&typed, C_AST_KIND_POINTER_DECL),
        vec![2],
        "qualified pointer array must contain pointer declarator"
    );
    assert_eq!(
        typed_indices(&typed, C_AST_KIND_ARRAY_DECL),
        vec![5],
        "qualified pointer array must contain array declarator"
    );
}

#[test]
fn cpu_gpu_multiple_declarators_keep_commas_out_of_expression_space() {
    let (tok_types, tok_starts, tok_lens) = fixture_multiple_declarators();
    let typed = cpu_gpu_classified(&tok_types, &tok_starts, &tok_lens);

    assert_eq!(
        word_at(&typed, VAST_STRIDE_U32),
        node_kind::VARIABLE,
        "first declarator identifier must be a variable"
    );
    assert_eq!(
        word_at(&typed, 4 * VAST_STRIDE_U32),
        node_kind::VARIABLE,
        "pointer declarator identifier must be a variable"
    );
    assert_eq!(
        word_at(&typed, 6 * VAST_STRIDE_U32),
        node_kind::VARIABLE,
        "array declarator identifier must be a variable"
    );
    assert_eq!(
        word_at(&typed, 3 * VAST_STRIDE_U32),
        C_AST_KIND_POINTER_DECL,
        "star before b must be a pointer declarator"
    );
    assert_eq!(
        word_at(&typed, 7 * VAST_STRIDE_U32),
        C_AST_KIND_ARRAY_DECL,
        "bracket after c must be an array declarator"
    );
    assert_eq!(
        word_at(&typed, 2 * VAST_STRIDE_U32),
        0,
        "comma between declarators must not become a binary expression"
    );
    assert_eq!(
        word_at(&typed, 5 * VAST_STRIDE_U32),
        0,
        "second comma between declarators must not become a binary expression"
    );
}

#[test]
fn cpu_gpu_flexible_array_member_keeps_empty_brackets_as_array_decl() {
    let (tok_types, tok_starts, tok_lens) = fixture_flexible_array_member();
    let typed = cpu_gpu_classified(&tok_types, &tok_starts, &tok_lens);

    assert_eq!(
        word_at(&typed, 3 * VAST_STRIDE_U32),
        C_AST_KIND_FIELD_DECL,
        "len inside struct body must be a field declaration"
    );
    assert_eq!(
        word_at(&typed, 6 * VAST_STRIDE_U32),
        C_AST_KIND_FIELD_DECL,
        "data inside struct body must be a field declaration"
    );
    assert_eq!(
        word_at(&typed, 7 * VAST_STRIDE_U32),
        C_AST_KIND_ARRAY_DECL,
        "empty flexible-array brackets must still classify as array declarator"
    );
}

#[test]
fn cpu_gpu_bitfields_do_not_invent_unnamed_field_declarators() {
    let (tok_types, tok_starts, tok_lens) = fixture_bitfield_declarators();
    let typed = cpu_gpu_classified(&tok_types, &tok_starts, &tok_lens);

    assert_eq!(
        word_at(&typed, 3 * VAST_STRIDE_U32),
        C_AST_KIND_BIT_FIELD_DECL,
        "named bitfield must classify its identifier as a bit-field declaration"
    );
    assert_eq!(
        word_at(&typed, 4 * VAST_STRIDE_U32),
        0,
        "bitfield colon must remain raw syntax"
    );
    assert_eq!(
        word_at(&typed, 5 * VAST_STRIDE_U32),
        node_kind::LITERAL,
        "bitfield width must classify as literal"
    );
    assert_eq!(
        word_at(&typed, 9 * VAST_STRIDE_U32),
        C_AST_KIND_BIT_FIELD_DECL,
        "unnamed zero-width bitfield colon must be the bit-field declaration marker"
    );
    assert_eq!(
        word_at(&typed, 10 * VAST_STRIDE_U32),
        node_kind::LITERAL,
        "unnamed bitfield width must classify as literal"
    );
}

// ---------------------------------------------------------------------------
// GPU parity tests  -  VAST builder
// ---------------------------------------------------------------------------

#[test]
fn gpu_parity_vast_builder_array_of_function_pointers() {
    let (tok_types, tok_starts, tok_lens) = fixture_array_of_function_pointers();
    let expected = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let gpu = run_gpu_vast_builder(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(
        gpu, expected,
        "GPU VAST builder must match CPU for array of function pointers"
    );
}

#[test]
fn gpu_parity_vast_builder_function_returning_fnptr() {
    let (tok_types, tok_starts, tok_lens) = fixture_function_returning_fnptr();
    let expected = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let gpu = run_gpu_vast_builder(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(
        gpu, expected,
        "GPU VAST builder must match CPU for function returning fnptr"
    );
}

#[test]
fn gpu_parity_vast_builder_nested_qualifiers() {
    let (tok_types, tok_starts, tok_lens) = fixture_nested_qualifiers();
    let expected = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let gpu = run_gpu_vast_builder(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(
        gpu, expected,
        "GPU VAST builder must match CPU for nested qualifiers"
    );
}

#[test]
fn gpu_parity_vast_builder_parameter_typedef_shadowing() {
    let fix = fixture_parameter_typedef_shadowing();
    let expected = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let gpu = run_gpu_vast_builder(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    assert_eq!(
        gpu, expected,
        "GPU VAST builder must match CPU for parameter typedef shadowing"
    );
}

#[test]
fn gpu_parity_vast_builder_abstract_declarator_cast() {
    let (tok_types, tok_starts, tok_lens) = fixture_abstract_declarator_cast();
    let expected = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let gpu = run_gpu_vast_builder(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(
        gpu, expected,
        "GPU VAST builder must match CPU for abstract declarator cast"
    );
}

#[test]
fn gpu_parity_vast_builder_abstract_declarator_sizeof() {
    let (tok_types, tok_starts, tok_lens) = fixture_abstract_declarator_sizeof();
    let expected = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let gpu = run_gpu_vast_builder(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(
        gpu, expected,
        "GPU VAST builder must match CPU for abstract declarator sizeof"
    );
}

#[test]
fn gpu_parity_vast_builder_kr_function() {
    let (tok_types, tok_starts, tok_lens) = fixture_kr_function();
    let expected = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let gpu = run_gpu_vast_builder(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(
        gpu, expected,
        "GPU VAST builder must match CPU for K&R function declaration"
    );
}

#[test]
fn gpu_parity_vast_builder_deeply_parenthesised_pointer() {
    let (tok_types, tok_starts, tok_lens) = fixture_deeply_parenthesised_pointer();
    let expected = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let gpu = run_gpu_vast_builder(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(
        gpu, expected,
        "GPU VAST builder must match CPU for deeply parenthesised pointer"
    );
}

#[test]
fn gpu_parity_vast_builder_qualified_pointer_array() {
    let (tok_types, tok_starts, tok_lens) = fixture_qualified_pointer_array();
    let expected = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let gpu = run_gpu_vast_builder(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(
        gpu, expected,
        "GPU VAST builder must match CPU for qualified pointer array"
    );
}

// ---------------------------------------------------------------------------
// GPU parity tests  -  classifier
// ---------------------------------------------------------------------------

#[test]
fn gpu_parity_classifier_array_of_function_pointers() {
    let (tok_types, tok_starts, tok_lens) = fixture_array_of_function_pointers();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let expected = reference_c11_classify_vast_node_kinds(&raw);
    let gpu = run_gpu_classifier(&raw, node_count_from_vast(&raw));
    assert_eq!(
        gpu, expected,
        "GPU classifier must match CPU for array of function pointers"
    );
}

#[test]
fn gpu_parity_classifier_function_returning_fnptr() {
    let (tok_types, tok_starts, tok_lens) = fixture_function_returning_fnptr();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let expected = reference_c11_classify_vast_node_kinds(&raw);
    let gpu = run_gpu_classifier(&raw, node_count_from_vast(&raw));
    assert_eq!(
        gpu, expected,
        "GPU classifier must match CPU for function returning fnptr"
    );
}

#[test]
fn gpu_parity_classifier_nested_qualifiers() {
    let (tok_types, tok_starts, tok_lens) = fixture_nested_qualifiers();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let expected = reference_c11_classify_vast_node_kinds(&raw);
    let gpu = run_gpu_classifier(&raw, node_count_from_vast(&raw));
    assert_eq!(
        gpu, expected,
        "GPU classifier must match CPU for nested qualifiers"
    );
}

#[test]
fn gpu_parity_classifier_parameter_typedef_shadowing() {
    let fix = fixture_parameter_typedef_shadowing();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, &fix.haystack);
    let expected = reference_c11_classify_vast_node_kinds(&annotated);
    let gpu_annotated = run_gpu_annotate(&raw, &fix.haystack, node_count_from_vast(&raw));
    let gpu = run_gpu_classifier(&gpu_annotated, node_count_from_vast(&gpu_annotated));
    assert_eq!(
        gpu, expected,
        "GPU classifier must match CPU for parameter typedef shadowing"
    );
}

