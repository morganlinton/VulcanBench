#[test]
fn cpu_reference_tag_separation() {
    let (tok_types, tok_starts, tok_lens) = fixture_tag_separation();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        word_at(&typed, 20 * VAST_STRIDE_U32),
        node_kind::VARIABLE,
        "a must be a variable"
    );
    assert_eq!(
        word_at(&typed, 22 * VAST_STRIDE_U32),
        node_kind::VARIABLE,
        "b must be a variable"
    );
}

#[test]
fn cpu_reference_gnu_attributes() {
    let (tok_types, tok_starts, tok_lens) = fixture_gnu_attributes();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        word_at(&typed, 0 * VAST_STRIDE_U32),
        C_AST_KIND_GNU_ATTRIBUTE
    );
    assert_eq!(
        word_at(&typed, 7 * VAST_STRIDE_U32),
        node_kind::FUNCTION_DECL
    );
}

#[test]
fn cpu_reference_compound_literal() {
    let (tok_types, tok_starts, tok_lens) = fixture_compound_literal();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_eq!(
        word_at(&typed, 0 * VAST_STRIDE_U32),
        C_AST_KIND_COMPOUND_LITERAL_EXPR
    );
}

#[test]
fn gpu_parity_contracts() {
    let fixtures = vec![
        fixture_typedef_shadowing(),
        fixture_cast_vs_multiply(),
        fixture_nested_fnptr(),
        fixture_tag_separation(),
        fixture_gnu_attributes(),
        fixture_compound_literal(),
    ];

    for (tok_types, tok_starts, tok_lens) in fixtures {
        let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
        let expected = reference_c11_classify_vast_node_kinds(&raw);
        let gpu = run_gpu_classifier(&raw, tok_types.len() as u32);

        assert_eq!(
            gpu, expected,
            "GPU classifier parity failed for contract fixture"
        );

        let typed = expected;
        let expected_pg = reference_ast_to_pg_nodes(&typed);
        let gpu_pg = run_gpu_pg_lower(&typed, tok_types.len() as u32);

        assert_eq!(
            gpu_pg, expected_pg,
            "GPU PG lowerer parity failed for contract fixture"
        );
    }
}
