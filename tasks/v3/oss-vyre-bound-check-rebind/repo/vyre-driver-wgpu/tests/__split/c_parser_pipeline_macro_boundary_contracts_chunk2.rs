#[test]
fn conditional_mask_is_deterministic_across_many_runs() {
    let inputs = vec![TOK_PREPROC, TOK_IDENTIFIER, TOK_HASH, TOK_INTEGER];
    let out_a = run_conditional_mask(&inputs).unwrap();
    let out_b = run_conditional_mask(&inputs).unwrap();
    let out_c = run_conditional_mask(&inputs).unwrap();

    assert_eq!(
        decode_u32_words(&out_a[0].to_bytes()),
        decode_u32_words(&out_b[0].to_bytes())
    );
    assert_eq!(
        decode_u32_words(&out_b[0].to_bytes()),
        decode_u32_words(&out_c[0].to_bytes())
    );
}

// ---------------------------------------------------------------------------
// 7. GPU parity for macro expansion
// ---------------------------------------------------------------------------

#[test]
fn gpu_macro_expansion_matches_reference_for_simple_replacement() {
    let mut fixture = MacroFixture::empty();
    fixture.insert(TOK_IDENTIFIER, 512, &[TOK_INTEGER, TOK_PLUS, TOK_INTEGER]);
    let input = [TOK_IDENTIFIER, TOK_SEMICOLON];

    let cpu = run_dynamic_macro_expansion(&input, &fixture, 8).unwrap();
    let gpu = run_gpu_macro_expansion(&input, &fixture, 8);

    assert_eq!(decode_u32_words(&cpu[0].to_bytes()), decode_u32_words(&gpu[0]));
    assert_eq!(decode_u32_words(&cpu[1].to_bytes()), decode_u32_words(&gpu[1]));
}

#[test]
fn gpu_macro_expansion_matches_reference_for_multi_macro() {
    let mut fixture = MacroFixture::empty();
    fixture.insert(TOK_IDENTIFIER, 512, &[TOK_INTEGER]);
    fixture.insert(TOK_STAR, 513, &[TOK_PLUS, TOK_PLUS]);
    let input = [TOK_IDENTIFIER, TOK_STAR, TOK_IDENTIFIER];

    let cpu = run_dynamic_macro_expansion(&input, &fixture, 16).unwrap();
    let gpu = run_gpu_macro_expansion(&input, &fixture, 16);

    assert_eq!(decode_u32_words(&cpu[0].to_bytes()), decode_u32_words(&gpu[0]));
    assert_eq!(decode_u32_words(&cpu[1].to_bytes()), decode_u32_words(&gpu[1]));
}

#[test]
fn gpu_macro_expansion_matches_reference_for_zero_length_replacement() {
    let mut fixture = MacroFixture::empty();
    fixture.insert(TOK_IDENTIFIER, 512, &[]);
    let input = [TOK_INT, TOK_IDENTIFIER, TOK_SEMICOLON];

    let cpu = run_dynamic_macro_expansion(&input, &fixture, 8).unwrap();
    let gpu = run_gpu_macro_expansion(&input, &fixture, 8);

    assert_eq!(decode_u32_words(&cpu[0].to_bytes()), decode_u32_words(&gpu[0]));
    assert_eq!(decode_u32_words(&cpu[1].to_bytes()), decode_u32_words(&gpu[1]));
}

// ---------------------------------------------------------------------------
// 8. No silent empty outputs
// ---------------------------------------------------------------------------

#[test]
fn macro_expansion_nonempty_input_produces_nonempty_output_or_error() {
    let fixture = MacroFixture::empty();
    let input = [TOK_IDENTIFIER];

    // No mapping -> passthrough, so output must be non-empty.
    let outputs =
        run_dynamic_macro_expansion(&input, &fixture, 8).expect("passthrough must succeed");
    let out_tokens = decode_u32_words(&outputs[0].to_bytes());
    let out_count = decode_u32_words(&outputs[1].to_bytes());
    assert_eq!(
        out_count[0], 1,
        "passthrough must not silently emit zero count"
    );
    assert_eq!(
        out_tokens[0], TOK_IDENTIFIER,
        "passthrough must preserve token"
    );
}

#[test]
fn macro_expansion_with_empty_fixture_still_emits_count() {
    let fixture = MacroFixture::empty();
    let input = [TOK_INT, TOK_IDENTIFIER, TOK_SEMICOLON];

    let outputs = run_dynamic_macro_expansion(&input, &fixture, 8).unwrap();
    let out_count = decode_u32_words(&outputs[1].to_bytes());
    assert_eq!(
        out_count[0], 3,
        "empty fixture must still emit correct count"
    );
}
