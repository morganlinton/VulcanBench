#[test]
fn pg_lower_preserves_computed_goto_rows() {
    let fix = fixture_computed_goto_simple();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let pg = reference_ast_to_pg_nodes(&typed);

    assert_pg_preserves_row(&typed, &pg, &fix, 9, C_AST_KIND_GNU_LABEL_ADDRESS_EXPR);
    assert_pg_preserves_row(&typed, &pg, &fix, 12, C_AST_KIND_LABEL_STMT);
}

#[test]
fn pg_lower_preserves_for_with_declaration_rows() {
    let fix = fixture_for_with_declaration();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let pg = reference_ast_to_pg_nodes(&typed);

    assert_pg_preserves_row(&typed, &pg, &fix, 7, C_AST_KIND_FOR_STMT);
}

#[test]
fn pg_lower_preserves_atomic_qualifier_rows() {
    let fix = fixture_atomic_qualifier();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let pg = reference_ast_to_pg_nodes(&typed);

    // PG lowering must preserve the function body brace.
    assert_eq!(
        pg_word_at(&pg, 4, 0),
        node_kind::BASIC_BLOCK,
        "function body brace must lower to BASIC_BLOCK"
    );
}

// ---------------------------------------------------------------------------
// Tests – GPU PG lowering parity
// ---------------------------------------------------------------------------

#[test]
fn gpu_pg_lower_matches_cpu_for_computed_goto_fixtures() {
    let fixtures: Vec<(&str, Fixture)> = vec![
        ("computed_goto_simple", fixture_computed_goto_simple()),
        ("computed_goto_in_goto", fixture_computed_goto_in_goto()),
        ("computed_goto_comma", fixture_computed_goto_comma()),
    ];

    for (label, fix) in fixtures {
        let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
        let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
        let typed = reference_c11_classify_vast_node_kinds(&annotated);
        let expected = reference_ast_to_pg_nodes(&typed);
        let gpu = run_gpu_pg_lower(&typed);
        assert_eq!(
            gpu, expected,
            "GPU PG lowerer must match CPU for fixture `{label}`"
        );
    }
}

#[test]
fn gpu_pg_lower_matches_cpu_for_loop_and_atomic_fixtures() {
    let fixtures: Vec<(&str, Fixture)> = vec![
        ("for_with_declaration", fixture_for_with_declaration()),
        (
            "for_with_multiple_declarators",
            fixture_for_with_multiple_declarators(),
        ),
        ("atomic_qualifier", fixture_atomic_qualifier()),
        ("atomic_type_specifier", fixture_atomic_type_specifier()),
    ];

    for (label, fix) in fixtures {
        let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
        let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
        let typed = reference_c11_classify_vast_node_kinds(&annotated);
        let expected = reference_ast_to_pg_nodes(&typed);
        let gpu = run_gpu_pg_lower(&typed);
        assert_eq!(
            gpu, expected,
            "GPU PG lowerer must match CPU for fixture `{label}`"
        );
    }
}
