#[test]
fn cpu_asm_alias_classifies() {
    let fix = fixture_asm_alias();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);

    assert_eq!(
        kind_at(&typed, 1),
        node_kind::FUNCTION_DECL,
        "function name `foo` must classify as FUNCTION_DECL"
    );
    // Desired behaviour: the asm alias is a first-class node.
    assert_eq!(
        row_indices(&typed, C_AST_KIND_INLINE_ASM),
        vec![5],
        "asm alias must classify as INLINE_ASM (or a dedicated alias kind)"
    );
    assert_eq!(
        kind_at(&typed, 7),
        C_AST_KIND_ASM_TEMPLATE,
        "asm alias string must classify as ASM_TEMPLATE"
    );
}

#[test]
fn cpu_mixed_designated_and_plain_initializer() {
    let fix = fixture_mixed_designated_and_plain_init();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        row_indices(&typed, C_AST_KIND_INITIALIZER_LIST),
        vec![4],
        "brace must be an initializer list"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_MEMBER_ACCESS_EXPR),
        vec![7],
        "dot designator must classify as MEMBER_ACCESS_EXPR"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ASSIGN_EXPR),
        vec![9],
        "declaration initializer `=` is not an assignment expression; only the designator `=` is"
    );
    let literals = row_indices(&typed, node_kind::LITERAL);
    assert!(literals.contains(&5), "plain literal `1` must be a LITERAL");
    assert!(
        literals.contains(&10),
        "designated value `2` must be a LITERAL"
    );
    assert!(
        literals.contains(&12),
        "trailing plain literal `3` must be a LITERAL"
    );
}

#[test]
fn cpu_incomplete_array_initializer() {
    let fix = fixture_incomplete_array_init();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        row_indices(&typed, C_AST_KIND_INITIALIZER_LIST),
        vec![6],
        "incomplete initializer brace must still be an initializer list"
    );
    let literals = row_indices(&typed, node_kind::LITERAL);
    assert!(literals.contains(&7), "first element `1` must be a LITERAL");
    assert!(
        literals.contains(&9),
        "second element `2` must be a LITERAL"
    );
}

#[test]
fn cpu_function_pointer_typedef_usage() {
    let fix = fixture_function_pointer_typedef_usage();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);

    assert_eq!(
        kind_at(&typed, 0),
        C_AST_KIND_TYPEDEF_DECL,
        "typedef keyword must classify as TYPEDEF_DECL"
    );
    assert_ne!(
        flags_at(&annotated, 4) & TYPEDEF_FLAG_DECL,
        0,
        "function-pointer typedef name `fn_t` must carry TYPEDEF_FLAG_DECL"
    );
    assert_ne!(
        flags_at(&annotated, 10) & TYPEDEF_FLAG_VISIBLE,
        0,
        "later use of `fn_t` must carry TYPEDEF_FLAG_VISIBLE"
    );
    assert_eq!(
        kind_at(&typed, 11),
        node_kind::VARIABLE,
        "`f` declared via typedef must classify as VARIABLE"
    );
}

// ---------------------------------------------------------------------------
// PG lowering preservation contracts
// ---------------------------------------------------------------------------

#[test]
fn pg_lower_preserves_enum_attribute_kinds() {
    let fix = fixture_enum_with_attribute();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let pg = reference_ast_to_pg_nodes(&typed);

    for idx in [0usize, 1, 4, 9, 11] {
        assert_pg_preserves_row(
            &typed,
            &pg,
            &fix.tok_starts,
            &fix.tok_lens,
            idx,
            kind_at(&typed, idx),
        );
    }
}

#[test]
fn pg_lower_preserves_parameter_attribute_kinds() {
    let fix = fixture_parameter_with_attribute();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let pg = reference_ast_to_pg_nodes(&typed);

    for idx in [1usize, 2, 4, 7, 10] {
        assert_pg_preserves_row(
            &typed,
            &pg,
            &fix.tok_starts,
            &fix.tok_lens,
            idx,
            kind_at(&typed, idx),
        );
    }
}

#[test]
fn pg_lower_preserves_mixed_initializer_kinds() {
    let fix = fixture_mixed_designated_and_plain_init();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let pg = reference_ast_to_pg_nodes(&typed);

    for idx in [4usize, 7, 9] {
        assert_pg_preserves_row(
            &typed,
            &pg,
            &fix.tok_starts,
            &fix.tok_lens,
            idx,
            kind_at(&typed, idx),
        );
    }
}

#[test]
fn pg_lower_preserves_asm_alias_kinds() {
    let fix = fixture_asm_alias();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let pg = reference_ast_to_pg_nodes(&typed);

    for idx in [1usize, 5, 7] {
        assert_pg_preserves_row(
            &typed,
            &pg,
            &fix.tok_starts,
            &fix.tok_lens,
            idx,
            kind_at(&typed, idx),
        );
    }
}

// ---------------------------------------------------------------------------
// GPU parity contracts
// ---------------------------------------------------------------------------

#[test]
fn gpu_parity_enum_with_attribute_classifier() {
    let fix = fixture_enum_with_attribute();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let expected = reference_c11_classify_vast_node_kinds(&annotated);
    let gpu = run_gpu_classifier(&annotated, node_count_from_vast(&annotated));
    assert_eq!(
        gpu, expected,
        "GPU classifier must match CPU for enum with attribute"
    );
}

#[test]
fn gpu_parity_parameter_with_attribute_classifier() {
    let fix = fixture_parameter_with_attribute();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let expected = reference_c11_classify_vast_node_kinds(&annotated);
    let gpu = run_gpu_classifier(&annotated, node_count_from_vast(&annotated));
    assert_eq!(
        gpu, expected,
        "GPU classifier must match CPU for parameter with attribute"
    );
}

#[test]
fn gpu_parity_mixed_initializer_classifier() {
    let fix = fixture_mixed_designated_and_plain_init();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let expected = reference_c11_classify_vast_node_kinds(&raw);
    let gpu = run_gpu_classifier(&raw, node_count_from_vast(&raw));
    assert_eq!(
        gpu, expected,
        "GPU classifier must match CPU for mixed initializer"
    );
}

#[test]
fn gpu_parity_asm_alias_classifier() {
    let fix = fixture_asm_alias();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let expected = reference_c11_classify_vast_node_kinds(&annotated);
    let gpu = run_gpu_classifier(&annotated, node_count_from_vast(&annotated));
    assert_eq!(gpu, expected, "GPU classifier must match CPU for asm alias");
}

#[test]
fn gpu_parity_enum_with_attribute_pg_lower() {
    let fix = fixture_enum_with_attribute();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed, node_count_from_vast(&typed));
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for enum with attribute"
    );
}

#[test]
fn gpu_parity_parameter_with_attribute_pg_lower() {
    let fix = fixture_parameter_with_attribute();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed, node_count_from_vast(&typed));
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for parameter with attribute"
    );
}
