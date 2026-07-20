use super::*;

#[test]
fn cpu_reference_typedef_expr_ambiguity() {
    let (tok_types, tok_starts, tok_lens) = fixture_typedef_expr_ambiguity();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        word_at(&typed, 11 * VAST_STRIDE_U32),
        C_AST_KIND_POINTER_DECL,
        "typedef name followed by star in declaration position must be a pointer declarator"
    );

    // Casts are recognised because the paren introduces a type-name context.
    let casts = typed_indices(&typed, C_AST_KIND_CAST_EXPR);
    assert!(
        casts.len() >= 2,
        "ambiguous scalar casts ((Foo)*b and (Foo)-1) must classify as cast expressions; got {casts:?}"
    );
    assert!(
        casts.contains(&14),
        "(Foo)*b cast paren must be a cast expr"
    );
    assert!(
        casts.contains(&20),
        "(Foo)-1 cast paren must be a cast expr"
    );

    // Compound literal
    assert_eq!(
        word_at(&typed, 28 * VAST_STRIDE_U32),
        C_AST_KIND_COMPOUND_LITERAL_EXPR,
        "typed compound literal introducer must classify as compound literal"
    );
    assert_eq!(
        word_at(&typed, 31 * VAST_STRIDE_U32),
        C_AST_KIND_INITIALIZER_LIST,
        "compound literal body must classify as initializer list"
    );
}

#[test]
fn cpu_reference_deeply_nested_declarator() {
    let (tok_types, tok_starts, tok_lens) = fixture_deeply_nested_declarator();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        typed_indices(&typed, C_AST_KIND_POINTER_DECL),
        vec![2, 4],
        "nested pointer declarators must preserve declaration context through parentheses"
    );

    let array_decls = typed_indices(&typed, C_AST_KIND_ARRAY_DECL);
    assert!(
        array_decls.contains(&6) && array_decls.contains(&14),
        "array declarators inside and after parenthesized declarators must survive; got {array_decls:?}"
    );
}

#[test]
fn cpu_reference_nested_struct_enum() {
    let (tok_types, tok_starts, tok_lens) = fixture_nested_struct_enum();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    let fields = typed_indices(&typed, C_AST_KIND_FIELD_DECL);
    assert!(
        fields.contains(&4),
        "outer struct field `a` must be a field decl"
    );
    assert!(
        fields.contains(&9),
        "nested struct field `b` must be a field decl"
    );
    assert!(
        fields.contains(&12),
        "outer struct field `nested` must be a field decl"
    );

    let enums = typed_indices(&typed, C_AST_KIND_ENUMERATOR_DECL);
    assert!(
        enums.contains(&19),
        "enumerator `A` must be typed as enumerator decl"
    );
    assert!(
        enums.contains(&21),
        "enumerator `B` must be typed as enumerator decl"
    );
}

#[test]
fn cpu_reference_nested_designated_init() {
    let (tok_types, tok_starts, tok_lens) = fixture_nested_designated_init();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    let init_lists = typed_indices(&typed, C_AST_KIND_INITIALIZER_LIST);
    assert!(
        init_lists.len() >= 2,
        "outer and nested initializer lists must both materialise; got {init_lists:?}"
    );

    let designators = typed_indices(&typed, C_AST_KIND_ARRAY_SUBSCRIPT_EXPR);
    assert!(
        designators.len() >= 4,
        "every [N] = designator must surface as array-subscript-like node; got {designators:?}"
    );
}

#[test]
fn cpu_reference_gnu_attribute_inline_asm() {
    let (tok_types, tok_starts, tok_lens) = fixture_gnu_attribute_inline_asm();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        typed_indices(&typed, C_AST_KIND_GNU_ATTRIBUTE),
        vec![0],
        "GNU attribute prefix must be a first-class VAST node"
    );
    assert_eq!(
        typed_indices(&typed, C_AST_KIND_INLINE_ASM),
        vec![13],
        "inline asm statement must be a first-class VAST node"
    );

    assert_eq!(
        typed_indices(&typed, C_AST_KIND_FUNCTION_DEFINITION),
        vec![7],
        "attribute-suffixed function with a body must type as FUNCTION_DEFINITION"
    );
}

#[test]
fn cpu_reference_compound_literal_stress() {
    let (tok_types, tok_starts, tok_lens) = fixture_compound_literal_stress();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    // (int []) {1,2,3}
    assert!(
        typed_indices(&typed, C_AST_KIND_COMPOUND_LITERAL_EXPR).contains(&10),
        "(int []) compound literal introducer must be typed"
    );
    assert!(
        typed_indices(&typed, C_AST_KIND_INITIALIZER_LIST).contains(&15),
        "(int []) body must be an initializer list"
    );

    // (struct S) { .a = 1 }
    assert!(
        typed_indices(&typed, C_AST_KIND_COMPOUND_LITERAL_EXPR).contains(&28),
        "(struct S) compound literal introducer must be typed"
    );
    assert!(
        typed_indices(&typed, C_AST_KIND_INITIALIZER_LIST).contains(&32),
        "(struct S) body must be an initializer list"
    );

    // Pointer declarators for `int *p` and `struct S *s`
    let ptrs = typed_indices(&typed, C_AST_KIND_POINTER_DECL);
    assert!(
        ptrs.contains(&7),
        "`int *p` star must be a pointer declarator"
    );
    assert!(
        ptrs.contains(&25),
        "`struct S *s` star must be a pointer declarator"
    );
}

