use super::*;

#[test]
fn cpu_nested_field_array_designator_materialises_all_kinds() {
    let (tok_types, tok_starts, tok_lens) = fixture_nested_field_array_designator();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        typed_indices(&typed, C_AST_KIND_INITIALIZER_LIST),
        vec![4, 16],
        "outer and inner initializer lists must materialise"
    );
    assert_eq!(
        typed_indices(&typed, C_AST_KIND_MEMBER_ACCESS_EXPR),
        vec![5, 13, 17],
        ".a, .b, .c designators must classify as MEMBER_ACCESS_EXPR"
    );
    assert_eq!(
        typed_indices(&typed, C_AST_KIND_ARRAY_SUBSCRIPT_EXPR),
        vec![7],
        "[0] array designator must classify as ARRAY_SUBSCRIPT_EXPR"
    );
    assert_eq!(
        typed_indices(&typed, C_AST_KIND_ASSIGN_EXPR),
        vec![10, 15, 19],
        "designator assignments must classify as ASSIGN_EXPR"
    );
    let literals = typed_indices(&typed, node_kind::LITERAL);
    assert!(literals.contains(&8), "index literal 0 must be LITERAL");
    assert!(literals.contains(&11), "value literal 1 must be LITERAL");
    assert!(literals.contains(&20), "value literal 2 must be LITERAL");

    // Declaration initializer `=` is suppressed.
    assert_ne!(
        kind_at(&typed, 3),
        C_AST_KIND_ASSIGN_EXPR,
        "declaration initializer `=` must not be ASSIGN_EXPR"
    );
}

#[test]
fn cpu_range_designator_materialises_range_expr() {
    let (tok_types, tok_starts, tok_lens) = fixture_range_designator_array();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        typed_indices(&typed, C_AST_KIND_RANGE_DESIGNATOR_EXPR),
        vec![9],
        "ellipsis ... must classify as RANGE_DESIGNATOR_EXPR"
    );
    assert_eq!(
        typed_indices(&typed, C_AST_KIND_INITIALIZER_LIST),
        vec![6],
        "brace must be an initializer list"
    );
    assert_eq!(
        typed_indices(&typed, C_AST_KIND_ARRAY_SUBSCRIPT_EXPR),
        vec![7, 15],
        "[0 ... 3] and [5] must classify as ARRAY_SUBSCRIPT_EXPR"
    );
    assert_eq!(
        typed_indices(&typed, C_AST_KIND_ASSIGN_EXPR),
        vec![12, 18],
        "designator assignments must classify as ASSIGN_EXPR"
    );
}

#[test]
fn cpu_union_field_designator_classifies() {
    let (tok_types, tok_starts, tok_lens) = fixture_union_field_designator();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        typed_indices(&typed, C_AST_KIND_UNION_DECL),
        vec![0],
        "union keyword must classify as UNION_DECL"
    );
    assert_eq!(
        typed_indices(&typed, C_AST_KIND_INITIALIZER_LIST),
        vec![4],
        "union initializer brace must be an initializer list"
    );
    assert_eq!(
        typed_indices(&typed, C_AST_KIND_MEMBER_ACCESS_EXPR),
        vec![5],
        ".f designator must classify as MEMBER_ACCESS_EXPR"
    );
    assert_eq!(
        typed_indices(&typed, C_AST_KIND_ASSIGN_EXPR),
        vec![7],
        "designator `=` must classify as ASSIGN_EXPR"
    );
    assert_ne!(
        kind_at(&typed, 3),
        C_AST_KIND_ASSIGN_EXPR,
        "declaration initializer `=` must not be ASSIGN_EXPR"
    );
}

#[test]
fn cpu_mixed_positional_designated_suppresses_declaration_assign() {
    let (tok_types, tok_starts, tok_lens) = fixture_mixed_positional_designated();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        typed_indices(&typed, C_AST_KIND_INITIALIZER_LIST),
        vec![4],
        "brace must be an initializer list"
    );
    assert_eq!(
        typed_indices(&typed, C_AST_KIND_MEMBER_ACCESS_EXPR),
        vec![7],
        ".b designator must classify as MEMBER_ACCESS_EXPR"
    );
    assert_eq!(
        typed_indices(&typed, C_AST_KIND_ASSIGN_EXPR),
        vec![9],
        "only designator `=` must be ASSIGN_EXPR; declaration `=` is suppressed"
    );
    let literals = typed_indices(&typed, node_kind::LITERAL);
    assert!(literals.contains(&5), "plain literal `1` must be LITERAL");
    assert!(
        literals.contains(&10),
        "designated value `2` must be LITERAL"
    );
    assert!(
        literals.contains(&12),
        "trailing plain literal `3` must be LITERAL"
    );
}

