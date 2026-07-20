//! GPU `#ifdef` / `#ifndef` evaluator reference roundtrip.
//!
//! Asserts the 17b.1 kernel emits `1`/`0` for each `ifdef`/`ifndef`
//! token matching what the CPU
//! `reference_c_preprocessor_directive_metadata` produces. Other
//! directive kinds must remain `0` in this kernel's output column.

#![cfg(feature = "c-parser")]
#![allow(deprecated)]
use vyre_libs::parsing::c::lex::tokens::{TOK_PP_IFDEF, TOK_PP_IFNDEF, TOK_PREPROC};
use vyre_libs::parsing::c::preprocess::gpu_directive_metadata::gpu_directive_metadata;
use vyre_libs::parsing::c::preprocess::gpu_ifdef_value::gpu_ifdef_value;
use vyre_libs::parsing::c::preprocess::reference_c_preprocessor_directive_metadata;
use vyre_libs::scan::dispatch_io::pack_u32_slice as pack_u32_le;
use vyre_reference::value::Value;

fn unpack_u32(bytes: &[u8]) -> Vec<u32> {
    bytes
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

/// Mirror of the helper in gpu_directive_metadata_roundtrip  -  emit
/// TOK_PREPROC tokens for directive rows and a sentinel `0` token per
/// non-directive byte.
fn build_token_stream(source: &[u8]) -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let mut tok_types = Vec::new();
    let mut tok_starts = Vec::new();
    let mut tok_lens = Vec::new();
    let mut i = 0usize;
    let mut at_line_start = true;
    while i < source.len() {
        if at_line_start {
            // Find first non-horizontal-whitespace.
            let mut j = i;
            while j < source.len() && matches!(source[j], b' ' | b'\t') {
                j += 1;
            }
            if j < source.len() && source[j] == b'#' {
                let row_start = i;
                let mut row_end = j;
                while row_end < source.len() {
                    if source[row_end] == b'\n' {
                        if row_end > row_start && source[row_end - 1] == b'\\' {
                            row_end += 1;
                            continue;
                        }
                        break;
                    }
                    if source[row_end] == b'\r' {
                        if row_end > row_start && source[row_end - 1] == b'\\' {
                            row_end += 1;
                            if row_end < source.len() && source[row_end] == b'\n' {
                                row_end += 1;
                            }
                            continue;
                        }
                        break;
                    }
                    row_end += 1;
                }
                tok_types.push(TOK_PREPROC);
                tok_starts.push(row_start as u32);
                tok_lens.push((row_end - row_start) as u32);
                i = row_end;
                at_line_start = true;
                continue;
            }
        }
        if source[i] == b'\n' || source[i] == b'\r' {
            at_line_start = true;
            i += 1;
            continue;
        }
        tok_types.push(0);
        tok_starts.push(i as u32);
        tok_lens.push(1);
        i += 1;
        at_line_start = false;
    }
    (tok_types, tok_starts, tok_lens)
}

/// Pack defined-macro names into the (names_packed, offsets) layout the
/// kernel expects.
fn pack_defined_macros(names: &[&[u8]]) -> (Vec<u8>, Vec<u32>) {
    let mut packed = Vec::new();
    let mut offsets = Vec::with_capacity(names.len() + 1);
    offsets.push(0u32);
    for name in names {
        packed.extend_from_slice(name);
        offsets.push(packed.len() as u32);
    }
    (packed, offsets)
}

