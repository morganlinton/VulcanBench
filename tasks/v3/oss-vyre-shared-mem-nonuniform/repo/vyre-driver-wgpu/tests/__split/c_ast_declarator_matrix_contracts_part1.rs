use super::*;

#[test]
fn cpu_pointer_to_array_classifies_correctly() {
    let fix = fixture_pointer_to_array();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        row_indices(&typed, C_AST_KIND_POINTER_DECL),
        vec![2],
        "inner * in `(*p)` must be POINTER_DECL"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ARRAY_DECL),
        vec![5],
        "outer [4] must be ARRAY_DECL"
    );
    assert_eq!(
        kind_at(&typed, 3),
        node_kind::VARIABLE,
        "`p` must classify as VARIABLE"
    );
    // Specifier propagation: int stays raw syntax (no declarator classification)
    assert_eq!(
        kind_at(&typed, 0),
        0,
        "type specifier `int` must remain raw syntax"
    );
}

#[test]
fn cpu_storage_class_multi_declarator_specifiers_stay_raw() {
    let fix = fixture_storage_class_multi_declarator();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    // static and const must stay raw syntax
    assert_eq!(kind_at(&typed, 0), 0, "`static` must remain raw syntax");
    assert_eq!(kind_at(&typed, 1), 0, "`const` must remain raw syntax");
    assert_eq!(kind_at(&typed, 2), 0, "`int` must remain raw syntax");

    // Declarator classification
    assert_eq!(
        kind_at(&typed, 3),
        C_AST_KIND_POINTER_DECL,
        "star before `p` must be POINTER_DECL"
    );
    assert_eq!(
        kind_at(&typed, 4),
        node_kind::VARIABLE,
        "`p` must classify as VARIABLE"
    );
    assert_eq!(
        kind_at(&typed, 5),
        0,
        "comma between declarators must stay raw"
    );
    assert_eq!(
        kind_at(&typed, 6),
        node_kind::VARIABLE,
        "`arr` must classify as VARIABLE"
    );
    assert_eq!(
        kind_at(&typed, 7),
        C_AST_KIND_ARRAY_DECL,
        "bracket after `arr` must be ARRAY_DECL"
    );
}

#[test]
fn cpu_parameter_array_static_restrict_stays_raw() {
    let fix = fixture_parameter_array_static_restrict();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        kind_at(&typed, 1),
        node_kind::FUNCTION_DECL,
        "`f` must classify as FUNCTION_DECL"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_FUNCTION_DECLARATOR),
        vec![2],
        "parameter list parens must be FUNCTION_DECLARATOR"
    );
    assert_eq!(
        kind_at(&typed, 4),
        node_kind::VARIABLE,
        "parameter `arr` must classify as VARIABLE"
    );
    assert_eq!(
        kind_at(&typed, 5),
        C_AST_KIND_ARRAY_DECL,
        "parameter array brackets must be ARRAY_DECL"
    );
    // static and restrict inside array brackets must stay raw
    assert_eq!(
        kind_at(&typed, 6),
        0,
        "`static` inside parameter array brackets must stay raw syntax"
    );
    assert_eq!(
        kind_at(&typed, 7),
        0,
        "`restrict` inside parameter array brackets must stay raw syntax"
    );
    assert_eq!(
        kind_at(&typed, 8),
        node_kind::LITERAL,
        "array size `10` must classify as LITERAL"
    );
}

#[test]
fn cpu_nested_typedef_complex_declarator_annotations() {
    let fix = fixture_nested_typedef_complex_declarator();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);

    // typedef int (*fn_t)(int);
    assert_eq!(
        kind_at(&typed, 0),
        C_AST_KIND_TYPEDEF_DECL,
        "typedef keyword must be TYPEDEF_DECL"
    );
    assert_ne!(
        flags_at(&annotated, 4) & TYPEDEF_FLAG_DECL,
        0,
        "function-pointer typedef name `fn_t` must carry TYPEDEF_FLAG_DECL"
    );

    // fn_t f;
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

#[test]
fn cpu_struct_tag_with_mixed_declarators() {
    let fix = fixture_struct_tag_with_mixed_declarators();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        row_indices(&typed, C_AST_KIND_STRUCT_DECL),
        vec![0],
        "`struct` must classify as STRUCT_DECL"
    );
    assert_eq!(
        kind_at(&typed, 4),
        C_AST_KIND_FIELD_DECL,
        "`x` inside struct body must be FIELD_DECL"
    );

    // Mixed declarators after struct definition
    assert_eq!(
        kind_at(&typed, 7),
        C_AST_KIND_POINTER_DECL,
        "star before `p` must be POINTER_DECL"
    );
    assert_eq!(
        kind_at(&typed, 8),
        node_kind::VARIABLE,
        "`p` must classify as VARIABLE"
    );
    assert_eq!(
        kind_at(&typed, 9),
        0,
        "comma between declarators must stay raw"
    );
    assert_eq!(
        kind_at(&typed, 10),
        node_kind::VARIABLE,
        "`arr` must classify as VARIABLE"
    );
    assert_eq!(
        kind_at(&typed, 11),
        C_AST_KIND_ARRAY_DECL,
        "bracket after `arr` must be ARRAY_DECL"
    );
}

