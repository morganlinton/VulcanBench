#[test]
fn compound_literal_designators_and_nested_conditional_lower_to_pg() {
    let (tok_types, tok_lens) = compound_literal_fixture();
    let rows = run_pipeline(&tok_types, &tok_lens);

    for (idx, kind) in [
        (1, C_AST_KIND_ASSIGN_EXPR),
        (2, C_AST_KIND_COMPOUND_LITERAL_EXPR),
        (6, C_AST_KIND_INITIALIZER_LIST),
        (7, C_AST_KIND_MEMBER_ACCESS_EXPR),
        (9, C_AST_KIND_ASSIGN_EXPR),
        (11, C_AST_KIND_ARRAY_SUBSCRIPT_EXPR),
        (15, C_AST_KIND_MEMBER_ACCESS_EXPR),
        (17, C_AST_KIND_ASSIGN_EXPR),
        (19, C_AST_KIND_CONDITIONAL_EXPR),
    ] {
        assert_pg_preserves_row(&rows, idx, kind);
    }
}

#[test]
fn label_switch_case_default_block_rows_lower_to_pg() {
    let (tok_types, tok_lens) = label_switch_fixture();
    let rows = run_pipeline(&tok_types, &tok_lens);

    for kind in [
        C_AST_KIND_LABEL_STMT,
        C_AST_KIND_SWITCH_STMT,
        node_kind::BASIC_BLOCK,
        C_AST_KIND_CASE_STMT,
        C_AST_KIND_ASSIGN_EXPR,
        C_AST_KIND_DEFAULT_STMT,
        C_AST_KIND_GOTO_STMT,
    ] {
        let indices = row_indices(&rows.typed_vast, VAST_STRIDE_U32, kind);
        assert!(
            !indices.is_empty(),
            "expected typed VAST to contain kind {kind:#x}"
        );
        for idx in indices {
            assert_pg_preserves_row(&rows, idx, kind);
        }
    }

    assert_shape_row(
        &rows.expr_shape,
        row_indices(&rows.typed_vast, VAST_STRIDE_U32, C_AST_KIND_ASSIGN_EXPR)[0],
        C_EXPR_SHAPE_BINARY,
        TOK_ASSIGN,
        2,
        C_EXPR_ASSOC_RIGHT,
        10,
        12,
        SENTINEL,
    );
    assert_eq!(
        word_at(&rows.pg_nodes, 6 * PG_STRIDE_U32 + 4),
        7,
        "switch body block must keep case as first child"
    );
    for idx in [0usize, 2, 6, 7, 11, 14, 16, 17] {
        assert_pg_links_match_vast(&rows, idx);
    }
}

#[test]
fn gpu_matches_cpu_for_expression_shape_and_pg_lowering() {
    let (tok_types, tok_lens) = expression_chain_fixture();
    let tok_starts = starts_for_lens(&tok_lens);
    let raw_vast = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed_vast = reference_c11_classify_vast_node_kinds(&raw_vast);
    let expected_shape = reference_c11_build_expression_shape_nodes(&raw_vast, &typed_vast);
    let expected_pg = run_reference_pg_lower(&typed_vast);

    assert_eq!(
        run_gpu_expr_shape(&raw_vast, &typed_vast),
        expected_shape,
        "GPU expression-shape rows must match CPU"
    );
    assert_eq!(
        run_gpu_pg_lower(&typed_vast),
        expected_pg,
        "GPU PG lowering must match CPU"
    );

    let typed_bytes = bytes(
        &typed_vast
            .chunks_exact(4)
            .map(|chunk| u32::from_le_bytes(chunk.try_into().unwrap()))
            .collect::<Vec<_>>(),
    );
    assert_eq!(
        typed_bytes, typed_vast,
        "typed VAST fixture must stay word-aligned for GPU dispatch"
    );
}
