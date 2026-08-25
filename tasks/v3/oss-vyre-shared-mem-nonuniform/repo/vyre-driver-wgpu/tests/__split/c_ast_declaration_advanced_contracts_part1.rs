use super::*;

#[test]
fn cpu_nested_struct_union_enum_kinds() {
    let fix = fixture_nested_struct_union_enum();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        row_indices(&typed, C_AST_KIND_STRUCT_DECL),
        vec![0, 5],
        "outer and inner struct keywords must classify as STRUCT_DECL"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_UNION_DECL),
        vec![3],
        "union keyword must classify as UNION_DECL"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ENUM_DECL),
        vec![19],
        "enum keyword must classify as ENUM_DECL"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ENUMERATOR_DECL),
        vec![21, 25],
        "enumerators A and B must classify as ENUMERATOR_DECL"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_FIELD_DECL),
        vec![8, 11, 14, 17, 29],
        "x, s, y, u, and e must classify as FIELD_DECL"
    );
}

#[test]
fn cpu_anonymous_struct_union_members() {
    let fix = fixture_anonymous_struct_union();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        row_indices(&typed, C_AST_KIND_UNION_DECL),
        vec![2],
        "anonymous union must classify as UNION_DECL"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_FIELD_DECL),
        vec![5, 8, 13],
        "i, f, and tag must classify as FIELD_DECL"
    );
    // The anonymous union has no declarator after its closing brace;
    // the semicolon after the brace must remain raw syntax.
    assert_eq!(
        kind_at(&typed, 11),
        0,
        "semicolon after anonymous union brace must stay raw"
    );
}

// ---------------------------------------------------------------------------
// CPU reference contracts  -  typedefs with multiple declarators
// ---------------------------------------------------------------------------

#[test]
fn cpu_typedef_multiple_complex_declarators() {
    let fix = fixture_typedef_multiple_declarators();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);

    assert_eq!(
        kind_at(&typed, 0),
        C_AST_KIND_TYPEDEF_DECL,
        "typedef keyword must be TYPEDEF_DECL"
    );
    assert_eq!(
        kind_at(&typed, 1),
        C_AST_KIND_STRUCT_DECL,
        "struct keyword must be STRUCT_DECL"
    );

    // Node declarator (index 8) and NodePtr declarator (index 11)
    assert_ne!(
        flags_at(&annotated, 8) & TYPEDEF_FLAG_DECL,
        0,
        "typedef name Node must carry TYPEDEF_FLAG_DECL"
    );
    assert_ne!(
        flags_at(&annotated, 11) & TYPEDEF_FLAG_DECL,
        0,
        "typedef name NodePtr must carry TYPEDEF_FLAG_DECL"
    );

    // The star before NodePtr must be a pointer declarator.
    assert_eq!(
        kind_at(&typed, 10),
        C_AST_KIND_POINTER_DECL,
        "star before NodePtr must be POINTER_DECL"
    );
}

// ---------------------------------------------------------------------------
// CPU reference contracts  -  deeply nested pointers
// ---------------------------------------------------------------------------

#[test]
fn cpu_deeply_nested_pointer_with_qualifiers() {
    let fix = fixture_deeply_nested_pointer();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    // All three stars must be POINTER_DECL
    assert_eq!(
        row_indices(&typed, C_AST_KIND_POINTER_DECL),
        vec![2, 4, 6],
        "triple-star must produce three POINTER_DECL rows"
    );

    // Qualifiers must stay raw syntax
    assert_eq!(kind_at(&typed, 0), 0, "const specifier must stay raw");
    assert_eq!(
        kind_at(&typed, 3),
        0,
        "interleaved const qualifier must stay raw"
    );
    assert_eq!(
        kind_at(&typed, 5),
        0,
        "interleaved volatile qualifier must stay raw"
    );
    assert_eq!(
        kind_at(&typed, 7),
        0,
        "interleaved restrict qualifier must stay raw"
    );

    assert_eq!(
        kind_at(&typed, 8),
        node_kind::VARIABLE,
        "identifier p must classify as VARIABLE"
    );
}

// ---------------------------------------------------------------------------
// CPU reference contracts  -  storage classes and qualifiers
// ---------------------------------------------------------------------------

#[test]
fn cpu_storage_class_combinations_stay_raw() {
    let fix = fixture_storage_class_combinations();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    // static inline int f(void);
    assert_eq!(kind_at(&typed, 0), 0, "static must remain raw syntax");
    assert_eq!(kind_at(&typed, 1), 0, "inline must remain raw syntax");
    assert_eq!(
        kind_at(&typed, 2),
        0,
        "int specifier must remain raw syntax"
    );
    assert_eq!(
        kind_at(&typed, 3),
        node_kind::FUNCTION_DECL,
        "f must classify as FUNCTION_DECL"
    );
    assert_eq!(
        kind_at(&typed, 4),
        C_AST_KIND_FUNCTION_DECLARATOR,
        "parameter list of f must be FUNCTION_DECLARATOR"
    );

    // extern register int x;
    assert_eq!(kind_at(&typed, 8), 0, "extern must remain raw syntax");
    assert_eq!(kind_at(&typed, 9), 0, "register must remain raw syntax");
    assert_eq!(
        kind_at(&typed, 11),
        node_kind::VARIABLE,
        "x must classify as VARIABLE"
    );

    // _Thread_local _Atomic int y;
    assert_eq!(
        kind_at(&typed, 13),
        0,
        "_Thread_local must remain raw syntax"
    );
    assert_eq!(kind_at(&typed, 14), 0, "_Atomic must remain raw syntax");
    assert_eq!(
        kind_at(&typed, 16),
        node_kind::VARIABLE,
        "y must classify as VARIABLE"
    );
}

