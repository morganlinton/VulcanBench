// Focused real-GPU diagnostic for the directive-classify and
// include-parse kernels. Purpose: when `gpu_prepare_tu_source_e2e`
// diverges between reference-eval (253/253 green) and real GPU (6/8),
// these tests narrow the divergence to a specific kernel by running
// each one alone with hand-built inputs that match a known failing
// row.
//
// If `directive_kind_ifdef_missing` fails, the bug is in
// `gpu_directive_metadata`'s IR or its lowering. If it passes, the
// upstream `c11_lexer` or buffer round-trip in
// `gpu_pipeline::gpu_tokenize_and_classify` is corrupting inputs.
//
// If `include_parse_quoted_local` fails, the bug is in
// `gpu_include_parse`'s IR or lowering. If it passes,
// `gpu_extract_directive_payloads` host-side packing is the issue.

#[allow(unused_imports)]
use vyre_driver_wgpu as _;

use vyre::DispatchConfig;
use vyre::execution_plan::fusion::fuse_programs;
use vyre_libs::parsing::c::lex::tokens::{
    TOK_PP_DEFINE, TOK_PP_IFDEF, TOK_PP_IFNDEF, TOK_PP_INCLUDE, TOK_PP_UNDEF, TOK_PREPROC,
};
use vyre_libs::parsing::c::preprocess::gpu_define_parse::gpu_define_parse;
use vyre_libs::parsing::c::preprocess::gpu_directive_metadata::gpu_directive_metadata;
use vyre_libs::parsing::c::preprocess::gpu_ifdef_value::gpu_ifdef_value;
use vyre_libs::parsing::c::preprocess::gpu_include_parse::gpu_include_parse;
use vyre_libs::parsing::c::preprocess::gpu_undef_parse::gpu_undef_parse;

fn pack_u32_le(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|w| w.to_le_bytes()).collect()
}

fn unpack_u32(bytes: &[u8]) -> Vec<u32> {
    bytes
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

fn run_directive_metadata_real_gpu(
    tok_types: &[u32],
    tok_starts: &[u32],
    tok_lens: &[u32],
    source: &[u8],
) -> (Vec<u32>, Vec<u32>) {
    let n = tok_types.len() as u32;
    let prog = gpu_directive_metadata(n, source.len() as u32);
    // Source is now declared as U32 in the kernel; pad to multiple of
    // 4 bytes so the last word is fully covered.
    let padded_src_len = source.len().div_ceil(4) * 4;
    let mut src = source.to_vec();
    src.resize(padded_src_len.max(4), 0);
    let inputs = vec![
        pack_u32_le(tok_types),
        pack_u32_le(tok_starts),
        pack_u32_le(tok_lens),
        src,
        vec![0u8; (n as usize).max(1) * 4],
        vec![0u8; (n as usize).max(1) * 4],
    ];
    let backend = vyre::backend::acquire_preferred_dispatch_backend()
        .expect("preferred backend available");
    let outs = backend
        .dispatch(&prog, &inputs, &DispatchConfig::default())
        .expect("dispatch directive_metadata");
    (unpack_u32(&outs[0]), unpack_u32(&outs[1]))
}

fn run_include_parse_real_gpu(
    tok_starts: &[u32],
    tok_lens: &[u32],
    directive_kinds: &[u32],
    source: &[u8],
) -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let n = tok_starts.len() as u32;
    let prog = gpu_include_parse(n, source.len() as u32);
    let padded_src_len = source.len().div_ceil(4) * 4;
    let mut src = source.to_vec();
    src.resize(padded_src_len.max(4), 0);
    let inputs = vec![
        pack_u32_le(tok_starts),
        pack_u32_le(tok_lens),
        pack_u32_le(directive_kinds),
        src,
        vec![0u8; (n as usize).max(1) * 4],
        vec![0u8; (n as usize).max(1) * 4],
        vec![0u8; (n as usize).max(1) * 4],
    ];
    let backend = vyre::backend::acquire_preferred_dispatch_backend()
        .expect("preferred backend available");
    let outs = backend
        .dispatch(&prog, &inputs, &DispatchConfig::default())
        .expect("dispatch include_parse");
    (
        unpack_u32(&outs[0]),
        unpack_u32(&outs[1]),
        unpack_u32(&outs[2]),
    )
}

