#[test]
fn plus_binary_is_binary_and_unary_is_unary() {
    let (tok_types, tok_lens) = plus_binary_fixture();
    let rows = run_pipeline(&tok_types, &tok_lens);

    assert_eq!(
        row_indices(&rows.typed_vast, VAST_STRIDE_U32, node_kind::BINARY),
        vec![1],
        "Fix: + in binary context must classify as BINARY"
    );
    assert_shape_row(
        &rows.expr_shape,
        1,
        C_EXPR_SHAPE_BINARY,
        TOK_PLUS,
        12,
        C_EXPR_ASSOC_LEFT,
        0,
        2,
        SENTINEL,
    );
    assert_pg_preserves_row(&rows, 1, node_kind::BINARY);
    assert_pg_links_match_vast(&rows, 1);

    let (tok_types_u, tok_lens_u) = plus_unary_fixture();
    let rows_u = run_pipeline(&tok_types_u, &tok_lens_u);
    assert_eq!(
        row_indices(&rows_u.typed_vast, VAST_STRIDE_U32, C_AST_KIND_UNARY_EXPR),
        vec![0],
        "Fix: + in unary context must classify as UNARY_EXPR"
    );
    assert_shape_row(
        &rows_u.expr_shape,
        0,
        C_EXPR_SHAPE_NONE,
        TOK_PLUS,
        0,
        C_EXPR_ASSOC_NONE,
        SENTINEL,
        SENTINEL,
        SENTINEL,
    );
    assert_pg_preserves_row(&rows_u, 0, C_AST_KIND_UNARY_EXPR);
    assert_pg_links_match_vast(&rows_u, 0);
}

#[test]
fn minus_binary_is_binary_and_unary_is_unary() {
    let (tok_types, tok_lens) = minus_binary_fixture();
    let rows = run_pipeline(&tok_types, &tok_lens);

    assert_eq!(
        row_indices(&rows.typed_vast, VAST_STRIDE_U32, node_kind::BINARY),
        vec![1],
        "Fix: - in binary context must classify as BINARY"
    );
    assert_shape_row(
        &rows.expr_shape,
        1,
        C_EXPR_SHAPE_BINARY,
        TOK_MINUS,
        12,
        C_EXPR_ASSOC_LEFT,
        0,
        2,
        SENTINEL,
    );
    assert_pg_preserves_row(&rows, 1, node_kind::BINARY);
    assert_pg_links_match_vast(&rows, 1);

    let (tok_types_u, tok_lens_u) = minus_unary_fixture();
    let rows_u = run_pipeline(&tok_types_u, &tok_lens_u);
    assert_eq!(
        row_indices(&rows_u.typed_vast, VAST_STRIDE_U32, C_AST_KIND_UNARY_EXPR),
        vec![0],
        "Fix: - in unary context must classify as UNARY_EXPR"
    );
    assert_shape_row(
        &rows_u.expr_shape,
        0,
        C_EXPR_SHAPE_NONE,
        TOK_MINUS,
        0,
        C_EXPR_ASSOC_NONE,
        SENTINEL,
        SENTINEL,
        SENTINEL,
    );
    assert_pg_preserves_row(&rows_u, 0, C_AST_KIND_UNARY_EXPR);
    assert_pg_links_match_vast(&rows_u, 0);
}

#[test]
fn cast_expr_classifies_lparen_and_paren_expr_does_not() {
    let (tok_types, tok_lens) = cast_simple_fixture();
    let rows = run_pipeline(&tok_types, &tok_lens);

    assert_eq!(
        word_at(&rows.typed_vast, 0 * VAST_STRIDE_U32),
        C_AST_KIND_CAST_EXPR,
        "Fix: (int)a must classify the opening ( as CAST_EXPR"
    );
    assert_shape_row(
        &rows.expr_shape,
        0,
        C_EXPR_SHAPE_NONE,
        TOK_LPAREN,
        0,
        C_EXPR_ASSOC_NONE,
        SENTINEL,
        SENTINEL,
        SENTINEL,
    );
    assert_pg_preserves_row(&rows, 0, C_AST_KIND_CAST_EXPR);
    assert_pg_links_match_vast(&rows, 0);

    let (tok_types_p, tok_lens_p) = paren_expr_fixture();
    let rows_p = run_pipeline(&tok_types_p, &tok_lens_p);
    assert_eq!(
        word_at(&rows_p.typed_vast, 0 * VAST_STRIDE_U32),
        0,
        "Fix: (a + b) must NOT classify the opening ( as CAST_EXPR"
    );
    assert_shape_row(
        &rows_p.expr_shape,
        0,
        C_EXPR_SHAPE_NONE,
        TOK_LPAREN,
        0,
        C_EXPR_ASSOC_NONE,
        SENTINEL,
        SENTINEL,
        SENTINEL,
    );
    // Plus inside parentheses.
    assert_shape_row(
        &rows_p.expr_shape,
        2,
        C_EXPR_SHAPE_BINARY,
        TOK_PLUS,
        12,
        C_EXPR_ASSOC_LEFT,
        1,
        3,
        SENTINEL,
    );
    assert_pg_preserves_row(&rows_p, 2, node_kind::BINARY);
    assert_pg_links_match_vast(&rows_p, 2);
}

