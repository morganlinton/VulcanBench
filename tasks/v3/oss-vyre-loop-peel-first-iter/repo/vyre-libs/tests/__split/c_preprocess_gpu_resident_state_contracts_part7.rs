use super::*;

#[test]
fn conditional_stack_else_selects_when_nothing_taken() {
    let tok_types = &[
        TOK_PREPROC,
        TOK_IDENTIFIER,
        TOK_PREPROC,
        TOK_IDENTIFIER,
        TOK_PREPROC,
    ];
    let directive_kinds = &[TOK_PP_IF, 0, TOK_PP_ELSE, 0, TOK_PP_ENDIF];
    let directive_values = &[0, 0, 0, 0, 0];
    let outputs =
        run_conditional_mask_with_directives(tok_types, directive_kinds, directive_values)
            .expect("else branch must succeed");
    let mask = decode_u32_words(&outputs[0].to_bytes());
    assert_eq!(mask, vec![1, 0, 1, 1, 1]);
}

#[test]
fn conditional_stack_nested_masking() {
    // #if 1
    //   #if 0
    //     x
    //   #endif
    //   y
    // #endif
    let tok_types = &[
        TOK_PREPROC,    // 0 #if 1
        TOK_PREPROC,    // 1 #if 0
        TOK_IDENTIFIER, // 2 x
        TOK_PREPROC,    // 3 #endif
        TOK_IDENTIFIER, // 4 y
        TOK_PREPROC,    // 5 #endif
    ];
    let directive_kinds = &[TOK_PP_IF, TOK_PP_IF, 0, TOK_PP_ENDIF, 0, TOK_PP_ENDIF];
    let directive_values = &[1, 0, 0, 0, 0, 0];
    let outputs =
        run_conditional_mask_with_directives(tok_types, directive_kinds, directive_values)
            .expect("nested conditional must succeed");
    let mask = decode_u32_words(&outputs[0].to_bytes());
    assert_eq!(mask, vec![1, 1, 0, 1, 1, 1]);
}

#[test]
fn conditional_stack_nesting_overflow_fails_loudly() {
    // Build 32 nested #if 1 directives (max depth is 31).
    let mut tok_types = vec![];
    let mut kinds = vec![];
    let mut values = vec![];
    for _ in 0..32 {
        tok_types.push(TOK_PREPROC);
        kinds.push(TOK_PP_IF);
        values.push(1);
    }
    tok_types.push(TOK_PREPROC);
    kinds.push(TOK_PP_ENDIF);
    values.push(0);

    let result = catch_unwind(AssertUnwindSafe(|| {
        run_conditional_mask_with_directives(&tok_types, &kinds, &values)
    }));
    let eval = result.expect("nesting overflow must return an error, not panic");
    let err = eval.expect_err("expected reference evaluation failure");
        assert!(
            err.to_string().contains("reference dispatch trapped"),
            "unexpected error: {err}"
        );
}

