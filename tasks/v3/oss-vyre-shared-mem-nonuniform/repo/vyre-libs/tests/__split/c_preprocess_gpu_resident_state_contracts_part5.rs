use super::*;

#[test]
fn function_like_macro_parameter_count_overflow_fails_loudly() {
    let stream = TokenStream {
        source: b"F(a)",
        types: vec![TOK_IDENTIFIER, TOK_LPAREN, TOK_IDENTIFIER, TOK_RPAREN],
        starts: vec![0, 1, 2, 3],
        lens: vec![1, 1, 1, 1],
    };
    let mut fixture = NamedFixture::empty();
    fixture.insert(b"F", 512, C_MACRO_KIND_FUNCTION_LIKE, 17, &[(0, 0)]);

    let result = catch_unwind(AssertUnwindSafe(|| run_named(&stream, &fixture, 8)));
    let eval = result.expect("param-count overflow must return an error, not panic");
    let err = eval.expect_err("expected reference evaluation failure");
        assert!(
            err.to_string().contains("reference dispatch trapped"),
            "unexpected error: {err}"
        );
}

#[test]
fn function_like_macro_argument_count_overflow_fails_loudly() {
    // Build a call with 17 comma-separated arguments.
    let mut types = vec![TOK_IDENTIFIER, TOK_LPAREN];
    let mut starts = vec![0, 1];
    let mut lens = vec![1, 1];
    for i in 0..17 {
        types.push(TOK_IDENTIFIER);
        starts.push(2 + i * 2);
        lens.push(1);
        if i < 16 {
            types.push(TOK_COMMA);
            starts.push(3 + i * 2);
            lens.push(1);
        }
    }
    types.push(TOK_RPAREN);
    starts.push(2 + 17 * 2);
    lens.push(1);

    let stream = TokenStream {
        source: b"F(a,a,a,a,a,a,a,a,a,a,a,a,a,a,a,a,a)",
        types,
        starts,
        lens,
    };
    let mut fixture = NamedFixture::empty();
    fixture.insert(b"F", 512, C_MACRO_KIND_FUNCTION_LIKE, 0, &[]);

    let result = catch_unwind(AssertUnwindSafe(|| run_named(&stream, &fixture, 8)));
    let eval = result.expect("arg-count overflow must return an error, not panic");
    let err = eval.expect_err("expected reference evaluation failure");
        assert!(
            err.to_string().contains("reference dispatch trapped"),
            "unexpected error: {err}"
        );
}

// ---------------------------------------------------------------------------
// 3. Conditional stack contracts
// ---------------------------------------------------------------------------

#[test]
fn conditional_stack_depth_starts_at_zero() {
    // An empty directive stream must leave depth == 0 and emit no trap.
    let tok_types = &[TOK_IDENTIFIER];
    let directive_kinds = &[0];
    let directive_values = &[0];
    let outputs =
        run_conditional_mask_with_directives(tok_types, directive_kinds, directive_values)
            .expect("empty conditional stack must succeed");
    let mask = decode_u32_words(&outputs[0].to_bytes());
    assert_eq!(mask, vec![1]);
}

