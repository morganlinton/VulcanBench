//! GPU `#include` row parser reference roundtrip.

#![cfg(feature = "c-parser")]
#![allow(deprecated)]
use vyre_libs::parsing::c::lex::tokens::TOK_PREPROC;
use vyre_libs::parsing::c::preprocess::gpu_directive_metadata::gpu_directive_metadata;
use vyre_libs::parsing::c::preprocess::gpu_include_parse::gpu_include_parse;
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
struct IncludeRow {
    path: Vec<u8>,
    is_system: bool,
}

fn run_pipeline(source: &[u8]) -> Vec<Option<IncludeRow>> {
    let (tt, ts, tl) = build_token_stream(source);
    let n = tt.len();
    let n_pad = n.max(1);
    // `source` is now declared as packed U32 words; pad to multiple
    // of 4 bytes.
    let src_pad = (source.len().div_ceil(4) * 4).max(4);

    let mut tt_b = pack_u32_le(&tt);
    tt_b.resize(n_pad * 4, 0);
    let mut ts_b = pack_u32_le(&ts);
    ts_b.resize(n_pad * 4, 0);
    let mut tl_b = pack_u32_le(&tl);
    tl_b.resize(n_pad * 4, 0);
    let mut src = source.to_vec();
    src.resize(src_pad, 0);

    let prog_a = gpu_directive_metadata(n as u32, source.len() as u32);
    let outputs_a = vyre_reference::reference_eval(
        &prog_a,
        &[
            Value::from(tt_b),
            Value::from(ts_b.clone()),
            Value::from(tl_b.clone()),
            Value::from(src.clone()),
            Value::from(vec![0u8; n_pad * 4]),
            Value::from(vec![0u8; n_pad * 4]),
        ],
    )
    .expect("17a kernel eval");
    let mut dk_bytes = outputs_a[0].to_bytes().to_vec();
    dk_bytes.resize(n_pad * 4, 0);

    let prog_b = gpu_include_parse(n as u32, source.len() as u32);
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
        ],
    )
    .expect("17b.7 kernel eval");
    let path_s = unpack_u32(&outs[0].to_bytes());
    let path_l = unpack_u32(&outs[1].to_bytes());
    let is_sys = unpack_u32(&outs[2].to_bytes());

    (0..n)
        .map(|i| {
            if path_l[i] == 0 {
                None
            } else {
                let s = path_s[i] as usize;
                let l = path_l[i] as usize;
                Some(IncludeRow {
                    path: source[s..s + l].to_vec(),
                    is_system: is_sys[i] == 1,
                })
            }
        })
        .collect()
}

fn first_include(source: &[u8]) -> IncludeRow {
    run_pipeline(source)
        .into_iter()
        .flatten()
        .next()
        .expect("expected an #include")
}

#[test]
fn system_include() {
    assert_eq!(
        first_include(b"#include <stdio.h>\n"),
        IncludeRow {
            path: b"stdio.h".to_vec(),
            is_system: true
        },
    );
}

#[test]
fn local_include() {
    assert_eq!(
        first_include(b"#include \"foo.h\"\n"),
        IncludeRow {
            path: b"foo.h".to_vec(),
            is_system: false
        },
    );
}

#[test]
fn system_include_with_path_components() {
    assert_eq!(
        first_include(b"#include <linux/kernel.h>\n"),
        IncludeRow {
            path: b"linux/kernel.h".to_vec(),
            is_system: true
        },
    );
}

#[test]
fn local_include_with_subdir() {
    assert_eq!(
        first_include(b"#include \"util/list.h\"\n"),
        IncludeRow {
            path: b"util/list.h".to_vec(),
            is_system: false
        },
    );
}

#[test]
fn long_system_include_path_is_not_truncated() {
    let path = format!(
        "linux/generated/{}/uapi/asm-offsets-autogen.h",
        "deep".repeat(32)
    );
    let source = format!("#include <{path}>\n");

    let row = first_include(source.as_bytes());

    assert_eq!(row.path, path.as_bytes());
    assert!(row.is_system);
}

#[test]
fn long_local_include_path_is_not_truncated() {
    let path = format!(
        "drivers/{}/include/config/generated_header.h",
        "nested/".repeat(24)
    );
    let source = format!("#include \"{path}\"\n");

    let row = first_include(source.as_bytes());

    assert_eq!(row.path, path.as_bytes());
    assert!(!row.is_system);
}

#[test]
fn include_next_system() {
    assert_eq!(
        first_include(b"#include_next <stdio.h>\n"),
        IncludeRow {
            path: b"stdio.h".to_vec(),
            is_system: true
        },
    );
}

#[test]
fn extra_whitespace_after_keyword() {
    assert_eq!(
        first_include(b"#include    <stdint.h>\n"),
        IncludeRow {
            path: b"stdint.h".to_vec(),
            is_system: true
        },
    );
}

#[test]
fn indented_hash() {
    assert_eq!(
        first_include(b"   #include <stddef.h>\n"),
        IncludeRow {
            path: b"stddef.h".to_vec(),
            is_system: true
        },
    );
}

#[test]
fn space_between_hash_and_include() {
    assert_eq!(
        first_include(b"# include <stddef.h>\n"),
        IncludeRow {
            path: b"stddef.h".to_vec(),
            is_system: true
        },
    );
}

#[test]
fn non_include_row_emits_zero_path_len() {
    let rows = run_pipeline(b"#define X 1\n#pragma once\n");
    assert!(rows.iter().all(|r| r.is_none()));
}

#[test]
fn mixed_directives_only_include_rows_have_paths() {
    let rows =
        run_pipeline(b"#define A 1\n#include <stdio.h>\n#include \"local.h\"\n#define B 2\n");
    let includes: Vec<_> = rows.into_iter().flatten().collect();
    assert_eq!(includes.len(), 2);
    assert_eq!(includes[0].path, b"stdio.h");
    assert!(includes[0].is_system);
    assert_eq!(includes[1].path, b"local.h");
    assert!(!includes[1].is_system);
}

#[test]
fn empty_path_in_quotes_is_zero_length() {
    // `#include ""` parses path_l=0, which we surface as "no parsed
    // include row"  -  matches the CPU fail-soft behaviour.
    let rows = run_pipeline(b"#include \"\"\n");
    assert!(rows.iter().all(|r| r.is_none()));
}
