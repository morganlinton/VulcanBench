use super::*;

#[test]
fn conditional_stack_endif_without_open_fails_loudly() {
    let tok_types = &[TOK_PREPROC];
    let directive_kinds = &[TOK_PP_ENDIF];
    let directive_values = &[0];

    let result = catch_unwind(AssertUnwindSafe(|| {
        run_conditional_mask_with_directives(tok_types, directive_kinds, directive_values)
    }));
    let eval = result.expect("endif-without-open must return an error, not panic");
    let err = eval.expect_err("expected reference evaluation failure");
        assert!(
            err.to_string().contains("reference dispatch trapped"),
            "unexpected error: {err}"
        );
}

// ---------------------------------------------------------------------------
// 4. Directive metadata contracts
// ---------------------------------------------------------------------------

#[test]
fn directive_metadata_kinds_map_to_stable_token_ids() {
    let cases: &[(CPreprocessorDirectiveKind, u32)] = &[
        (CPreprocessorDirectiveKind::Null, TOK_PP_NULL),
        (CPreprocessorDirectiveKind::Define, TOK_PP_DEFINE),
        (CPreprocessorDirectiveKind::Undef, TOK_PP_UNDEF),
        (CPreprocessorDirectiveKind::Include, TOK_PP_INCLUDE),
        (CPreprocessorDirectiveKind::If, TOK_PP_IF),
        (CPreprocessorDirectiveKind::Ifdef, TOK_PP_IFDEF),
        (CPreprocessorDirectiveKind::Ifndef, TOK_PP_IFNDEF),
        (CPreprocessorDirectiveKind::Elif, TOK_PP_ELIF),
        (CPreprocessorDirectiveKind::Else, TOK_PP_ELSE),
        (CPreprocessorDirectiveKind::Endif, TOK_PP_ENDIF),
        (CPreprocessorDirectiveKind::Pragma, TOK_PP_PRAGMA),
        (CPreprocessorDirectiveKind::Line, TOK_PP_LINE),
        (CPreprocessorDirectiveKind::Error, TOK_PP_ERROR),
    ];
    for (kind, expected) in cases {
        assert_eq!(
            kind.token_id(),
            *expected,
            "token_id mismatch for {:?}",
            kind
        );
    }
}

#[test]
fn directive_metadata_classifies_spliced_rows() {
    let source = b"#def\
ine FOO 1\n";
    let spliced = c_translation_phase_line_splice(source);
    // After splicing: "#define FOO 1\n"
    let (kinds, values) = reference_c_preprocessor_directive_metadata(
        &[TOK_PREPROC],
        &[0],
        &[spliced.bytes.len() as u32],
        &spliced.bytes,
        &[],
    )
    .expect("spliced directive must classify");
    assert_eq!(kinds, vec![TOK_PP_DEFINE]);
    assert_eq!(values, vec![0]);
}