// ---------------------------------------------------------------------------
// PG lowering correctness
// ---------------------------------------------------------------------------

#[test]
fn pg_lower_preserves_hostile_typedef_ambiguity_kinds() {
    let (tok_types, tok_starts, tok_lens) = fixture_typedef_expr_ambiguity();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let pg = reference_ast_to_pg_nodes(&typed);

    let pg_casts = pg
        .chunks_exact(PG_STRIDE_U32 * 4)
        .enumerate()
        .filter_map(|(idx, row)| {
            let kind = u32::from_le_bytes(row[0..4].try_into().unwrap());
            (kind == C_AST_KIND_CAST_EXPR).then_some(idx)
        })
        .collect::<Vec<_>>();
    assert!(
        pg_casts.len() >= 2,
        "PG must preserve at least two cast expr kinds"
    );

    // Compound literal span must survive lowering
    let cl_idx = 28usize;
    assert_eq!(
        pg_word_at(&pg, cl_idx, 0),
        C_AST_KIND_COMPOUND_LITERAL_EXPR,
        "PG compound literal kind must survive lowering"
    );
    assert_eq!(
        pg_word_at(&pg, cl_idx, 1),
        tok_starts[cl_idx],
        "PG compound literal span start must survive"
    );
}

#[test]
fn pg_lower_preserves_nested_declarator_kinds() {
    let (tok_types, tok_starts, tok_lens) = fixture_deeply_nested_declarator();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let pg = reference_ast_to_pg_nodes(&typed);

    // Outer pointer declarator
    assert_eq!(
        pg_word_at(&pg, 2, 0),
        C_AST_KIND_POINTER_DECL,
        "PG outer pointer declarator kind must survive lowering"
    );
    assert_eq!(
        pg_word_at(&pg, 2, 1),
        tok_starts[2],
        "PG outer pointer declarator span start must survive"
    );
}

#[test]
fn pg_lower_preserves_gnu_attribute_and_asm_kinds() {
    let (tok_types, tok_starts, tok_lens) = fixture_gnu_attribute_inline_asm();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let pg = reference_ast_to_pg_nodes(&typed);

    assert_eq!(
        pg_word_at(&pg, 0, 0),
        C_AST_KIND_GNU_ATTRIBUTE,
        "PG must preserve GNU attribute kind"
    );
    assert_eq!(
        pg_word_at(&pg, 13, 0),
        C_AST_KIND_INLINE_ASM,
        "PG must preserve inline asm kind"
    );
    assert_eq!(
        pg_word_at(&pg, 7, 0),
        C_AST_KIND_FUNCTION_DEFINITION,
        "PG must preserve function definition despite attribute"
    );
}

// ---------------------------------------------------------------------------
// GPU parity tests (CPU reference == GPU dispatch)
// ---------------------------------------------------------------------------

#[test]
fn gpu_parity_typedef_expr_ambiguity_classifier() {
    let (tok_types, tok_starts, tok_lens) = fixture_typedef_expr_ambiguity();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let expected = reference_c11_classify_vast_node_kinds(&raw);
    let gpu = run_gpu_classifier(&raw, tok_types.len() as u32);

    assert_eq!(
        gpu, expected,
        "GPU classifier must match CPU for typedef ambiguity"
    );
    assert_eq!(
        word_at(&gpu, 11 * VAST_STRIDE_U32),
        C_AST_KIND_POINTER_DECL,
        "GPU: typedef-name pointer declarator must be classified"
    );
    assert!(typed_indices(&gpu, C_AST_KIND_CAST_EXPR).len() >= 2);
    assert_kind(&gpu, 28, C_AST_KIND_COMPOUND_LITERAL_EXPR);
    assert_kind(&gpu, 31, C_AST_KIND_INITIALIZER_LIST);
}

#[test]
fn gpu_parity_nested_declarator_classifier() {
    let (tok_types, tok_starts, tok_lens) = fixture_deeply_nested_declarator();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let expected = reference_c11_classify_vast_node_kinds(&raw);
    let gpu = run_gpu_classifier(&raw, tok_types.len() as u32);

    assert_eq!(
        gpu, expected,
        "GPU classifier must match CPU for nested declarator"
    );
    assert_eq!(
        typed_indices(&gpu, C_AST_KIND_POINTER_DECL),
        vec![2, 4],
        "GPU: nested pointer declarators must preserve declaration context"
    );
}

