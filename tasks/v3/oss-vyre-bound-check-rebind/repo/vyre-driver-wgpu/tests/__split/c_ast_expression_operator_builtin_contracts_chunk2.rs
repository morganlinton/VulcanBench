#[test]
fn builtin_shapes_are_none_not_binary() {
    let fixtures = [
        builtin_constant_p_fixture(),
        builtin_choose_expr_fixture(),
        builtin_types_compatible_p_fixture(),
        generic_selection_fixture(),
    ];

    for (fixture_idx, (tok_types, tok_lens)) in fixtures.iter().enumerate() {
        let rows = run_pipeline(tok_types, tok_lens);
        for idx in 0..tok_types.len() {
            let shape_kind = word_at(&rows.expr_shape, idx * C_EXPR_SHAPE_STRIDE_U32 as usize);
            assert_ne!(
                shape_kind,
                1, // C_EXPR_SHAPE_BINARY
                "Fix: builtin fixture {fixture_idx} row {idx} must not receive BINARY shape"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// GPU / CPU parity
// ---------------------------------------------------------------------------

#[test]
fn gpu_matches_cpu_for_builtin_fixtures() {
    let fixtures: Vec<(Vec<u32>, Vec<u32>)> = vec![
        builtin_constant_p_fixture(),
        builtin_choose_expr_fixture(),
        builtin_types_compatible_p_fixture(),
        generic_selection_fixture(),
        nested_builtin_fixture(),
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