#[test]
fn complex_cast_vs_nested_parens_classification() {
    let (tok_types, tok_lens) = cast_complex_fixture();
    let rows = run_pipeline(&tok_types, &tok_lens);

    assert_eq!(
        word_at(&rows.typed_vast, 0 * VAST_STRIDE_U32),
        C_AST_KIND_CAST_EXPR,
        "Fix: (const int *)p must classify as CAST_EXPR"
    );
    assert_pg_preserves_row(&rows, 0, C_AST_KIND_CAST_EXPR);
    assert_pg_links_match_vast(&rows, 0);

    // The * inside the cast is a pointer declarator, not an operator.
    assert_eq!(
        word_at(&rows.typed_vast, 3 * VAST_STRIDE_U32),
        0xC011_D001, // C_AST_KIND_POINTER_DECL
        "Fix: * inside a cast type-name must classify as POINTER_DECL"
    );

    let (tok_types_p, tok_lens_p) = paren_nested_fixture();
    let rows_p = run_pipeline(&tok_types_p, &tok_lens_p);
    assert_eq!(
        word_at(&rows_p.typed_vast, 0 * VAST_STRIDE_U32),
        0,
        "Fix: ((a)) outer ( must NOT be CAST_EXPR"
    );
    assert_eq!(
        word_at(&rows_p.typed_vast, VAST_STRIDE_U32),
        0,
        "Fix: ((a)) inner ( must NOT be CAST_EXPR"
    );
}

#[test]
fn sizeof_and_typeof_do_not_classify_inner_lparen_as_cast() {
    let (tok_types_s, tok_lens_s) = sizeof_typename_fixture();
    let rows_s = run_pipeline(&tok_types_s, &tok_lens_s);

    assert_eq!(
        word_at(&rows_s.typed_vast, 0 * VAST_STRIDE_U32),
        C_AST_KIND_SIZEOF_EXPR,
        "Fix: sizeof must classify as SIZEOF_EXPR"
    );
    assert_eq!(
        word_at(&rows_s.typed_vast, VAST_STRIDE_U32),
        0,
        "Fix: ( after sizeof must NOT be classified as CAST_EXPR"
    );
    assert_pg_preserves_row(&rows_s, 0, C_AST_KIND_SIZEOF_EXPR);
    assert_pg_links_match_vast(&rows_s, 0);

    let (tok_types_e, tok_lens_e) = sizeof_expr_fixture();
    let rows_e = run_pipeline(&tok_types_e, &tok_lens_e);
    assert_eq!(
        word_at(&rows_e.typed_vast, 0 * VAST_STRIDE_U32),
        C_AST_KIND_SIZEOF_EXPR,
        "Fix: sizeof expr must classify as SIZEOF_EXPR"
    );
    assert_pg_preserves_row(&rows_e, 0, C_AST_KIND_SIZEOF_EXPR);

    let (tok_types_t, tok_lens_t) = typeof_typename_fixture();
    let rows_t = run_pipeline(&tok_types_t, &tok_lens_t);
    assert_eq!(
        word_at(&rows_t.typed_vast, 0 * VAST_STRIDE_U32),
        C_AST_KIND_SIZEOF_EXPR,
        "Fix: typeof must reuse SIZEOF_EXPR kind"
    );
    assert_eq!(
        word_at(&rows_t.typed_vast, VAST_STRIDE_U32),
        0,
        "Fix: ( after typeof must NOT be classified as CAST_EXPR"
    );

    let (tok_types_te, tok_lens_te) = typeof_expr_fixture();
    let rows_te = run_pipeline(&tok_types_te, &tok_lens_te);
    assert_eq!(
        word_at(&rows_te.typed_vast, 0 * VAST_STRIDE_U32),
        C_AST_KIND_SIZEOF_EXPR,
        "Fix: typeof expr must reuse SIZEOF_EXPR kind"
    );
}

// ---------------------------------------------------------------------------
// GPU / CPU parity
// ---------------------------------------------------------------------------

#[test]
fn gpu_matches_cpu_for_ambiguity_fixtures() {
    let fixtures: Vec<(Vec<u32>, Vec<u32>)> = vec![
        star_binary_fixture(),
        star_unary_fixture(),
        amp_binary_fixture(),
        amp_unary_fixture(),
        plus_binary_fixture(),
        plus_unary_fixture(),
        minus_binary_fixture(),
        minus_unary_fixture(),
        cast_simple_fixture(),
        paren_expr_fixture(),
        cast_complex_fixture(),
        paren_nested_fixture(),
        sizeof_typename_fixture(),
        sizeof_expr_fixture(),
        typeof_typename_fixture(),
        typeof_expr_fixture(),
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