#[test]
fn directive_kind_ifdef_missing_classifies_as_pp_ifdef() {
    // `#ifdef MISSING\n`  -  the input that fails in
    // `ifdef_inactive_block_dropped`.
    let source: &[u8] = b"#ifdef MISSING\n";
    let tok_types = vec![TOK_PREPROC];
    let tok_starts = vec![0u32];
    let tok_lens = vec![source.len() as u32];

    let (kinds, _values) = run_directive_metadata_real_gpu(&tok_types, &tok_starts, &tok_lens, source);

    assert_eq!(
        kinds.first().copied(),
        Some(TOK_PP_IFDEF),
        "real-GPU gpu_directive_metadata must classify `#ifdef MISSING` as TOK_PP_IFDEF (got kinds={kinds:?})"
    );
}

#[test]
fn directive_kind_define_macro_classifies_as_pp_define() {
    // Sanity twin: `#define FOO 1\n` must classify as TOK_PP_DEFINE.
    let source: &[u8] = b"#define FOO 1\n";
    let tok_types = vec![TOK_PREPROC];
    let tok_starts = vec![0u32];
    let tok_lens = vec![source.len() as u32];

    let (kinds, _values) = run_directive_metadata_real_gpu(&tok_types, &tok_starts, &tok_lens, source);

    assert_eq!(
        kinds.first().copied(),
        Some(TOK_PP_DEFINE),
        "real-GPU gpu_directive_metadata must classify `#define FOO 1` as TOK_PP_DEFINE (got kinds={kinds:?})"
    );
}

#[test]
fn directive_kind_include_classifies_as_pp_include() {
    let source: &[u8] = b"#include \"foo.h\"\n";
    let tok_types = vec![TOK_PREPROC];
    let tok_starts = vec![0u32];
    let tok_lens = vec![source.len() as u32];

    let (kinds, _values) = run_directive_metadata_real_gpu(&tok_types, &tok_starts, &tok_lens, source);

    assert_eq!(
        kinds.first().copied(),
        Some(TOK_PP_INCLUDE),
        "real-GPU gpu_directive_metadata must classify `#include \"foo.h\"` as TOK_PP_INCLUDE (got kinds={kinds:?})"
    );
}

#[test]
fn directive_kind_negative_ordinary_token_stays_zero() {
    // Non-PREPROC token must come back as kind=0 even on real GPU.
    use vyre_libs::parsing::c::lex::tokens::TOK_IDENTIFIER;
    let source: &[u8] = b"int x";
    let tok_types = vec![TOK_IDENTIFIER];
    let tok_starts = vec![0u32];
    let tok_lens = vec![3u32];

    let (kinds, _values) = run_directive_metadata_real_gpu(&tok_types, &tok_starts, &tok_lens, source);

    assert_eq!(
        kinds.first().copied(),
        Some(0),
        "non-PREPROC token must yield kind=0 (got kinds={kinds:?})"
    );
}

#[test]
fn include_parse_quoted_local_extracts_path() {
    // `#include "foo.h"\n`  -  the input that fails in
    // `local_include_inlines_file_from_disk`.
    let source: &[u8] = b"#include \"foo.h\"\n";
    let tok_starts = vec![0u32];
    let tok_lens = vec![source.len() as u32];
    let directive_kinds = vec![TOK_PP_INCLUDE];

    let (path_starts, path_lens, is_system) =
        run_include_parse_real_gpu(&tok_starts, &tok_lens, &directive_kinds, source);

    assert_eq!(
        path_lens.first().copied(),
        Some(5),
        "path_len for `foo.h` must be 5 (got path_lens={path_lens:?}, path_starts={path_starts:?})"
    );
    let s = path_starts[0] as usize;
    let l = path_lens[0] as usize;
    assert_eq!(
        &source[s..s + l],
        b"foo.h",
        "path bytes must be `foo.h` (got bytes={:?})",
        &source[s..s + l]
    );
    assert_eq!(
        is_system.first().copied(),
        Some(0),
        "is_system must be 0 for quoted include (got {is_system:?})"
    );
}

#[test]
fn include_parse_angle_system_extracts_path_and_flags_system() {
    let source: &[u8] = b"#include <bar.h>\n";
    let tok_starts = vec![0u32];
    let tok_lens = vec![source.len() as u32];
    let directive_kinds = vec![TOK_PP_INCLUDE];

    let (path_starts, path_lens, is_system) =
        run_include_parse_real_gpu(&tok_starts, &tok_lens, &directive_kinds, source);

    assert_eq!(path_lens.first().copied(), Some(5));
    let s = path_starts[0] as usize;
    let l = path_lens[0] as usize;
    assert_eq!(&source[s..s + l], b"bar.h");
    assert_eq!(is_system.first().copied(), Some(1));
}

