#[test]
fn function_pointer_table_gpu_pg_lower_matches_cpu() {
    let fix = fixture_function_pointer_table();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed, node_count_from_vast(&typed));
    assert_eq!(
        gpu, expected,
        "PG lowerer parity for function pointer table"
    );
}

// ---------------------------------------------------------------------------
// Tests  -  nested anonymous union
// ---------------------------------------------------------------------------

#[test]
fn nested_anonymous_union_classifies_union_and_struct_rows() {
    let fix = fixture_nested_anonymous_union();
    assert_full_pipeline_parity(&fix, "nested_anonymous_union");

    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);

    let unions = row_indices(&typed, C_AST_KIND_UNION_DECL);
    assert_ne!(unions.len(), 0,
        "anonymous union must produce UNION_DECL"
    );

    let structs = row_indices(&typed, C_AST_KIND_STRUCT_DECL);
    assert_ne!(structs.len(), 0,
        "outer struct X must produce STRUCT_DECL"
    );

    // Array declarator for char c[4]
    let arrays = row_indices(&typed, C_AST_KIND_ARRAY_DECL);
    assert_ne!(arrays.len(), 0, "char c[4] must produce ARRAY_DECL");
}

#[test]
fn nested_anonymous_union_gpu_pg_lower_matches_cpu() {
    let fix = fixture_nested_anonymous_union();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed, node_count_from_vast(&typed));
    assert_eq!(
        gpu, expected,
        "PG lowerer parity for nested anonymous union"
    );
}

// ---------------------------------------------------------------------------
// Tests  -  compound literal in array initializer
// ---------------------------------------------------------------------------

#[test]
fn compound_literal_in_array_init_classifies_compound_literal_expr() {
    let fix = fixture_compound_literal_in_array_init();
    assert_full_pipeline_parity(&fix, "compound_literal_in_array_init");

    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);

    let compounds = row_indices(&typed, C_AST_KIND_COMPOUND_LITERAL_EXPR);
    assert_eq!(
        compounds.len(),
        2,
        "two compound literals must produce two COMPOUND_LITERAL_EXPR rows, got {:?}",
        compounds
    );

    let inits = row_indices(&typed, C_AST_KIND_INITIALIZER_LIST);
    assert!(
        !inits.is_empty(),
        "outer array init must produce at least one INITIALIZER_LIST row"
    );
}

#[test]
fn compound_literal_in_array_init_gpu_pg_lower_matches_cpu() {
    let fix = fixture_compound_literal_in_array_init();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed, node_count_from_vast(&typed));
    assert_eq!(
        gpu, expected,
        "PG lowerer parity for compound literal in array init"
    );
}

// ---------------------------------------------------------------------------
// Tests  -  designated range initializer
// ---------------------------------------------------------------------------

#[test]
fn designated_range_initializer_classifies_range_designator_or_fallback() {
    let fix = fixture_designated_range_initializer();
    assert_full_pipeline_parity(&fix, "designated_range_initializer");

    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    let inits = row_indices(&typed, C_AST_KIND_INITIALIZER_LIST);
    assert!(
        !inits.is_empty(),
        "range-designated initializer must still produce INITIALIZER_LIST"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ARRAY_SUBSCRIPT_EXPR),
        vec![7, 15],
        "both designator brackets must produce ARRAY_SUBSCRIPT_EXPR rows"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_RANGE_DESIGNATOR_EXPR),
        vec![9],
        "range-designated initializer must produce RANGE_DESIGNATOR_EXPR for ellipsis"
    );
}

#[test]
fn designated_range_initializer_gpu_pg_lower_matches_cpu() {
    let fix = fixture_designated_range_initializer();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed, node_count_from_vast(&typed));
    assert_eq!(
        gpu, expected,
        "PG lowerer parity for designated range initializer"
    );
}

// ---------------------------------------------------------------------------
// Tests  -  typedef struct inline
// ---------------------------------------------------------------------------

#[test]
fn typedef_struct_inline_classifies_typedef_and_usage() {
    let fix = fixture_typedef_struct_inline();
    assert_full_pipeline_parity(&fix, "typedef_struct_inline");

    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);

    assert_eq!(
        kind_at(&typed, 0),
        C_AST_KIND_TYPEDEF_DECL,
        "typedef keyword must classify as TYPEDEF_DECL"
    );

    let name_t = lexeme_indices(&fix, "name_t");
    assert_eq!(name_t.len(), 2, "name_t appears in decl and use");

    // The second occurrence is the variable declaration using the typedef
    assert_eq!(
        kind_at(&typed, name_t[1]),
        node_kind::VARIABLE,
        "name_t x must classify x as VARIABLE"
    );
}

#[test]
fn typedef_struct_inline_gpu_pg_lower_matches_cpu() {
    let fix = fixture_typedef_struct_inline();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed, node_count_from_vast(&typed));
    assert_eq!(gpu, expected, "PG lowerer parity for typedef struct inline");
}
