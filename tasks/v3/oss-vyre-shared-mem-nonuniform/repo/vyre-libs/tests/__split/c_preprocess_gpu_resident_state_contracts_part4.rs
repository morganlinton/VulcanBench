use super::*;

#[test]
fn function_like_macro_arg_arena_preserves_nested_parens() {
    let stream = TokenStream {
        source: b"F((a),b)",
        types: vec![
            TOK_IDENTIFIER,
            TOK_LPAREN,
            TOK_LPAREN,
            TOK_IDENTIFIER,
            TOK_RPAREN,
            TOK_COMMA,
            TOK_IDENTIFIER,
            TOK_RPAREN,
        ],
        starts: vec![0, 1, 2, 3, 4, 5, 6, 7],
        lens: vec![1, 1, 1, 1, 1, 1, 1, 1],
    };
    let mut fixture = NamedFixture::empty();
    fixture.insert(b"F", 512, C_MACRO_KIND_FUNCTION_LIKE, 2, &[(0, 0), (0, 1)]);

    let outputs = run_named(&stream, &fixture, 8).expect("nested-paren arg must succeed");
    let out = decode_u32_words(&outputs[0].to_bytes());
    let count = decode_u32_words(&outputs[1].to_bytes());
    assert_eq!(count, vec![4]);
    // arg0 is "(a)", arg1 is "b"
    assert_eq!(
        &out[..4],
        &[TOK_LPAREN, TOK_IDENTIFIER, TOK_RPAREN, TOK_IDENTIFIER]
    );
}

#[test]
fn function_like_macro_arg_count_mismatch_fails_loudly() {
    let stream = TokenStream {
        source: b"MAX(a)",
        types: vec![TOK_IDENTIFIER, TOK_LPAREN, TOK_IDENTIFIER, TOK_RPAREN],
        starts: vec![0, 3, 4, 5],
        lens: vec![3, 1, 1, 1],
    };
    let mut fixture = NamedFixture::empty();
    fixture.insert(b"MAX", 512, C_MACRO_KIND_FUNCTION_LIKE, 2, &[(0, 0)]);

    let result = catch_unwind(AssertUnwindSafe(|| run_named(&stream, &fixture, 8)));
    let eval = result.expect("arg-count mismatch must return an error, not panic");
    let err = eval.expect_err("expected reference evaluation failure");
        assert!(
            err.to_string().contains("reference dispatch trapped"),
            "unexpected error: {err}"
        );
}

#[test]
fn function_like_macro_replacement_parameter_out_of_range_fails_loudly() {
    let stream = TokenStream {
        source: b"F(a)",
        types: vec![TOK_IDENTIFIER, TOK_LPAREN, TOK_IDENTIFIER, TOK_RPAREN],
        starts: vec![0, 1, 2, 3],
        lens: vec![1, 1, 1, 1],
    };
    let mut fixture = NamedFixture::empty();
    // Param index 5 exceeds param_count 1
    fixture.insert(b"F", 512, C_MACRO_KIND_FUNCTION_LIKE, 1, &[(0, 5)]);

    let result = catch_unwind(AssertUnwindSafe(|| run_named(&stream, &fixture, 8)));
    let eval = result.expect("out-of-range param must return an error, not panic");
    let err = eval.expect_err("expected reference evaluation failure");
        assert!(
            err.to_string().contains("reference dispatch trapped"),
            "unexpected error: {err}"
        );
}