fn run_undef_parse_real_gpu(
    tok_starts: &[u32],
    tok_lens: &[u32],
    directive_kinds: &[u32],
    source: &[u8],
) -> (Vec<u32>, Vec<u32>) {
    let n = tok_starts.len() as u32;
    let prog = gpu_undef_parse(n, source.len() as u32);
    let padded_src_len = source.len().div_ceil(4) * 4;
    let mut src = source.to_vec();
    src.resize(padded_src_len.max(4), 0);
    let inputs = vec![
        pack_u32_le(tok_starts),
        pack_u32_le(tok_lens),
        pack_u32_le(directive_kinds),
        src,
        vec![0u8; (n as usize).max(1) * 4],
        vec![0u8; (n as usize).max(1) * 4],
    ];
    let backend = vyre::backend::acquire_preferred_dispatch_backend()
        .expect("preferred backend available");
    let outs = backend
        .dispatch(&prog, &inputs, &DispatchConfig::default())
        .expect("dispatch undef_parse");
    (unpack_u32(&outs[0]), unpack_u32(&outs[1]))
}

#[test]
fn undef_parse_extracts_simple_macro_name() {
    let source: &[u8] = b"#undef FOO\n";
    let tok_starts = vec![0u32];
    let tok_lens = vec![source.len() as u32];
    let directive_kinds = vec![TOK_PP_UNDEF];

    let (name_starts, name_lens) =
        run_undef_parse_real_gpu(&tok_starts, &tok_lens, &directive_kinds, source);

    assert_eq!(name_lens.first().copied(), Some(3));
    let s = name_starts[0] as usize;
    let l = name_lens[0] as usize;
    assert_eq!(&source[s..s + l], b"FOO");
}

#[test]
fn undef_parse_handles_long_identifier() {
    let source: &[u8] = b"#undef _MY_MACRO_42\n";
    let tok_starts = vec![0u32];
    let tok_lens = vec![source.len() as u32];
    let directive_kinds = vec![TOK_PP_UNDEF];

    let (name_starts, name_lens) =
        run_undef_parse_real_gpu(&tok_starts, &tok_lens, &directive_kinds, source);

    let s = name_starts[0] as usize;
    let l = name_lens[0] as usize;
    assert_eq!(&source[s..s + l], b"_MY_MACRO_42");
}

#[test]
fn undef_parse_zero_for_non_undef_kind() {
    let source: &[u8] = b"#define FOO 1\n";
    let tok_starts = vec![0u32];
    let tok_lens = vec![source.len() as u32];
    let directive_kinds = vec![TOK_PP_DEFINE];

    let (name_starts, name_lens) =
        run_undef_parse_real_gpu(&tok_starts, &tok_lens, &directive_kinds, source);

    assert_eq!(name_starts.first().copied(), Some(0));
    assert_eq!(name_lens.first().copied(), Some(0));
}

fn run_define_parse_real_gpu(
    tok_starts: &[u32],
    tok_lens: &[u32],
    directive_kinds: &[u32],
    source: &[u8],
) -> (Vec<u32>, Vec<u32>, Vec<u32>, Vec<u32>, Vec<u32>, Vec<u32>, Vec<u32>) {
    let n = tok_starts.len() as u32;
    let prog = gpu_define_parse(n, source.len() as u32);
    let padded_src_len = source.len().div_ceil(4) * 4;
    let mut src = source.to_vec();
    src.resize(padded_src_len.max(4), 0);
    let zero = vec![0u8; (n as usize).max(1) * 4];
    let inputs = vec![
        pack_u32_le(tok_starts),
        pack_u32_le(tok_lens),
        pack_u32_le(directive_kinds),
        src,
        zero.clone(),
        zero.clone(),
        zero.clone(),
        zero.clone(),
        zero.clone(),
        zero.clone(),
        zero,
    ];
    let backend = vyre::backend::acquire_preferred_dispatch_backend()
        .expect("preferred backend available");
    let outs = backend
        .dispatch(&prog, &inputs, &DispatchConfig::default())
        .expect("dispatch define_parse");
    (
        unpack_u32(&outs[0]),
        unpack_u32(&outs[1]),
        unpack_u32(&outs[2]),
        unpack_u32(&outs[3]),
        unpack_u32(&outs[4]),
        unpack_u32(&outs[5]),
        unpack_u32(&outs[6]),
    )
}

