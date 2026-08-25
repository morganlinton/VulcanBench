use super::*;

#[test]
fn directive_metadata_evaluates_ifdef_truth() {
    let source = b"#ifdef FOO\n";
    let (kinds, values) = reference_c_preprocessor_directive_metadata(
        &[TOK_PREPROC],
        &[0],
        &[source.len() as u32],
        source,
        &[b"FOO".as_slice()],
    )
    .expect("ifdef must evaluate");
    assert_eq!(kinds, vec![TOK_PP_IFDEF]);
    assert_eq!(values, vec![1]);
}

#[test]
fn directive_metadata_evaluates_ifndef_truth() {
    let source = b"#ifndef FOO\n";
    let (kinds, values) = reference_c_preprocessor_directive_metadata(
        &[TOK_PREPROC],
        &[0],
        &[source.len() as u32],
        source,
        &[b"FOO".as_slice()],
    )
    .expect("ifndef must evaluate");
    assert_eq!(kinds, vec![TOK_PP_IFNDEF]);
    assert_eq!(values, vec![0]);
}

#[test]
fn directive_metadata_rejects_mismatched_stream_lengths() {
    let err = reference_c_preprocessor_directive_metadata(
        &[TOK_PREPROC, TOK_PREPROC],
        &[0],
        &[1],
        b"#if 1\n",
        &[],
    )
    .expect_err("length mismatch must fail loudly");
    assert_eq!(
        err.message,
        "Fix: token type/start/length streams must have identical lengths"
    );
}

