//! GPU `#define` row parser reference roundtrip.
//!
//! Pins the kernel against ground-truth name/args/body byte spans for
//! object-like and function-like macros, including edge cases:
//! empty body, leading/trailing whitespace, function-like with no
//! args, function-like with multiple args, indented `#`.

#![cfg(feature = "c-parser")]
#![allow(deprecated)]
use vyre_libs::parsing::c::lex::tokens::TOK_PREPROC;
use vyre_libs::parsing::c::preprocess::gpu_define_parse::gpu_define_parse;
use vyre_libs::parsing::c::preprocess::gpu_directive_metadata::gpu_directive_metadata;
use vyre_libs::scan::dispatch_io::pack_u32_slice as pack_u32_le;
use vyre_reference::value::Value;

fn unpack_u32(bytes: &[u8]) -> Vec<u32> {
    bytes
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

fn build_token_stream(source: &[u8]) -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let mut tt = Vec::new();
    let mut ts = Vec::new();
    let mut tl = Vec::new();
    let mut i = 0usize;
    let mut at_line_start = true;
    while i < source.len() {
        if at_line_start {
            let mut j = i;
            while j < source.len() && matches!(source[j], b' ' | b'\t') {
                j += 1;
            }
            if j < source.len() && source[j] == b'#' {
                let row_start = i;
                let mut row_end = j;
                while row_end < source.len() && source[row_end] != b'\n' {
                    row_end += 1;
                }
                tt.push(TOK_PREPROC);
                ts.push(row_start as u32);
                tl.push((row_end - row_start) as u32);
                i = row_end;
                at_line_start = true;
                continue;
            }
        }
        if source[i] == b'\n' {
            at_line_start = true;
            i += 1;
            continue;
        }
        tt.push(0);
        ts.push(i as u32);
        tl.push(1);
        i += 1;
        at_line_start = false;
    }
    (tt, ts, tl)
}

#[derive(Debug, PartialEq, Eq)]
struct DefineRow {
    name: Vec<u8>,
    args: Vec<u8>,
    body: Vec<u8>,
    is_func: bool,
}

fn run_pipeline(source: &[u8]) -> Vec<Option<DefineRow>> {
    let (tt, ts, tl) = build_token_stream(source);
    let n = tt.len();
    let n_pad = n.max(1);
    // `source` is declared as packed U32 words in both
    // gpu_directive_metadata and gpu_define_parse (so reference-eval
    // and naga-emitted real GPU agree on word-indexed access). Pad
    // input bytes to a multiple of 4 to fill the last word.
    let src_pad = (source.len().div_ceil(4) * 4).max(4);

    // Stage 1: directive_kinds.
    let mut tt_b = pack_u32_le(&tt);
    tt_b.resize(n_pad * 4, 0);
    let mut ts_b = pack_u32_le(&ts);
    ts_b.resize(n_pad * 4, 0);
    let mut tl_b = pack_u32_le(&tl);
    tl_b.resize(n_pad * 4, 0);
    let mut src = source.to_vec();
    src.resize(src_pad, 0);
    let dk_init = vec![0u8; n_pad * 4];
    let dv_init = vec![0u8; n_pad * 4];

    let prog_a = gpu_directive_metadata(n as u32, source.len() as u32);
    let outputs_a = vyre_reference::reference_eval(
        &prog_a,
        &[
            Value::from(tt_b),
            Value::from(ts_b.clone()),
            Value::from(tl_b.clone()),
            Value::from(src.clone()),
            Value::from(dk_init),
            Value::from(dv_init),
        ],
    )
    .expect("17a kernel eval");
    let mut dk_bytes = outputs_a[0].to_bytes().to_vec();
    dk_bytes.resize(n_pad * 4, 0);

    // Stage 2: define parse.
    let prog_b = gpu_define_parse(n as u32, source.len() as u32);
    let outs = vyre_reference::reference_eval(
        &prog_b,
        &[
            Value::from(ts_b),
            Value::from(tl_b),
            Value::from(dk_bytes),
            Value::from(src),
            Value::from(vec![0u8; n_pad * 4]),
            Value::from(vec![0u8; n_pad * 4]),
            Value::from(vec![0u8; n_pad * 4]),
            Value::from(vec![0u8; n_pad * 4]),
            Value::from(vec![0u8; n_pad * 4]),
            Value::from(vec![0u8; n_pad * 4]),
            Value::from(vec![0u8; n_pad * 4]),
        ],
    )
    .expect("17b.6 kernel eval");
    let name_s = unpack_u32(&outs[0].to_bytes());
    let name_l = unpack_u32(&outs[1].to_bytes());
    let args_s = unpack_u32(&outs[2].to_bytes());
    let args_l = unpack_u32(&outs[3].to_bytes());
    let body_s = unpack_u32(&outs[4].to_bytes());
    let body_l = unpack_u32(&outs[5].to_bytes());
    let is_f = unpack_u32(&outs[6].to_bytes());

    (0..n)
        .map(|i| {
            if name_l[i] == 0 {
                None
            } else {
                let nb = name_s[i] as usize;
                let nl = name_l[i] as usize;
                let ab = args_s[i] as usize;
                let al = args_l[i] as usize;
                let bb = body_s[i] as usize;
                let bl = body_l[i] as usize;
                Some(DefineRow {
                    name: source[nb..nb + nl].to_vec(),
                    args: if al == 0 {
                        Vec::new()
                    } else {
                        source[ab..ab + al].to_vec()
                    },
                    body: if bl == 0 {
                        Vec::new()
                    } else {
                        source[bb..bb + bl].to_vec()
                    },
                    is_func: is_f[i] == 1,
                })
            }
        })
        .collect()
}

