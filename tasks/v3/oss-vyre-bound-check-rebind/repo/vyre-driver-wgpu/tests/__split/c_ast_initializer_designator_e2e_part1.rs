use super::*;

#[test]
fn cpu_reference_array_initializer_list_materialises_initializer_list() {
    let (tok_types, tok_starts, tok_lens) = fixture_array_initializer_list();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    let lists = typed_indices(&typed, C_AST_KIND_INITIALIZER_LIST);
    assert_eq!(lists, vec![6], "array initializer list must be one node");

    let literals = typed_indices(&typed, node_kind::LITERAL);
    assert!(
        literals.len() >= 3,
        "three integer literals must appear; got {literals:?}"
    );
}

#[test]
fn cpu_reference_struct_initializer_list_materialises_fields_and_list() {
    let (tok_types, tok_starts, tok_lens) = fixture_struct_initializer_list();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    let lists = typed_indices(&typed, C_AST_KIND_INITIALIZER_LIST);
    assert_eq!(lists, vec![4], "struct initializer list must be one node");

    let fields = typed_indices(&typed, C_AST_KIND_FIELD_DECL);
    assert!(
        fields.is_empty(),
        "field decls only appear inside record bodies, not initializers"
    );
}

#[test]
fn cpu_reference_union_designated_init_materialises_member_access() {
    let (tok_types, tok_starts, tok_lens) = fixture_union_designated_init();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    let lists = typed_indices(&typed, C_AST_KIND_INITIALIZER_LIST);
    assert_eq!(lists, vec![4], "union initializer list must be one node");

    let designators = typed_indices(&typed, C_AST_KIND_MEMBER_ACCESS_EXPR);
    assert!(
        !designators.is_empty(),
        "dot designator .i must surface; got {designators:?}"
    );
}

#[test]
fn cpu_reference_enum_declaration_and_init_preserves_enumerator_kinds() {
    let (tok_types, tok_starts, tok_lens) = fixture_enum_with_initializer();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    let enumerators = typed_indices(&typed, C_AST_KIND_ENUMERATOR_DECL);
    assert_eq!(
        enumerators,
        vec![3, 7, 9],
        "RED, GREEN, BLUE must all be enumerator decls"
    );

    let assigns = typed_indices(&typed, C_AST_KIND_ASSIGN_EXPR);
    assert!(
        assigns.contains(&4),
        "RED = 0 must be an assignment expression"
    );
    assert!(
        assigns.contains(&10),
        "BLUE = 2 must be an assignment expression"
    );

    // Second declaration: enum Color c = GREEN;
    let vars = typed_indices(&typed, node_kind::VARIABLE);
    assert!(vars.contains(&16), "variable c must type as VARIABLE");
}

#[test]
fn cpu_reference_nested_designator_mixed_materialises_all_lists() {
    let (tok_types, tok_starts, tok_lens) = fixture_nested_designator_mixed();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    let init_lists = typed_indices(&typed, C_AST_KIND_INITIALIZER_LIST);
    assert!(
        init_lists.len() >= 3,
        "outer, middle, and inner initializer lists must all materialise; got {init_lists:?}"
    );

    let array_designators = typed_indices(&typed, C_AST_KIND_ARRAY_SUBSCRIPT_EXPR);
    assert!(
        array_designators.len() >= 3,
        "array designators [0], [1], [2] must surface; got {array_designators:?}"
    );

    let member_access = typed_indices(&typed, C_AST_KIND_MEMBER_ACCESS_EXPR);
    assert!(
        member_access.len() >= 4,
        "dot designators .name, .dims, .x, .y must surface; got {member_access:?}"
    );
}

#[test]
fn cpu_reference_compound_literal_materialises_initializer_list() {
    let (tok_types, tok_starts, tok_lens) = fixture_compound_literal_expr();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    let compounds = typed_indices(&typed, C_AST_KIND_COMPOUND_LITERAL_EXPR);
    assert_eq!(
        compounds,
        vec![4],
        "compound literal expr must be a first-class node"
    );

    let lists = typed_indices(&typed, C_AST_KIND_INITIALIZER_LIST);
    assert_eq!(
        lists,
        vec![8],
        "compound literal body must be an initializer list"
    );

    let assigns = typed_indices(&typed, C_AST_KIND_ASSIGN_EXPR);
    assert!(
        assigns.len() >= 2,
        ".w = 10 and .h = 20 must be assignment exprs; got {assigns:?}"
    );
}

