//! GPU directive-metadata reference roundtrip  -  validates the
//! 17a classifier kernel against the CPU `reference_c_preprocessor_directive_metadata`.
//!
//! For phase 17a only the `directive_kinds` column is checked. The
//! `directive_values` column is asserted to be all-zero (the CPU
//! reference may compute non-zero conditional values which 17b will
//! match; for now the kernel emits 0).

#![cfg(feature = "c-parser")]
#![allow(deprecated)]
use vyre_libs::parsing::c::lex::tokens::TOK_PREPROC;
use vyre_libs::parsing::c::preprocess::gpu_directive_metadata::{
    gpu_directive_metadata, gpu_directive_metadata_u8,
};
use vyre_libs::parsing::c::preprocess::reference_c_preprocessor_directive_metadata;
use vyre_libs::scan::dispatch_io::pack_u32_slice as pack_u32_le;
use vyre_reference::value::Value;

fn unpack_u32(bytes: &[u8]) -> Vec<u32> {
    bytes
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

/// Build (tok_types, tok_starts, tok_lens) by scanning `source` for
/// each `#…<newline>` directive row. Non-directive bytes are not
/// tokenized  -  we emit one TOK_PREPROC token per directive row plus
/// optional sentinel non-PREPROC tokens for the gaps so the kernel sees
/// a realistic mixed stream.
fn build_token_stream(source: &[u8]) -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let mut tok_types = Vec::new();
    let mut tok_starts = Vec::new();
    let mut tok_lens = Vec::new();
    let mut i = 0usize;
    let mut at_line_start = true;
    while i < source.len() {
        if at_line_start
            && (source[i] == b'#'
                || source[i..]
                    .iter()
                    .take_while(|b| matches!(b, b' ' | b'\t'))
                    .count()
                    > 0
                    && source[i + source[i..]
                        .iter()
                        .take_while(|b| matches!(b, b' ' | b'\t'))
                        .count()
                        .min(source.len() - i)]
                        == b'#')
        {
            // Find the row end: nearest unsplit newline.
            let row_start = i;
            let mut row_end = row_start;
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
        if source[i] == b'\n' || source[i] == b'\r' {
            at_line_start = true;
            i += 1;
            continue;
        }
        // Emit a single non-PREPROC sentinel token covering this byte
        // so the kernel sees mixed token kinds.
        tok_types.push(0); // Use 0 as a non-TOK_PREPROC sentinel.
        tok_starts.push(i as u32);
        tok_lens.push(1);
        i += 1;
        at_line_start = false;
    }
    (tok_types, tok_starts, tok_lens)
}

fn run_gpu_kernel(source: &[u8]) -> (Vec<u32>, Vec<u32>) {
    let (tok_types, tok_starts, tok_lens) = build_token_stream(source);
    let n = tok_types.len();
    let n_padded = n.max(1);
    // The kernel declares `source` as packed U32 words (so reference-
    // eval and naga-emitted real GPU agree on word-indexed access).
    // Pad the input bytes to a multiple of 4 to fill the last word.
    let src_padded = source.len().div_ceil(4) * 4;
    let src_padded = src_padded.max(4);

    let mut tt = pack_u32_le(&tok_types);
    tt.resize(n_padded * 4, 0);
    let mut ts = pack_u32_le(&tok_starts);
    ts.resize(n_padded * 4, 0);
    let mut tl = pack_u32_le(&tok_lens);
    tl.resize(n_padded * 4, 0);
    let mut src = source.to_vec();
    src.resize(src_padded, 0);
    let dk_init = vec![0u8; n_padded * 4];
    let dv_init = vec![0u8; n_padded * 4];

    let prog = gpu_directive_metadata(n as u32, source.len() as u32);
    let outputs = vyre_reference::reference_eval(
        &prog,
        &[
            Value::from(tt),
            Value::from(ts),
            Value::from(tl),
            Value::from(src),
            Value::from(dk_init),
            Value::from(dv_init),
        ],
    )
    .expect("gpu_directive_metadata reference eval");

    let mut kinds = unpack_u32(&outputs[0].to_bytes());
    kinds.truncate(n);
    let mut values = unpack_u32(&outputs[1].to_bytes());
    values.truncate(n);
    (kinds, values)
}

fn run_gpu_kernel_u8(source: &[u8]) -> (Vec<u32>, Vec<u32>) {
    let (tok_types, tok_starts, tok_lens) = build_token_stream(source);
    let n = tok_types.len();
    let n_padded = n.max(1);

    let mut tt = pack_u32_le(&tok_types);
    tt.resize(n_padded * 4, 0);
    let mut ts = pack_u32_le(&tok_starts);
    ts.resize(n_padded * 4, 0);
    let mut tl = pack_u32_le(&tok_lens);
    tl.resize(n_padded * 4, 0);
    let dk_init = vec![0u8; n_padded * 4];
    let dv_init = vec![0u8; n_padded * 4];

    let prog = gpu_directive_metadata_u8(n as u32, source.len() as u32);
    let outputs = vyre_reference::reference_eval(
        &prog,
        &[
            Value::from(tt),
            Value::from(ts),
            Value::from(tl),
            Value::from(source.to_vec()),
            Value::from(dk_init),
            Value::from(dv_init),
        ],
    )
    .expect("gpu_directive_metadata_u8 reference eval");

    let mut kinds = unpack_u32(&outputs[0].to_bytes());
    kinds.truncate(n);
    let mut values = unpack_u32(&outputs[1].to_bytes());
    values.truncate(n);
    (kinds, values)
}

fn run_cpu_kernel(source: &[u8]) -> Vec<u32> {
    let (tok_types, tok_starts, tok_lens) = build_token_stream(source);
    let (kinds, _values) = reference_c_preprocessor_directive_metadata(
        &tok_types,
        &tok_starts,
        &tok_lens,
        source,
        &[],
    )
    .expect("Reference oracle eval");
    kinds
}

#[test]
fn gpu_classifies_define_to_pp_define() {
    let src = b"#define FOO 1\n";
    let (gpu_kinds, _gpu_values) = run_gpu_kernel(src);
    let cpu_kinds = run_cpu_kernel(src);
    assert_eq!(gpu_kinds, cpu_kinds, "GPU vs CPU directive_kinds mismatch");
}

#[test]
fn gpu_classifies_undef_to_pp_undef() {
    let src = b"#undef FOO\n";
    assert_eq!(run_gpu_kernel(src).0, run_cpu_kernel(src));
}

#[test]
fn gpu_classifies_include_system_form() {
    let src = b"#include <stdio.h>\n";
    assert_eq!(run_gpu_kernel(src).0, run_cpu_kernel(src));
}

#[test]
fn gpu_classifies_include_local_form() {
    let src = b"#include \"foo.h\"\n";
    assert_eq!(run_gpu_kernel(src).0, run_cpu_kernel(src));
}

#[test]
fn gpu_classifies_include_next() {
    let src = b"#include_next <stdio.h>\n";
    assert_eq!(run_gpu_kernel(src).0, run_cpu_kernel(src));
}

#[test]
fn gpu_classifies_if_ifdef_ifndef_separately() {
    let src = b"#if 1\n#ifdef FOO\n#ifndef BAR\n";
    assert_eq!(run_gpu_kernel(src).0, run_cpu_kernel(src));
}

#[test]
fn gpu_classifies_else_endif_elif() {
    let src = b"#if 1\n#elif 0\n#else\n#endif\n";
    assert_eq!(run_gpu_kernel(src).0, run_cpu_kernel(src));
}

#[test]
fn gpu_classifies_pragma_line_error_warning() {
    let src = b"#pragma once\n#line 42\n#error oops\n#warning watch out\n";
    assert_eq!(run_gpu_kernel(src).0, run_cpu_kernel(src));
}

#[test]
fn gpu_classifies_ident_and_sccs() {
    let src = b"#ident \"foo\"\n#sccs \"bar\"\n";
    assert_eq!(run_gpu_kernel(src).0, run_cpu_kernel(src));
}

#[test]
fn gpu_classifies_null_directive() {
    // `#` with nothing else on the line is the "null directive".
    let src = b"#\n";
    assert_eq!(run_gpu_kernel(src).0, run_cpu_kernel(src));
}

#[test]
fn gpu_handles_horizontal_whitespace_after_hash() {
    // `#   define X` is still a #define.
    let src = b"#   define X 1\n";
    assert_eq!(run_gpu_kernel(src).0, run_cpu_kernel(src));
}

#[test]
fn gpu_u8_source_classifies_dense_directive_block() {
    let src = b"#ifndef G\n#define G 1\n#include_next <linux/compiler.h>\n#pragma once\n#undef G\n#endif\n";
    assert_eq!(run_gpu_kernel_u8(src).0, run_cpu_kernel(src));
    assert!(run_gpu_kernel_u8(src).1.iter().all(|&value| value == 0));
}

#[test]
fn gpu_u8_source_handles_whitespace_and_mixed_tokens() {
    let src = b"int x;\n   #   define X 1\nint y;\n#warning watch\n";
    assert_eq!(run_gpu_kernel_u8(src).0, run_cpu_kernel(src));
}

#[test]
fn gpu_handles_leading_whitespace_before_hash() {
    let src = b"   #define INDENTED 1\n";
    assert_eq!(run_gpu_kernel(src).0, run_cpu_kernel(src));
}

#[test]
fn gpu_emits_zero_for_non_preproc_tokens() {
    // Non-directive tokens between two #defines must come out as 0.
    let src = b"#define A 1\nint x = 1;\n#define B 2\n";
    let (gpu_kinds, _) = run_gpu_kernel(src);
    let cpu_kinds = run_cpu_kernel(src);
    assert_eq!(gpu_kinds, cpu_kinds);
    // Sanity: there must be at least one zero entry from the int line.
    assert!(gpu_kinds.iter().any(|&k| k == 0));
    // And at least two non-zero entries from the two #define rows.
    assert!(gpu_kinds.iter().filter(|&&k| k != 0).count() >= 2);
}

#[test]
fn gpu_handles_dense_directive_block() {
    // A real-world preprocessor wall.
    let src = b"#ifndef GUARD\n#define GUARD\n#include <stddef.h>\n#define MAX(a,b) ((a)>(b)?(a):(b))\n#if defined(__GNUC__)\n#pragma GCC diagnostic push\n#endif\n#endif\n";
    assert_eq!(run_gpu_kernel(src).0, run_cpu_kernel(src));
}

#[test]
fn gpu_directive_metadata_keeps_values_column_zero() {
    // Directive metadata only classifies directive rows. Expression truth is
    // produced by the dedicated gpu_if_expression / gpu_ifdef_value stages.
    let src = b"#if 1\n#elif 0\n#endif\n";
    let (_kinds, values) = run_gpu_kernel(src);
    assert!(
        values.iter().all(|&v| v == 0),
        "directive metadata values column must be zero-filled (saw {values:?})"
    );
}

#[test]
fn gpu_does_not_misclassify_ident_starting_with_directive_prefix() {
    // `#defined` is NOT a #define directive  -  the keyword 'defined'
    // never appears at the directive-name slot. The Reference oracle
    // returns an error for this row; the GPU 17a kernel is tolerant
    // and emits 0 (caller treats 0 as "unrecognized"). Test asserts the
    // GPU's tolerance: kind_out is 0 for the unknown-keyword row.
    let src = b"#defined FOO\n";
    let (gpu_kinds, _) = run_gpu_kernel(src);
    assert_eq!(gpu_kinds, vec![0]);
}