#[test]
fn gpu_parity_nested_struct_enum_classifier() {
    let (tok_types, tok_starts, tok_lens) = fixture_nested_struct_enum();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let expected = reference_c11_classify_vast_node_kinds(&raw);
    let gpu = run_gpu_classifier(&raw, tok_types.len() as u32);

    assert_eq!(
        gpu, expected,
        "GPU classifier must match CPU for nested struct/enum"
    );
    let fields = typed_indices(&gpu, C_AST_KIND_FIELD_DECL);
    assert!(fields.contains(&4), "GPU field a");
    assert!(fields.contains(&9), "GPU field b");
    assert!(fields.contains(&12), "GPU field nested");

    let enums = typed_indices(&gpu, C_AST_KIND_ENUMERATOR_DECL);
    assert!(enums.contains(&19), "GPU enumerator A");
    assert!(enums.contains(&21), "GPU enumerator B");
}

#[test]
fn gpu_parity_designated_init_classifier() {
    let (tok_types, tok_starts, tok_lens) = fixture_nested_designated_init();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let expected = reference_c11_classify_vast_node_kinds(&raw);
    let gpu = run_gpu_classifier(&raw, tok_types.len() as u32);

    assert_eq!(
        gpu, expected,
        "GPU classifier must match CPU for designated init"
    );
    assert!(
        typed_indices(&gpu, C_AST_KIND_INITIALIZER_LIST).len() >= 2,
        "GPU must materialise nested initializer lists"
    );
    assert!(
        typed_indices(&gpu, C_AST_KIND_ARRAY_SUBSCRIPT_EXPR).len() >= 4,
        "GPU must surface array designators"
    );
}

#[test]
fn gpu_parity_gnu_attribute_asm_classifier() {
    let (tok_types, tok_starts, tok_lens) = fixture_gnu_attribute_inline_asm();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let expected = reference_c11_classify_vast_node_kinds(&raw);
    let gpu = run_gpu_classifier(&raw, tok_types.len() as u32);

    assert_eq!(
        gpu, expected,
        "GPU classifier must match CPU for GNU attribute + inline asm"
    );
    assert_kind(&gpu, 0, C_AST_KIND_GNU_ATTRIBUTE);
    assert_kind(&gpu, 13, C_AST_KIND_INLINE_ASM);
    assert_kind(&gpu, 7, C_AST_KIND_FUNCTION_DEFINITION);
}

#[test]
fn gpu_parity_compound_literal_classifier() {
    let (tok_types, tok_starts, tok_lens) = fixture_compound_literal_stress();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let expected = reference_c11_classify_vast_node_kinds(&raw);
    let gpu = run_gpu_classifier(&raw, tok_types.len() as u32);

    assert_eq!(
        gpu, expected,
        "GPU classifier must match CPU for compound literal stress"
    );
    assert!(typed_indices(&gpu, C_AST_KIND_COMPOUND_LITERAL_EXPR).contains(&10));
    assert!(typed_indices(&gpu, C_AST_KIND_INITIALIZER_LIST).contains(&15));
    assert!(typed_indices(&gpu, C_AST_KIND_COMPOUND_LITERAL_EXPR).contains(&28));
    assert!(typed_indices(&gpu, C_AST_KIND_INITIALIZER_LIST).contains(&32));
}

// ---------------------------------------------------------------------------
// GPU parity for VAST builder (delimiter tree)
// ---------------------------------------------------------------------------

#[test]
fn gpu_parity_vast_builder_nested_declarator() {
    let (tok_types, tok_starts, tok_lens) = fixture_deeply_nested_declarator();
    let expected = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let gpu = run_gpu_vast_builder(&tok_types, &tok_starts, &tok_lens);

    assert_eq!(
        gpu, expected,
        "GPU VAST builder must match CPU for deeply nested declarator"
    );
    // Delimiter sanity: the first LPAREN's first_child is the next token
    // and its next_sibling is the trailing LBRACKET (idx 14) because both
    // are top-level children.
    assert_vast_row(&gpu, 1, TOK_LPAREN, u32::MAX, 2, 14);
}

#[test]
fn gpu_parity_vast_builder_designated_init() {
    let (tok_types, tok_starts, tok_lens) = fixture_nested_designated_init();
    let expected = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let gpu = run_gpu_vast_builder(&tok_types, &tok_starts, &tok_lens);

    assert_eq!(
        gpu, expected,
        "GPU VAST builder must match CPU for designated initializer"
    );
    // Outer brace first_child points at first designator; next_sibling is
    // the trailing SEMICOLON (idx 30) because both are top-level.
    assert_vast_row(&gpu, 5, TOK_LBRACE, u32::MAX, 6, 30);
}

// ---------------------------------------------------------------------------
// GPU parity for PG lowering
// ---------------------------------------------------------------------------