#[test]
fn cpu_reference_compound_literal_in_call_materialises_call_and_compound() {
    let (tok_types, tok_starts, tok_lens) = fixture_compound_literal_in_call();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    let calls = typed_indices(&typed, node_kind::CALL);
    assert_ne!(calls.len(), 0,
        "function call f(...) must materialise; got {calls:?}"
    );

    let compounds = typed_indices(&typed, C_AST_KIND_COMPOUND_LITERAL_EXPR);
    assert_ne!(compounds.len(), 0,
        "compound literal inside call must materialise; got {compounds:?}"
    );
}

// ---------------------------------------------------------------------------
// PG lowering preservation tests
// ---------------------------------------------------------------------------

#[test]
fn pg_lowering_preserves_spans_and_tree_links_for_array_initializer() {
    let (tok_types, tok_starts, tok_lens) = fixture_array_initializer_list();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let pg = run_reference_pg_lower(&typed);

    for idx in typed_indices(&typed, C_AST_KIND_INITIALIZER_LIST) {
        assert_pg_preserves_kind_span_and_links(
            &typed,
            &pg,
            &tok_starts,
            &tok_lens,
            idx,
            C_AST_KIND_INITIALIZER_LIST,
        );
    }
    for idx in typed_indices(&typed, node_kind::LITERAL) {
        assert_pg_preserves_kind_span_and_links(
            &typed,
            &pg,
            &tok_starts,
            &tok_lens,
            idx,
            node_kind::LITERAL,
        );
    }
}

#[test]
fn pg_lowering_preserves_spans_and_tree_links_for_struct_initializer() {
    let (tok_types, tok_starts, tok_lens) = fixture_struct_initializer_list();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let pg = run_reference_pg_lower(&typed);

    for idx in typed_indices(&typed, C_AST_KIND_INITIALIZER_LIST) {
        assert_pg_preserves_kind_span_and_links(
            &typed,
            &pg,
            &tok_starts,
            &tok_lens,
            idx,
            C_AST_KIND_INITIALIZER_LIST,
        );
    }
    for idx in typed_indices(&typed, node_kind::LITERAL) {
        assert_pg_preserves_kind_span_and_links(
            &typed,
            &pg,
            &tok_starts,
            &tok_lens,
            idx,
            node_kind::LITERAL,
        );
    }
}

#[test]
fn pg_lowering_preserves_spans_and_tree_links_for_union_designated_init() {
    let (tok_types, tok_starts, tok_lens) = fixture_union_designated_init();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let pg = run_reference_pg_lower(&typed);

    for idx in typed_indices(&typed, C_AST_KIND_INITIALIZER_LIST) {
        assert_pg_preserves_kind_span_and_links(
            &typed,
            &pg,
            &tok_starts,
            &tok_lens,
            idx,
            C_AST_KIND_INITIALIZER_LIST,
        );
    }
    for idx in typed_indices(&typed, C_AST_KIND_MEMBER_ACCESS_EXPR) {
        assert_pg_preserves_kind_span_and_links(
            &typed,
            &pg,
            &tok_starts,
            &tok_lens,
            idx,
            C_AST_KIND_MEMBER_ACCESS_EXPR,
        );
    }
    for idx in typed_indices(&typed, C_AST_KIND_ASSIGN_EXPR) {
        assert_pg_preserves_kind_span_and_links(
            &typed,
            &pg,
            &tok_starts,
            &tok_lens,
            idx,
            C_AST_KIND_ASSIGN_EXPR,
        );
    }
}

#[test]
fn pg_lowering_preserves_spans_and_tree_links_for_enum_declaration() {
    let (tok_types, tok_starts, tok_lens) = fixture_enum_with_initializer();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let pg = run_reference_pg_lower(&typed);

    for idx in typed_indices(&typed, C_AST_KIND_ENUMERATOR_DECL) {
        assert_pg_preserves_kind_span_and_links(
            &typed,
            &pg,
            &tok_starts,
            &tok_lens,
            idx,
            C_AST_KIND_ENUMERATOR_DECL,
        );
    }
    for idx in typed_indices(&typed, C_AST_KIND_ASSIGN_EXPR) {
        assert_pg_preserves_kind_span_and_links(
            &typed,
            &pg,
            &tok_starts,
            &tok_lens,
            idx,
            C_AST_KIND_ASSIGN_EXPR,
        );
    }
}