#[test]
fn cpu_compound_literal_nested_materialises() {
    let (tok_types, tok_starts, tok_lens) = fixture_compound_literal_nested();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        typed_indices(&typed, C_AST_KIND_COMPOUND_LITERAL_EXPR),
        vec![8],
        "compound literal (struct S){{...}} must be a first-class node"
    );
    assert_eq!(
        typed_indices(&typed, C_AST_KIND_INITIALIZER_LIST),
        vec![4, 12],
        "outer and compound-literal initializer lists must materialise"
    );
    assert_eq!(
        typed_indices(&typed, C_AST_KIND_MEMBER_ACCESS_EXPR),
        vec![5, 13],
        ".inner and .x designators must classify as MEMBER_ACCESS_EXPR"
    );
    assert_eq!(
        typed_indices(&typed, C_AST_KIND_ASSIGN_EXPR),
        vec![7, 15],
        "designator assignments must classify as ASSIGN_EXPR"
    );
    assert_eq!(
        typed_indices(&typed, C_AST_KIND_STRUCT_DECL),
        vec![0, 9],
        "both struct keywords must classify as STRUCT_DECL"
    );
}

#[test]
fn cpu_declaration_initializer_assignment_suppression() {
    let (tok_types, tok_starts, tok_lens) = fixture_assignment_suppression();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert!(
        typed_indices(&typed, C_AST_KIND_ASSIGN_EXPR).is_empty(),
        "declaration initializer `=` must be suppressed: no ASSIGN_EXPR nodes allowed"
    );
    assert_eq!(
        typed_indices(&typed, C_AST_KIND_INITIALIZER_LIST),
        vec![3],
        "brace initializer list must still materialise"
    );
    assert_eq!(
        typed_indices(&typed, node_kind::LITERAL),
        vec![4],
        "literal `1` must classify as LITERAL"
    );
}

#[test]
fn cpu_designator_assignment_classification() {
    let (tok_types, tok_starts, tok_lens) = fixture_designator_assignment_class();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_ne!(
        kind_at(&typed, 3),
        C_AST_KIND_ASSIGN_EXPR,
        "declaration initializer `=` must be suppressed"
    );
    assert_eq!(
        typed_indices(&typed, C_AST_KIND_ASSIGN_EXPR),
        vec![7],
        "designator `=` must classify as ASSIGN_EXPR"
    );
    assert_eq!(
        typed_indices(&typed, C_AST_KIND_MEMBER_ACCESS_EXPR),
        vec![5],
        ".a designator must classify as MEMBER_ACCESS_EXPR"
    );
}

#[test]
fn cpu_string_char_array_nested_initialization() {
    let (tok_types, tok_starts, tok_lens) = fixture_string_char_array_nested();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        typed_indices(&typed, C_AST_KIND_ARRAY_DECL),
        vec![5],
        "data[4] array declarator must classify as ARRAY_DECL"
    );
    assert_eq!(
        typed_indices(&typed, C_AST_KIND_FIELD_DECL),
        vec![4],
        "field `data` must classify as FIELD_DECL"
    );
    assert_eq!(
        typed_indices(&typed, C_AST_KIND_INITIALIZER_LIST),
        vec![12],
        "outer designated initializer brace must be an initializer list"
    );
    assert_eq!(
        typed_indices(&typed, C_AST_KIND_MEMBER_ACCESS_EXPR),
        vec![13],
        ".data designator must classify as MEMBER_ACCESS_EXPR"
    );
    assert_eq!(
        typed_indices(&typed, C_AST_KIND_ASSIGN_EXPR),
        vec![15],
        "designator `=` must classify as ASSIGN_EXPR"
    );
    let literals = typed_indices(&typed, node_kind::LITERAL);
    assert!(
        literals.contains(&16),
        "string literal must classify as LITERAL; got {literals:?}"
    );
}