#[test]
fn cpu_union_tag_with_mixed_declarators() {
    let fix = fixture_union_tag_with_mixed_declarators();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        row_indices(&typed, C_AST_KIND_UNION_DECL),
        vec![0],
        "`union` must classify as UNION_DECL"
    );
    assert_eq!(
        kind_at(&typed, 4),
        C_AST_KIND_FIELD_DECL,
        "`c` inside union body must be FIELD_DECL"
    );
    assert_eq!(
        kind_at(&typed, 7),
        C_AST_KIND_FIELD_DECL,
        "`i` inside union body must be FIELD_DECL"
    );

    // Mixed declarators after union definition
    assert_eq!(
        kind_at(&typed, 10),
        node_kind::VARIABLE,
        "`u` must classify as VARIABLE"
    );
    assert_eq!(
        kind_at(&typed, 12),
        C_AST_KIND_POINTER_DECL,
        "star before `up` must be POINTER_DECL"
    );
    assert_eq!(
        kind_at(&typed, 13),
        node_kind::VARIABLE,
        "`up` must classify as VARIABLE"
    );
}

#[test]
fn cpu_enum_tag_with_mixed_declarators() {
    let fix = fixture_enum_tag_with_mixed_declarators();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        row_indices(&typed, C_AST_KIND_ENUM_DECL),
        vec![0],
        "`enum` must classify as ENUM_DECL"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ENUMERATOR_DECL),
        vec![3, 5],
        "enumerators `ON` and `OFF` must classify as ENUMERATOR_DECL"
    );

    // Mixed declarators after enum definition
    assert_eq!(
        kind_at(&typed, 7),
        node_kind::VARIABLE,
        "`ev` must classify as VARIABLE"
    );
    assert_eq!(
        kind_at(&typed, 9),
        C_AST_KIND_POINTER_DECL,
        "star before `ep` must be POINTER_DECL"
    );
    assert_eq!(
        kind_at(&typed, 10),
        node_kind::VARIABLE,
        "`ep` must classify as VARIABLE"
    );
}

#[test]
fn cpu_heavy_qualifiers_and_storage_multi_decl_specifiers_stay_raw() {
    let fix = fixture_heavy_qualifiers_and_storage_multi_decl();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    // All specifiers must stay raw
    assert_eq!(kind_at(&typed, 0), 0, "`extern` must remain raw syntax");
    assert_eq!(kind_at(&typed, 1), 0, "`volatile` must remain raw syntax");
    assert_eq!(kind_at(&typed, 2), 0, "`char` must remain raw syntax");

    // First declarator: * const * restrict x
    assert_eq!(
        kind_at(&typed, 3),
        C_AST_KIND_POINTER_DECL,
        "first star must be POINTER_DECL"
    );
    assert_eq!(
        kind_at(&typed, 4),
        0,
        "`const` qualifier must remain raw syntax"
    );
    assert_eq!(
        kind_at(&typed, 5),
        C_AST_KIND_POINTER_DECL,
        "second star must be POINTER_DECL"
    );
    assert_eq!(
        kind_at(&typed, 6),
        0,
        "`restrict` qualifier must remain raw syntax"
    );
    assert_eq!(
        kind_at(&typed, 7),
        node_kind::VARIABLE,
        "`x` must classify as VARIABLE"
    );

    // Comma and second declarator
    assert_eq!(
        kind_at(&typed, 8),
        0,
        "comma between declarators must stay raw"
    );
    assert_eq!(
        kind_at(&typed, 9),
        node_kind::VARIABLE,
        "`y` must classify as VARIABLE"
    );
    assert_eq!(
        kind_at(&typed, 10),
        C_AST_KIND_ARRAY_DECL,
        "bracket after `y` must be ARRAY_DECL"
    );
}

#[test]
fn cpu_abstract_declarator_with_qualifiers_classifies() {
    let fix = fixture_abstract_declarator_with_qualifiers();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    // The outer parens are a cast expression
    assert_eq!(
        kind_at(&typed, 0),
        C_AST_KIND_CAST_EXPR,
        "outer parens of cast must be CAST_EXPR"
    );
    // const and int inside cast stay raw
    assert_eq!(
        kind_at(&typed, 1),
        0,
        "`const` inside abstract declarator must stay raw"
    );
    assert_eq!(
        kind_at(&typed, 2),
        0,
        "`int` inside abstract declarator must stay raw"
    );
    // inner abstract declarator: (*)
    assert_eq!(
        kind_at(&typed, 4),
        C_AST_KIND_POINTER_DECL,
        "abstract pointer declarator must be POINTER_DECL"
    );
    // The abstract function-pointer parameter suffix remains a declarator;
    // only the outer type-name paren is the cast expression.
    assert_eq!(
        kind_at(&typed, 6),
        C_AST_KIND_FUNCTION_DECLARATOR,
        "abstract function-pointer parameter suffix must classify as FUNCTION_DECLARATOR"
    );
    // The identifier being cast
    assert_eq!(
        kind_at(&typed, 10),
        node_kind::VARIABLE,
        "`p` after cast must classify as VARIABLE"
    );
}

#[test]
fn cpu_gnu_restrict_spelling_normalizes_to_qualifier() {
    let fix = fixture_gnu_restrict_qualifier();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        fix.tok_types[2], TOK_RESTRICT,
        "`__restrict` must normalize to TOK_RESTRICT"
    );
    assert_eq!(
        kind_at(&typed, 2),
        0,
        "`__restrict` qualifier must remain raw syntax after normalization"
    );
    assert_eq!(
        kind_at(&typed, 3),
        node_kind::VARIABLE,
        "`z` must classify as VARIABLE"
    );
}

// ---------------------------------------------------------------------------
// PG lowering preservation contracts
// ---------------------------------------------------------------------------

#[test]
fn pg_lower_preserves_struct_tag_mixed_declarator_rows() {
    let fix = fixture_struct_tag_with_mixed_declarators();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let pg = reference_ast_to_pg_nodes(&typed);

    for idx in [0usize, 4, 7, 8, 10, 11] {
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

