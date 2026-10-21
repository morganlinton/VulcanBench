use super::*;

#[test]
fn overflow_named_macro_expansion_output_capacity() {
    let stream = TokenStream {
        source: b"FOO",
        types: vec![TOK_IDENTIFIER],
        starts: vec![0],
        lens: vec![3],
    };
    let mut fixture = NamedFixture::empty();
    fixture.insert(
        b"FOO",
        512,
        C_MACRO_KIND_OBJECT_LIKE,
        0,
        &[
            (TOK_INTEGER, C_MACRO_REPLACEMENT_LITERAL),
            (TOK_PLUS, C_MACRO_REPLACEMENT_LITERAL),
            (TOK_INTEGER, C_MACRO_REPLACEMENT_LITERAL),
        ],
    );

    let result = catch_unwind(AssertUnwindSafe(|| run_named(&stream, &fixture, 2)));
    let eval = result.expect("output overflow must return an error, not panic");
    let err = eval.expect_err("expected reference evaluation failure");
        assert!(
            err.to_string().contains("reference dispatch trapped"),
            "unexpected error: {err}"
        );
}

#[test]
fn overflow_dynamic_macro_expansion_output_capacity() {
    let mut fixture = DynamicFixture::empty();
    fixture.insert(TOK_IDENTIFIER, 512, &[TOK_INTEGER, TOK_PLUS, TOK_INTEGER]);

    let result = catch_unwind(AssertUnwindSafe(|| {
        run_dynamic(&[TOK_IDENTIFIER, TOK_IDENTIFIER], &fixture, 5)
    }));
    let eval = result.expect("dynamic output overflow must return an error, not panic");
    let err = eval.expect_err("expected reference evaluation failure");
        assert!(
            err.to_string().contains("reference dispatch trapped"),
            "unexpected error: {err}"
        );
}

#[test]
fn overflow_conditional_mask_empty_stream() {
    let result = run_conditional_mask(&[]);
    assert!(
        result.is_err(),
        "empty conditional mask must fail loudly instead of hiding bad bounds"
    );
    let err = result.unwrap_err();
    assert!(
        err.to_string()
            .contains("conditional-mask-empty-token-stream"),
        "empty conditional-mask failure must identify the parser pipeline boundary"
    );
}