fn first_define(source: &[u8]) -> DefineRow {
    let rows = run_pipeline(source);
    rows.into_iter()
        .flatten()
        .next()
        .expect("expected at least one #define row")
}

#[test]
fn object_like_simple() {
    let r = first_define(b"#define FOO 1\n");
    assert_eq!(r.name, b"FOO");
    assert!(r.args.is_empty());
    assert_eq!(r.body, b"1");
    assert!(!r.is_func);
}

#[test]
fn object_like_no_body() {
    let r = first_define(b"#define FOO\n");
    assert_eq!(r.name, b"FOO");
    assert!(r.args.is_empty());
    assert!(r.body.is_empty());
    assert!(!r.is_func);
}

#[test]
fn object_like_multiword_body() {
    let r = first_define(b"#define PI 3.14\n");
    assert_eq!(r.name, b"PI");
    assert_eq!(r.body, b"3.14");
}

#[test]
fn object_like_with_underscore_and_digits_in_name() {
    let r = first_define(b"#define HAVE_LIB_2 1\n");
    assert_eq!(r.name, b"HAVE_LIB_2");
}

#[test]
fn object_like_long_macro_name_is_not_truncated() {
    let name = format!("MACRO_{}", "A".repeat(160));
    let source = format!("#define {name} 1\n");

    let r = first_define(source.as_bytes());

    assert_eq!(r.name, name.as_bytes());
    assert_eq!(r.body, b"1");
}

#[test]
fn macro_name_starting_with_digit_is_rejected() {
    let rows = run_pipeline(b"#define 1BAD 9\n");

    assert!(
        rows.iter().all(Option::is_none),
        "C macro identifiers must not start with a digit"
    );
}

#[test]
fn function_like_no_args() {
    let r = first_define(b"#define FOO() 1\n");
    assert_eq!(r.name, b"FOO");
    assert!(r.args.is_empty());
    assert_eq!(r.body, b"1");
    assert!(r.is_func);
}

#[test]
fn function_like_one_arg() {
    let r = first_define(b"#define SQ(x) ((x)*(x))\n");
    assert_eq!(r.name, b"SQ");
    assert_eq!(r.args, b"x");
    assert_eq!(r.body, b"((x)*(x))");
    assert!(r.is_func);
}

#[test]
fn function_like_multi_arg() {
    let r = first_define(b"#define MAX(a,b) ((a)>(b)?(a):(b))\n");
    assert_eq!(r.name, b"MAX");
    assert_eq!(r.args, b"a,b");
    assert_eq!(r.body, b"((a)>(b)?(a):(b))");
    assert!(r.is_func);
}

#[test]
fn function_like_args_with_whitespace() {
    let r = first_define(b"#define ADD(a, b) (a+b)\n");
    assert_eq!(r.name, b"ADD");
    assert_eq!(r.args, b"a, b");
    assert_eq!(r.body, b"(a+b)");
}

#[test]
fn function_like_long_arg_list_is_not_truncated() {
    let args = (0..80)
        .map(|index| format!("a{index}"))
        .collect::<Vec<_>>()
        .join(",");
    let source = format!("#define MANY({args}) body\n");

    let r = first_define(source.as_bytes());

    assert_eq!(r.name, b"MANY");
    assert_eq!(r.args, args.as_bytes());
    assert_eq!(r.body, b"body");
    assert!(r.is_func);
}

#[test]
fn extra_whitespace_after_define_keyword() {
    let r = first_define(b"#define   FOO   42\n");
    assert_eq!(r.name, b"FOO");
    assert_eq!(r.body, b"42");
}

#[test]
fn indented_hash() {
    let r = first_define(b"   #define INDENTED 1\n");
    assert_eq!(r.name, b"INDENTED");
    assert_eq!(r.body, b"1");
}

#[test]
fn space_between_hash_and_define() {
    let r = first_define(b"# define SPACED 1\n");
    assert_eq!(r.name, b"SPACED");
    assert_eq!(r.body, b"1");
}

#[test]
fn body_with_trailing_whitespace_is_trimmed() {
    let r = first_define(b"#define X foo   \n");
    assert_eq!(r.body, b"foo");
}

#[test]
fn non_define_row_emits_zero_name_len() {
    let rows = run_pipeline(b"#include <stdio.h>\n#pragma once\n");
    assert!(rows.iter().all(|r| r.is_none()));
}

#[test]
fn mixed_directives_only_define_rows_have_names() {
    let rows = run_pipeline(b"#define A 1\n#include <foo.h>\n#define B 2\n");
    let defines: Vec<_> = rows.into_iter().flatten().collect();
    assert_eq!(defines.len(), 2);
    assert_eq!(defines[0].name, b"A");
    assert_eq!(defines[1].name, b"B");
}