fn run_full_pipeline(source: &[u8], defined_macros: &[&[u8]]) -> (Vec<u32>, Vec<u32>) {
    let (tok_types, tok_starts, tok_lens) = build_token_stream(source);
    let n = tok_types.len();
    let n_padded = n.max(1);
    // `source` and `macro_names_packed` are declared as packed U32
    // words in their kernels; pad inputs to multiple-of-4 bytes.
    let src_padded = (source.len().div_ceil(4) * 4).max(4);

    // ---- Stage 1: directive_kinds via gpu_directive_metadata ----
    let mut tt = pack_u32_le(&tok_types);
    tt.resize(n_padded * 4, 0);
    let mut ts = pack_u32_le(&tok_starts);
    ts.resize(n_padded * 4, 0);
    let mut tl = pack_u32_le(&tok_lens);
    tl.resize(n_padded * 4, 0);
    let mut src = source.to_vec();
    src.resize(src_padded, 0);
    let dk_init = vec![0u8; n_padded * 4];
    let dv_init_a = vec![0u8; n_padded * 4];

    let prog_a = gpu_directive_metadata(n as u32, source.len() as u32);
    let outputs_a = vyre_reference::reference_eval(
        &prog_a,
        &[
            Value::from(tt),
            Value::from(ts.clone()),
            Value::from(tl.clone()),
            Value::from(src.clone()),
            Value::from(dk_init),
            Value::from(dv_init_a),
        ],
    )
    .expect("17a kernel eval");
    let mut directive_kinds_bytes = outputs_a[0].to_bytes().to_vec();
    directive_kinds_bytes.resize(n_padded * 4, 0);

    // ---- Stage 2: directive_values via gpu_ifdef_value ----
    let (macro_names, macro_offsets_words) = pack_defined_macros(defined_macros);
    let mut macro_names_padded = macro_names.clone();
    let macro_names_pad_len = (macro_names.len().div_ceil(4) * 4).max(4);
    macro_names_padded.resize(macro_names_pad_len, 0);
    let macro_offsets_bytes = pack_u32_le(&macro_offsets_words);

    let dv_init_b = vec![0u8; n_padded * 4];
    let prog_b = gpu_ifdef_value(n as u32, source.len() as u32);
    let outputs_b = vyre_reference::reference_eval(
        &prog_b,
        &[
            Value::from(ts),
            Value::from(tl),
            Value::from(directive_kinds_bytes.clone()),
            Value::from(src),
            Value::from(macro_names_padded),
            Value::from(macro_offsets_bytes),
            Value::from(dv_init_b),
        ],
    )
    .expect("17b.1 kernel eval");

    let mut kinds = unpack_u32(&directive_kinds_bytes);
    kinds.truncate(n);
    let mut values = unpack_u32(&outputs_b[0].to_bytes());
    values.truncate(n);
    (kinds, values)
}

fn cpu_kinds_and_values(source: &[u8], defined_macros: &[&[u8]]) -> (Vec<u32>, Vec<u32>) {
    let (tok_types, tok_starts, tok_lens) = build_token_stream(source);
    reference_c_preprocessor_directive_metadata(
        &tok_types,
        &tok_starts,
        &tok_lens,
        source,
        defined_macros,
    )
    .expect("Reference oracle eval")
}

/// Filter values to only keep ifdef/ifndef rows so we don't get
/// confused by 17b.4 work that hasn't shipped (the Reference oracle
/// computes `if`/`elif` values too; this kernel returns 0 for them).
fn keep_only_ifdef_ifndef(kinds: &[u32], values: &[u32]) -> Vec<u32> {
    kinds
        .iter()
        .zip(values)
        .map(|(k, v)| {
            if *k == TOK_PP_IFDEF || *k == TOK_PP_IFNDEF {
                *v
            } else {
                0
            }
        })
        .collect()
}

#[test]
fn ifdef_returns_one_when_macro_is_defined() {
    let src = b"#ifdef FOO\n";
    let defined = [b"FOO".as_slice()];
    let (kinds, gpu_values) = run_full_pipeline(src, &defined);
    let (cpu_kinds, cpu_values) = cpu_kinds_and_values(src, &defined);
    assert_eq!(kinds, cpu_kinds);
    assert_eq!(gpu_values, keep_only_ifdef_ifndef(&cpu_kinds, &cpu_values));
    assert_eq!(gpu_values, vec![1]);
}

#[test]
fn ifdef_returns_zero_when_macro_is_undefined() {
    let src = b"#ifdef MISSING\n";
    let defined = [b"FOO".as_slice()];
    let (kinds, gpu_values) = run_full_pipeline(src, &defined);
    let (cpu_kinds, cpu_values) = cpu_kinds_and_values(src, &defined);
    assert_eq!(kinds, cpu_kinds);
    assert_eq!(gpu_values, keep_only_ifdef_ifndef(&cpu_kinds, &cpu_values));
    assert_eq!(gpu_values, vec![0]);
}

