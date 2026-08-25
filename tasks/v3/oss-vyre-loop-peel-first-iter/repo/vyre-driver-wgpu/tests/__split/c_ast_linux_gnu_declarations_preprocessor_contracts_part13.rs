// (use super::* removed  -  parts now share crate-root scope via flat include)

#[test]
fn cpu_nested_conditional_preproc_mask_contract() {
    let fix = fixture_nested_conditional_preproc();
    let (directive_kinds, directive_values) = reference_c_preprocessor_directive_metadata(
        &fix.tok_types,
        &fix.tok_starts,
        &fix.tok_lens,
        fix.source.as_bytes(),
        &[],
    )
    .expect("directive metadata must classify nested conditionals");

    let mut expected_kinds = vec![0u32; fix.tok_types.len()];
    expected_kinds[0] = TOK_PP_IF;
    expected_kinds[4] = TOK_PP_IFDEF;
    expected_kinds[8] = TOK_PP_ELSE;
    expected_kinds[12] = TOK_PP_ENDIF;
    expected_kinds[13] = TOK_PP_ELIF;
    expected_kinds[17] = TOK_PP_ELSE;
    expected_kinds[21] = TOK_PP_ENDIF;
    assert_eq!(directive_kinds, expected_kinds, "directive kinds mismatch");

    let mut expected_values = vec![0u32; fix.tok_types.len()];
    expected_values[0] = 1; // #if 1
                            // #ifdef MISSING -> 0 (false), #elif 0 -> 0 (false)
    assert_eq!(
        directive_values, expected_values,
        "directive values mismatch"
    );

    let program = opt_conditional_mask_with_directives(
        "tok_types",
        "directive_kinds",
        "directive_values",
        "out_mask",
        Expr::u32(fix.tok_types.len() as u32),
    );
    let values = [
        Value::from(u32_bytes(&fix.tok_types)),
        Value::from(u32_bytes(&directive_kinds)),
        Value::from(u32_bytes(&directive_values)),
        Value::from(vec![0u8; fix.tok_types.len() * 4]),
    ];
    let outputs =
        vyre_reference::reference_eval(&program, &values).expect("conditional mask must succeed");
    let mask = decode_u32_words(&outputs[0].to_bytes());

    let expected_mask = [
        1u32, // #if 1
        1, 1, 1, // int a;
        1, // #ifdef MISSING
        0, 0, 0, // int b; (dead)
        1, // #else
        1, 1, 1, // int c; (live)
        1, // #endif
        1, // #elif 0
        0, 0, 0, // int d; (dead)
        1, // #else
        0, 0, 0, // int e; (dead because outer #if already true)
        1, // #endif
        1, 1, 1, // int f; (live)
    ];
    assert_eq!(
        &mask[..fix.tok_types.len()],
        &expected_mask[..],
        "conditional mask must match expected nested pattern"
    );
}

#[test]
fn cpu_nested_conditional_preproc_vast_survives() {
    let fix = fixture_nested_conditional_preproc();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = classify_fixture(&fix);

    for idx in [0usize, 4, 8, 12, 13, 17, 21] {
        assert_eq!(
            word_at(&raw, idx * VAST_STRIDE_U32),
            TOK_PREPROC,
            "raw VAST must preserve TOK_PREPROC at index {idx}"
        );
        assert_eq!(
            word_at(&typed, idx * VAST_STRIDE_U32),
            0,
            "typed VAST must leave preproc rows as raw syntax"
        );
    }
}

#[test]
fn pg_lower_preserves_nested_conditional_preproc_rows() {
    let fix = fixture_nested_conditional_preproc();
    let typed = classify_fixture(&fix);
    let pg = reference_ast_to_pg_nodes(&typed);

    for idx in [0usize, 4, 8, 12, 13, 17, 21] {
        assert_pg_preserves_row(&typed, &pg, &fix, idx, 0);
    }
}

