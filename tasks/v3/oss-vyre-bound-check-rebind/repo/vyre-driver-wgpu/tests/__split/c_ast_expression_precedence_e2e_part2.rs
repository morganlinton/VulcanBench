use super::*;

#[test]
fn unary_chain_typing_and_pg_lower() {
    let (tok_types, tok_lens) = unary_chain_fixture();
    let rows = run_pipeline(&tok_types, &tok_lens);

    // Unary operators receive NONE shape rows.
    for idx in [0usize, 1, 2, 3, 4, 5] {
        assert_shape_row(
            &rows.expr_shape,
            idx,
            C_EXPR_SHAPE_NONE,
            tok_types[idx],
            0,
            C_EXPR_ASSOC_NONE,
            SENTINEL,
            SENTINEL,
            SENTINEL,
        );
    }

    assert_eq!(
        row_indices(&rows.typed_vast, VAST_STRIDE_U32, C_AST_KIND_UNARY_EXPR),
        vec![0, 1, 2, 3, 4, 5]
    );

    for idx in [0usize, 1, 2, 3, 4, 5] {
        assert_pg_preserves_row(&rows, idx, C_AST_KIND_UNARY_EXPR);
        assert_pg_links_match_vast(&rows, idx);
    }
}

// ---------------------------------------------------------------------------
// GPU / CPU parity test
// ---------------------------------------------------------------------------

#[test]
fn gpu_matches_cpu_for_expression_shape_and_pg_lowering() {
    let fixtures: Vec<(Vec<u32>, Vec<u32>)> = vec![
        comma_fixture(),
        assignment_chain_fixture(),
        ternary_nesting_fixture(),
        logical_bitwise_fixture(),
        cast_vs_paren_fixture(),
        postfix_fixture(),
        unary_chain_fixture(),
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
