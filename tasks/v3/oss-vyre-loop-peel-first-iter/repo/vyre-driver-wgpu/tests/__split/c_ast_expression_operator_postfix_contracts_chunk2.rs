#[test]
fn postfix_inc_dec_are_not_unary_and_not_binary() {
    let (tok_types_i, tok_lens_i) = postfix_inc_fixture();
    let rows_i = run_pipeline(&tok_types_i, &tok_lens_i);

    let kind_i = word_at(&rows_i.typed_vast, VAST_STRIDE_U32);
    assert_ne!(
        kind_i, C_AST_KIND_UNARY_EXPR,
        "Fix: postfix ++ must NOT be classified as UNARY_EXPR"
    );
    assert_ne!(
        kind_i,
        node_kind::BINARY,
        "Fix: postfix ++ must NOT be classified as BINARY"
    );
    assert_shape_none(&rows_i.expr_shape, 1, TOK_INC);

    let (tok_types_d, tok_lens_d) = postfix_dec_fixture();
    let rows_d = run_pipeline(&tok_types_d, &tok_lens_d);

    let kind_d = word_at(&rows_d.typed_vast, VAST_STRIDE_U32);
    assert_ne!(
        kind_d, C_AST_KIND_UNARY_EXPR,
        "Fix: postfix -- must NOT be classified as UNARY_EXPR"
    );
    assert_ne!(
        kind_d,
        node_kind::BINARY,
        "Fix: postfix -- must NOT be classified as BINARY"
    );
    assert_shape_none(&rows_d.expr_shape, 1, TOK_DEC);
}

// ---------------------------------------------------------------------------
// GPU / CPU parity
// ---------------------------------------------------------------------------

#[test]
fn gpu_matches_cpu_for_postfix_fixtures() {
    let fixtures: Vec<(Vec<u32>, Vec<u32>)> = vec![
        chained_member_fixture(),
        chained_arrow_fixture(),
        mixed_postfix_fixture(),
        unary_deref_fixture(),
        unary_addressof_fixture(),
        gnu_real_fixture(),
        gnu_imag_fixture(),
        label_address_fixture(),
        postfix_inc_fixture(),
        postfix_dec_fixture(),
    ];

    for (fixture_idx, (tok_types, tok_lens)) in fixtures.iter().enumerate() {
        let tok_starts = starts_for_lens(tok_lens);
        let raw_vast = reference_c11_build_vast_nodes(tok_types, &tok_starts, tok_lens);
        let typed_vast = reference_c11_classify_vast_node_kinds(&raw_vast);
        let expected_shape = reference_c11_build_expression_shape_nodes(&raw_vast, &typed_vast);
        let expected_pg = run_reference_pg_lower(&typed_vast);

        assert_eq!(
            run_gpu_expr_shape(&raw_vast, &typed_vast),
            expected_shape,
            "GPU expression-shape rows must match CPU for fixture {fixture_idx}"
        );
        assert_eq!(
            run_gpu_pg_lower(&typed_vast),
            expected_pg,
            "GPU PG lowering must match CPU for fixture {fixture_idx}"
        );

        let typed_bytes = bytes(
            &typed_vast
                .chunks_exact(4)
                .map(|chunk| u32::from_le_bytes(chunk.try_into().unwrap()))
                .collect::<Vec<_>>(),
        );
        assert_eq!(
            typed_bytes, typed_vast,
            "typed VAST fixture {fixture_idx} must stay word-aligned for GPU dispatch"
        );
    }
}