// ---------------------------------------------------------------------------
// CPU reference contracts  -  bit-fields in nested structs
// ---------------------------------------------------------------------------

#[test]
fn cpu_bitfield_nested_struct_kinds() {
    let fix = fixture_bitfield_nested_struct();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        row_indices(&typed, C_AST_KIND_BIT_FIELD_DECL),
        vec![4, 11, 17],
        "named bitfield a, named bitfield b, and unnamed zero-width bitfield must classify"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_FIELD_DECL),
        vec![21],
        "inner field declarator must classify as FIELD_DECL"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_STRUCT_DECL),
        vec![0, 8],
        "outer and inner struct keywords must classify as STRUCT_DECL"
    );
}

// ---------------------------------------------------------------------------
// CPU reference contracts  -  GNU attributes on fields and typedefs
// ---------------------------------------------------------------------------

#[test]
fn cpu_gnu_attribute_on_field_and_typedef() {
    let fix = fixture_gnu_attribute_field_and_typedef();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    // First __attribute__ on struct field
    assert_eq!(
        row_indices(&typed, C_AST_KIND_GNU_ATTRIBUTE),
        vec![2, 18],
        "both __attribute__ tokens must classify as GNU_ATTRIBUTE"
    );
    // The classifier may not resolve specific attribute-name kinds for
    // attributes attached to struct fields or typedefs; the GPU parity
    // tests already prove CPU/GPU agreement on the current shape.
    assert_eq!(
        kind_at(&typed, 12),
        C_AST_KIND_FIELD_DECL,
        "x inside struct must classify as FIELD_DECL"
    );

    // typedef declarator
    assert_eq!(
        kind_at(&typed, 16),
        C_AST_KIND_TYPEDEF_DECL,
        "typedef keyword must classify as TYPEDEF_DECL"
    );
    assert_eq!(
        kind_at(&typed, 24),
        node_kind::VARIABLE,
        "packed_int typedef declarator must classify as VARIABLE"
    );
}

// ---------------------------------------------------------------------------
// CPU reference contracts  -  function pointer to pointer
// ---------------------------------------------------------------------------

#[test]
fn cpu_function_pointer_to_pointer_kinds() {
    let fix = fixture_function_pointer_to_pointer();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        row_indices(&typed, C_AST_KIND_POINTER_DECL),
        vec![2, 3],
        "both stars in (**fp) must be POINTER_DECL"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_FUNCTION_DECLARATOR),
        vec![6],
        "parameter list must be FUNCTION_DECLARATOR"
    );
    assert_eq!(
        kind_at(&typed, 4),
        node_kind::VARIABLE,
        "fp must classify as VARIABLE"
    );
}

// ---------------------------------------------------------------------------
// CPU reference contracts  -  array of function pointers with qualifiers
// ---------------------------------------------------------------------------

#[test]
fn cpu_array_of_function_pointers_qualified_kinds() {
    let fix = fixture_array_of_function_pointers_qualified();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        row_indices(&typed, C_AST_KIND_POINTER_DECL),
        vec![2, 13],
        "outer declarator star and parameter pointer star must be POINTER_DECL"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ARRAY_DECL),
        vec![4],
        "handlers brackets must be ARRAY_DECL"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_FUNCTION_DECLARATOR),
        vec![8],
        "parameter list must be FUNCTION_DECLARATOR"
    );

    // Parameter qualifiers must stay raw
    assert_eq!(kind_at(&typed, 11), 0, "const in parameter must stay raw");
    assert_eq!(
        kind_at(&typed, 14),
        0,
        "restrict in parameter must stay raw"
    );
}

// ---------------------------------------------------------------------------
// PG lowering preservation contracts
// ---------------------------------------------------------------------------

#[test]
fn pg_lower_preserves_nested_struct_union_enum_rows() {
    let fix = fixture_nested_struct_union_enum();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let pg = reference_ast_to_pg_nodes(&typed);

    for idx in [0usize, 3, 5, 8, 14, 19, 21, 25, 30] {
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
fn pg_lower_preserves_typedef_multiple_declarator_rows() {
    let fix = fixture_typedef_multiple_declarators();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let pg = reference_ast_to_pg_nodes(&typed);

    for idx in [0usize, 1, 8, 10, 11] {
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
fn pg_lower_preserves_deeply_nested_pointer_rows() {
    let fix = fixture_deeply_nested_pointer();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let pg = reference_ast_to_pg_nodes(&typed);

    for idx in [2usize, 4, 6, 8] {
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
fn pg_lower_preserves_storage_class_rows() {
    let fix = fixture_storage_class_combinations();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let pg = reference_ast_to_pg_nodes(&typed);

    for idx in [3usize, 4, 11, 16] {
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

