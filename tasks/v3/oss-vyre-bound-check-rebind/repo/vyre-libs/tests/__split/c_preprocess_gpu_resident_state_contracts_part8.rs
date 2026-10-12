use super::*;

#[test]
fn conditional_stack_unclosed_open_fails_loudly() {
    let tok_types = &[TOK_PREPROC, TOK_IDENTIFIER];
    let directive_kinds = &[TOK_PP_IF, 0];
    let directive_values = &[1, 0];

    let result = catch_unwind(AssertUnwindSafe(|| {
        run_conditional_mask_with_directives(tok_types, directive_kinds, directive_values)
    }));
    let eval = result.expect("unclosed conditional must return an error, not panic");
    let err = eval.expect_err("expected reference evaluation failure");
        assert!(
            err.to_string().contains("reference dispatch trapped"),
            "unexpected error: {err}"
        );
}

#[test]
fn conditional_stack_elif_without_open_fails_loudly() {
    let tok_types = &[TOK_PREPROC];
    let directive_kinds = &[TOK_PP_ELIF];
    let directive_values = &[1];

    let result = catch_unwind(AssertUnwindSafe(|| {
        run_conditional_mask_with_directives(tok_types, directive_kinds, directive_values)
    }));
    let eval = result.expect("elif-without-open must return an error, not panic");
    let err = eval.expect_err("expected reference evaluation failure");
        assert!(
            err.to_string().contains("reference dispatch trapped"),
            "unexpected error: {err}"
        );
}

#[test]
fn conditional_stack_else_without_open_fails_loudly() {
    let tok_types = &[TOK_PREPROC];
    let directive_kinds = &[TOK_PP_ELSE];
    let directive_values = &[0];

    let result = catch_unwind(AssertUnwindSafe(|| {
        run_conditional_mask_with_directives(tok_types, directive_kinds, directive_values)
    }));
    let eval = result.expect("else-without-open must return an error, not panic");
    let err = eval.expect_err("expected reference evaluation failure");
        assert!(
            err.to_string().contains("reference dispatch trapped"),
            "unexpected error: {err}"
        );
}