#[test]
fn define_parse_extracts_object_like_name_and_body() {
    // `#define FOO 1\n`  -  object-like macro, name=FOO, body=`1`.
    let source: &[u8] = b"#define FOO 1\n";
    let tok_starts = vec![0u32];
    let tok_lens = vec![source.len() as u32];
    let directive_kinds = vec![TOK_PP_DEFINE];

    let (name_s, name_l, args_s, args_l, body_s, body_l, is_func) =
        run_define_parse_real_gpu(&tok_starts, &tok_lens, &directive_kinds, source);

    let ns = name_s[0] as usize;
    let nl = name_l[0] as usize;
    assert_eq!(&source[ns..ns + nl], b"FOO", "name span must be FOO");
    assert_eq!(args_l.first().copied(), Some(0));
    assert_eq!(args_s.first().copied(), Some(0));
    let bs = body_s[0] as usize;
    let bl = body_l[0] as usize;
    assert_eq!(&source[bs..bs + bl], b"1", "body span must be `1`");
    assert_eq!(is_func.first().copied(), Some(0));
}

#[test]
fn define_parse_extracts_function_like_args_and_body() {
    // `#define MAX(a, b) ((a) > (b) ? (a) : (b))\n`
    let source: &[u8] = b"#define MAX(a,b) ((a)>(b)?(a):(b))\n";
    let tok_starts = vec![0u32];
    let tok_lens = vec![source.len() as u32];
    let directive_kinds = vec![TOK_PP_DEFINE];

    let (name_s, name_l, args_s, args_l, _body_s, _body_l, is_func) =
        run_define_parse_real_gpu(&tok_starts, &tok_lens, &directive_kinds, source);

    let ns = name_s[0] as usize;
    let nl = name_l[0] as usize;
    assert_eq!(&source[ns..ns + nl], b"MAX");
    assert_eq!(is_func.first().copied(), Some(1), "must classify as function-like");
    let asx = args_s[0] as usize;
    let al = args_l[0] as usize;
    assert_eq!(&source[asx..asx + al], b"a,b", "args span must be `a,b`");
}

fn run_ifdef_value_real_gpu(
    tok_starts: &[u32],
    tok_lens: &[u32],
    directive_kinds: &[u32],
    source: &[u8],
    macro_names: &[u8],
    macro_offsets: &[u32],
) -> Vec<u32> {
    let n = tok_starts.len() as u32;
    let prog = gpu_ifdef_value(n, source.len() as u32);

    let padded_src_len = source.len().div_ceil(4) * 4;
    let mut src = source.to_vec();
    src.resize(padded_src_len.max(4), 0);

    let padded_names_len = macro_names.len().div_ceil(4) * 4;
    let mut names = macro_names.to_vec();
    names.resize(padded_names_len.max(4), 0);

    let inputs = vec![
        pack_u32_le(tok_starts),
        pack_u32_le(tok_lens),
        pack_u32_le(directive_kinds),
        src,
        names,
        pack_u32_le(macro_offsets),
        vec![0u8; (n as usize).max(1) * 4],
    ];
    let backend = vyre::backend::acquire_preferred_dispatch_backend()
        .expect("preferred backend available");
    let outs = backend
        .dispatch(&prog, &inputs, &DispatchConfig::default())
        .expect("dispatch ifdef_value");
    unpack_u32(&outs[0])
}

#[test]
fn ifdef_value_returns_one_when_macro_is_defined() {
    let source: &[u8] = b"#ifdef FOO\n";
    let tok_starts = vec![0u32];
    let tok_lens = vec![source.len() as u32];
    let directive_kinds = vec![TOK_PP_IFDEF];
    let macro_names: &[u8] = b"FOOBAR";
    let macro_offsets = vec![0u32, 3, 6]; // {FOO, BAR}

    let values = run_ifdef_value_real_gpu(
        &tok_starts,
        &tok_lens,
        &directive_kinds,
        source,
        macro_names,
        &macro_offsets,
    );
    assert_eq!(values.first().copied(), Some(1), "FOO is defined → ifdef value 1");
}
