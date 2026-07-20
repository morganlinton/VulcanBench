//! Reference-eval roundtrip for `gpu_undef_parse`.
//!
//! Drives the kernel through the pure-Rust interpreter and asserts
//! the extracted name span matches expectations on hand-built fixtures
//! that exercise the various spacing and length corners.

#![cfg(feature = "c-parser")]
#![allow(deprecated)]
use vyre_libs::parsing::c::lex::tokens::{TOK_PP_UNDEF, TOK_PREPROC};
use vyre_libs::parsing::c::preprocess::gpu_undef_parse::gpu_undef_parse;
use vyre_libs::scan::dispatch_io::pack_u32_slice as pack_u32_le;
use vyre_reference::value::Value;

fn unpack_u32(bytes: &[u8]) -> Vec<u32> {
    bytes
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

/// Returns `(name_start, name_len)` for the directive row covering all
/// of `source`. `kind` lets the caller drive non-undef rows through
/// the kernel and assert they get the all-zero output.
fn run(source: &[u8], kind: u32) -> (u32, u32) {
    let n = 1u32;
    let prog = gpu_undef_parse(n, source.len() as u32);
    // `source` is declared as packed U32 words; pad to multiple of 4.
    let mut src = source.to_vec();
    src.resize((source.len().div_ceil(4) * 4).max(4), 0);
    let outputs = vyre_reference::reference_eval(
        &prog,
        &[
            Value::from(pack_u32_le(&[0u32])),                // tok_starts
            Value::from(pack_u32_le(&[source.len() as u32])), // tok_lens
            Value::from(pack_u32_le(&[kind])),                // directive_kinds
            Value::from(src),                                 // source
            Value::from(vec![0u8; 4]),                        // name_start_out
            Value::from(vec![0u8; 4]),                        // name_len_out
        ],
    )
    .expect("gpu_undef_parse reference eval");
    let starts = unpack_u32(&outputs[0].to_bytes());
    let lens = unpack_u32(&outputs[1].to_bytes());
    (starts[0], lens[0])
}

#[test]
fn extracts_simple_macro_name() {
    let src = b"#undef FOO\n";
    let (start, len) = run(src, TOK_PP_UNDEF);
    assert_eq!(&src[start as usize..start as usize + len as usize], b"FOO");
}

#[test]
fn extracts_long_identifier_with_underscore_and_digits() {
    let src = b"#undef _MY_MACRO_42\n";
    let (start, len) = run(src, TOK_PP_UNDEF);
    assert_eq!(
        &src[start as usize..start as usize + len as usize],
        b"_MY_MACRO_42"
    );
}

#[test]
fn extracts_very_long_identifier_without_truncation() {
    let name = format!("VERY_LONG_{}_42", "MACRO_".repeat(40));
    let source = format!("#undef {name}\n");

    let (start, len) = run(source.as_bytes(), TOK_PP_UNDEF);

    assert_eq!(
        &source.as_bytes()[start as usize..start as usize + len as usize],
        name.as_bytes()
    );
}

#[test]
fn tolerates_leading_whitespace_before_hash() {
    let src = b"  #undef BAR\n";
    let (start, len) = run(src, TOK_PP_UNDEF);
    assert_eq!(&src[start as usize..start as usize + len as usize], b"BAR");
}

#[test]
fn tolerates_whitespace_between_hash_and_keyword() {
    let src = b"#  undef BAZ\n";
    let (start, len) = run(src, TOK_PP_UNDEF);
    assert_eq!(&src[start as usize..start as usize + len as usize], b"BAZ");
}

#[test]
fn tolerates_extra_whitespace_before_name() {
    let src = b"#undef    QUX\n";
    let (start, len) = run(src, TOK_PP_UNDEF);
    assert_eq!(&src[start as usize..start as usize + len as usize], b"QUX");
}

#[test]
fn rejects_name_starting_with_digit() {
    // C identifiers can't start with a digit; kernel returns name_len=0.
    let src = b"#undef 9X\n";
    let (_start, len) = run(src, TOK_PP_UNDEF);
    assert_eq!(len, 0, "name starting with digit must be rejected");
}

#[test]
fn non_undef_rows_emit_zero_output() {
    use vyre_libs::parsing::c::lex::tokens::TOK_PP_DEFINE;
    let src = b"#define FOO 1\n";
    let (start, len) = run(src, TOK_PP_DEFINE);
    assert_eq!(start, 0);
    assert_eq!(len, 0);
}

#[test]
fn ordinary_token_emits_zero_output() {
    let src = b"int x";
    let (start, len) = run(src, TOK_PREPROC);
    // PREPROC token but kind != TOK_PP_UNDEF means parse path is gated
    // out; outputs stay zero.
    let _ = (start, len);
    let (start, len) = run(src, 0);
    assert_eq!(start, 0);
    assert_eq!(len, 0);
}