#[test]
fn pg_lowering_preserves_spans_and_tree_links_for_nested_designators() {
    let (tok_types, tok_starts, tok_lens) = fixture_nested_designator_mixed();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let pg = run_reference_pg_lower(&typed);

    for idx in typed_indices(&typed, C_AST_KIND_INITIALIZER_LIST) {
        assert_pg_preserves_kind_span_and_links(
            &typed,
            &pg,
            &tok_starts,
            &tok_lens,
            idx,
            C_AST_KIND_INITIALIZER_LIST,
        );
    }
    for idx in typed_indices(&typed, C_AST_KIND_MEMBER_ACCESS_EXPR) {
        assert_pg_preserves_kind_span_and_links(
            &typed,
            &pg,
            &tok_starts,
            &tok_lens,
            idx,
            C_AST_KIND_MEMBER_ACCESS_EXPR,
        );
    }
    for idx in typed_indices(&typed, C_AST_KIND_ARRAY_SUBSCRIPT_EXPR) {
        assert_pg_preserves_kind_span_and_links(
            &typed,
            &pg,
            &tok_starts,
            &tok_lens,
            idx,
            C_AST_KIND_ARRAY_SUBSCRIPT_EXPR,
        );
    }
}

#[test]
fn pg_lowering_preserves_spans_and_tree_links_for_compound_literal() {
    let (tok_types, tok_starts, tok_lens) = fixture_compound_literal_expr();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let pg = run_reference_pg_lower(&typed);

    for idx in typed_indices(&typed, C_AST_KIND_COMPOUND_LITERAL_EXPR) {
        assert_pg_preserves_kind_span_and_links(
            &typed,
            &pg,
            &tok_starts,
            &tok_lens,
            idx,
            C_AST_KIND_COMPOUND_LITERAL_EXPR,
        );
    }
    for idx in typed_indices(&typed, C_AST_KIND_INITIALIZER_LIST) {
        assert_pg_preserves_kind_span_and_links(
            &typed,
            &pg,
            &tok_starts,
            &tok_lens,
            idx,
            C_AST_KIND_INITIALIZER_LIST,
        );
    }
    for idx in typed_indices(&typed, C_AST_KIND_ASSIGN_EXPR) {
        assert_pg_preserves_kind_span_and_links(
            &typed,
            &pg,
            &tok_starts,
            &tok_lens,
            idx,
            C_AST_KIND_ASSIGN_EXPR,
        );
    }
}

// ---------------------------------------------------------------------------
// GPU/CPU parity tests
// ---------------------------------------------------------------------------

#[test]
fn gpu_parity_pg_lower_array_initializer_list() {
    let (tok_types, tok_starts, tok_lens) = fixture_array_initializer_list();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let expected = run_reference_pg_lower(&typed);
    let gpu = run_gpu_pg_lower(&typed);
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for array initializer list"
    );
}

#[test]
fn gpu_parity_pg_lower_struct_initializer_list() {
    let (tok_types, tok_starts, tok_lens) = fixture_struct_initializer_list();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let expected = run_reference_pg_lower(&typed);
    let gpu = run_gpu_pg_lower(&typed);
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for struct initializer list"
    );
}

#[test]
fn gpu_parity_pg_lower_union_designated_init() {
    let (tok_types, tok_starts, tok_lens) = fixture_union_designated_init();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let expected = run_reference_pg_lower(&typed);
    let gpu = run_gpu_pg_lower(&typed);
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for union designated init"
    );
}

#[test]
fn gpu_parity_pg_lower_enum_declaration_and_init() {
    let (tok_types, tok_starts, tok_lens) = fixture_enum_with_initializer();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let expected = run_reference_pg_lower(&typed);
    let gpu = run_gpu_pg_lower(&typed);
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for enum declaration and init"
    );
}