// ---------------------------------------------------------------------------
// PG lowering preservation contracts
// ---------------------------------------------------------------------------

#[test]
fn pg_lower_preserves_nested_field_array_designator() {
    let (tok_types, tok_starts, tok_lens) = fixture_nested_field_array_designator();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let pg = run_reference_pg_lower(&typed);

    for idx in [4usize, 5, 7, 10, 13, 15, 16, 17, 19] {
        assert_pg_preserves_row(
            &typed,
            &pg,
            &tok_starts,
            &tok_lens,
            idx,
            kind_at(&typed, idx),
        );
    }
}

#[test]
fn pg_lower_preserves_range_designator_array() {
    let (tok_types, tok_starts, tok_lens) = fixture_range_designator_array();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let pg = run_reference_pg_lower(&typed);

    for idx in [6usize, 7, 9, 12, 15, 18] {
        assert_pg_preserves_row(
            &typed,
            &pg,
            &tok_starts,
            &tok_lens,
            idx,
            kind_at(&typed, idx),
        );
    }
}

#[test]
fn pg_lower_preserves_union_field_designator() {
    let (tok_types, tok_starts, tok_lens) = fixture_union_field_designator();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let pg = run_reference_pg_lower(&typed);

    for idx in [0usize, 4, 5, 7] {
        assert_pg_preserves_row(
            &typed,
            &pg,
            &tok_starts,
            &tok_lens,
            idx,
            kind_at(&typed, idx),
        );
    }
}

#[test]
fn pg_lower_preserves_mixed_positional_designated() {
    let (tok_types, tok_starts, tok_lens) = fixture_mixed_positional_designated();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let pg = run_reference_pg_lower(&typed);

    for idx in [4usize, 5, 7, 9, 10, 12] {
        assert_pg_preserves_row(
            &typed,
            &pg,
            &tok_starts,
            &tok_lens,
            idx,
            kind_at(&typed, idx),
        );
    }
}

#[test]
fn pg_lower_preserves_compound_literal_nested() {
    let (tok_types, tok_starts, tok_lens) = fixture_compound_literal_nested();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let pg = run_reference_pg_lower(&typed);

    for idx in [4usize, 5, 7, 8, 9, 12, 13, 15] {
        assert_pg_preserves_row(
            &typed,
            &pg,
            &tok_starts,
            &tok_lens,
            idx,
            kind_at(&typed, idx),
        );
    }
}

#[test]
fn pg_lower_preserves_assignment_suppression() {
    let (tok_types, tok_starts, tok_lens) = fixture_assignment_suppression();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let pg = run_reference_pg_lower(&typed);

    for idx in [0usize, 1, 3, 4] {
        assert_pg_preserves_row(
            &typed,
            &pg,
            &tok_starts,
            &tok_lens,
            idx,
            kind_at(&typed, idx),
        );
    }
}

#[test]
fn pg_lower_preserves_designator_assignment_class() {
    let (tok_types, tok_starts, tok_lens) = fixture_designator_assignment_class();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let pg = run_reference_pg_lower(&typed);

    for idx in [0usize, 4, 5, 7, 8] {
        assert_pg_preserves_row(
            &typed,
            &pg,
            &tok_starts,
            &tok_lens,
            idx,
            kind_at(&typed, idx),
        );
    }
}

#[test]
fn pg_lower_preserves_string_char_array_nested() {
    let (tok_types, tok_starts, tok_lens) = fixture_string_char_array_nested();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let pg = run_reference_pg_lower(&typed);

    for idx in [0usize, 3, 4, 5, 12, 13, 15, 16] {
        assert_pg_preserves_row(
            &typed,
            &pg,
            &tok_starts,
            &tok_lens,
            idx,
            kind_at(&typed, idx),
        );
    }
}

// ---------------------------------------------------------------------------
// GPU / CPU parity contracts
// ---------------------------------------------------------------------------

#[test]
fn gpu_parity_nested_field_array_designator() {
    let (tok_types, tok_starts, tok_lens) = fixture_nested_field_array_designator();
    assert_full_pipeline_parity(&tok_types, &tok_starts, &tok_lens, "nested_field_array");
}