#[test]
fn ifndef_returns_one_when_macro_is_undefined() {
    let src = b"#ifndef NEWHEADER\n";
    let defined = [b"FOO".as_slice()];
    let (_kinds, gpu_values) = run_full_pipeline(src, &defined);
    assert_eq!(gpu_values, vec![1]);
}

#[test]
fn ifndef_returns_zero_when_macro_is_defined() {
    let src = b"#ifndef FOO\n";
    let defined = [b"FOO".as_slice()];
    let (_kinds, gpu_values) = run_full_pipeline(src, &defined);
    assert_eq!(gpu_values, vec![0]);
}

#[test]
fn macro_with_underscore_and_digits_is_matched_byte_for_byte() {
    let src = b"#ifdef HAVE_LIB_2\n";
    let defined = [b"HAVE_LIB_2".as_slice()];
    let (_kinds, gpu_values) = run_full_pipeline(src, &defined);
    assert_eq!(gpu_values, vec![1]);
}

#[test]
fn long_ifdef_identifier_is_not_truncated() {
    let name = format!("CONFIG_{}_FEATURE", "LONG_".repeat(40));
    let source = format!("#ifdef {name}\n");
    let defined = [name.as_bytes()];

    let (_kinds, gpu_values) = run_full_pipeline(source.as_bytes(), &defined);

    assert_eq!(gpu_values, vec![1]);
}

#[test]
fn long_ifndef_identifier_matches_full_name_before_inverting() {
    let name = format!("HAVE_{}_HEADER", "GENERATED_".repeat(32));
    let source = format!("#ifndef {name}\n");
    let defined = [name.as_bytes()];

    let (_kinds, gpu_values) = run_full_pipeline(source.as_bytes(), &defined);

    assert_eq!(gpu_values, vec![0]);
}

#[test]
fn macro_substring_match_does_not_count_as_defined() {
    // The defined name FOO is NOT a substring match for FOOBAR  -  must
    // be a full byte-for-byte equality.
    let src = b"#ifdef FOOBAR\n";
    let defined = [b"FOO".as_slice()];
    let (_kinds, gpu_values) = run_full_pipeline(src, &defined);
    assert_eq!(gpu_values, vec![0]);
}

#[test]
fn extra_horizontal_whitespace_between_directive_and_name() {
    let src = b"#ifdef    SPACED\n";
    let defined = [b"SPACED".as_slice()];
    let (_kinds, gpu_values) = run_full_pipeline(src, &defined);
    assert_eq!(gpu_values, vec![1]);
}

#[test]
fn other_directive_kinds_emit_zero_in_value_column() {
    let src = b"#define X 1\n#include <foo.h>\n#pragma once\n";
    let defined: [&[u8]; 0] = [];
    let (_kinds, gpu_values) = run_full_pipeline(src, &defined);
    assert!(
        gpu_values.iter().all(|&v| v == 0),
        "non-ifdef/ifndef rows must emit 0; got {gpu_values:?}"
    );
}

#[test]
fn dense_block_with_mixed_defined_undefined_macros() {
    let src = b"#ifdef A\n#ifndef B\n#ifdef C\n#ifndef D\n";
    let defined = [b"A".as_slice(), b"C".as_slice()];
    let (_kinds, gpu_values) = run_full_pipeline(src, &defined);
    // A defined → 1; B undefined → 1; C defined → 1; D undefined → 1.
    assert_eq!(gpu_values, vec![1, 1, 1, 1]);
}

#[test]
fn empty_macro_table_means_every_ifdef_is_zero_and_ifndef_is_one() {
    let src = b"#ifdef X\n#ifndef Y\n";
    let defined: [&[u8]; 0] = [];
    let (_kinds, gpu_values) = run_full_pipeline(src, &defined);
    assert_eq!(gpu_values, vec![0, 1]);
}
