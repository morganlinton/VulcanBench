//! Parallelization-contract tests for the C11 GPU lexer.
//!
//! Covers the five clauses of the C lexer parallelization contract:
//!   1. source-order stability
//!   2. no atomics nondeterminism
//!   3. ellipsis max-munch
//!   4. keyword promotion for GNU builtins
//!   5. token count bounds

#![cfg(feature = "c-parser")]
#![allow(deprecated)]
mod common;
use common::words_from_bytes;

use c_grammar_gen::lex_c11_max_munch_kinds;
use vyre::DispatchConfig;
use vyre_emit_naga::program::emit_module;
use vyre_libs::parsing::c::lex::keyword::{
    c_keyword, c_keyword_map_words, reference_c_keyword_types, C_KEYWORDS,
};
use vyre_libs::parsing::c::lex::lexer::{c11_lex_digraphs, c11_lexer};
use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::pipeline::stages::C11_AST_MAX_TOK_SCAN;
use vyre_reference::value::Value;

fn bytes(words: &[u32]) -> Vec<u8> {
    vyre_primitives::wire::pack_u32_slice(words)
}

fn haystack_words(source: &[u8]) -> Vec<u32> {
    source.iter().map(|b| u32::from(*b)).collect()
}

fn emit_wgsl(program: &vyre::ir::Program) -> String {
    let module = emit_module(program, &DispatchConfig::default(), [1, 1, 1])
        .expect("Program must lower to a valid Naga module");
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .expect("Naga must accept the Program");
    naga::back::wgsl::write_string(&module, &info, naga::back::wgsl::WriterFlags::empty())
        .expect("Program must serialize to WGSL")
}

/// Run the GPU lexer `c11_lexer` through the CPU reference oracle and return
/// the compact token stream (`tok_types`, `tok_starts`, `tok_lens`) plus the
/// emitted token count.
fn run_c11_lexer(source: &[u8], haystack_len: u32) -> (Vec<u32>, Vec<u32>, Vec<u32>, u32) {
    let program = c11_lexer(
        "haystack",
        "out_tok_types",
        "out_tok_starts",
        "out_tok_lens",
        "out_counts",
        haystack_len,
    );
    let haystack_buf = bytes(&haystack_words(source));
    let zero_buf = vec![0u8; haystack_len as usize * 4];
    let count_zero = vec![0u8; 4];
    let inputs = [
        Value::from(haystack_buf),
        Value::from(zero_buf.clone()),
        Value::from(zero_buf.clone()),
        Value::from(zero_buf),
        Value::from(count_zero),
    ];
    let outputs = vyre_reference::reference_eval(&program, &inputs)
        .expect("c11_lexer must execute under the reference oracle");
    assert_eq!(
        outputs.len(),
        4,
        "expected [tok_types, tok_starts, tok_lens, counts]"
    );
    let tok_types = words_from_bytes(&outputs[0].to_bytes());
    let tok_starts = words_from_bytes(&outputs[1].to_bytes());
    let tok_lens = words_from_bytes(&outputs[2].to_bytes());
    let counts = words_from_bytes(&outputs[3].to_bytes());
    let tok_count = counts.first().copied().unwrap_or(0);
    // Trim to the actual number of emitted tokens.
    (
        tok_types[..tok_count as usize].to_vec(),
        tok_starts[..tok_count as usize].to_vec(),
        tok_lens[..tok_count as usize].to_vec(),
        tok_count,
    )
}

// ---------------------------------------------------------------------------
// 1. source-order stability
// ---------------------------------------------------------------------------

#[test]
fn lexer_emits_tokens_in_strict_source_order() {
    let source = b"int main(void) { return 42; }";
    let haystack_len = source.len() as u32;
    let (_, tok_starts, _, _) = run_c11_lexer(source, haystack_len);
    assert!(
        tok_starts.windows(2).all(|w| w[0] < w[1]),
        "tok_starts must be strictly monotonically increasing: {:?}",
        tok_starts
    );
}

#[test]
fn lexer_source_order_stable_for_whitespace_rich_source() {
    let source = b"  \t\n  int   \n\n   x   ;  \n";
    let haystack_len = source.len() as u32;
    let (_, tok_starts, _, _) = run_c11_lexer(source, haystack_len);
    assert!(
        tok_starts.windows(2).all(|w| w[0] < w[1]),
        "tok_starts must remain strictly increasing even with whitespace: {:?}",
        tok_starts
    );
}

// ---------------------------------------------------------------------------
// 2. no atomics nondeterminism
// ---------------------------------------------------------------------------

#[test]
fn c11_lexer_wgsl_contains_no_atomics() {
    let program = c11_lexer(
        "haystack",
        "out_tok_types",
        "out_tok_starts",
        "out_tok_lens",
        "out_counts",
        4096,
    );
    let wgsl = emit_wgsl(&program);
    let wgsl_lower = wgsl.to_lowercase();
    let has_atomic_op = [
        "atomicload",
        "atomicstore",
        "atomicadd",
        "atomicsub",
        "atomicmax",
        "atomicmin",
        "atomicand",
        "atomicor",
        "atomicxor",
        "atomicexchange",
        "atomiccompareexchangeweak",
    ]
    .iter()
    .any(|op| wgsl_lower.contains(op));
    assert!(
        !has_atomic_op,
        "c11_lexer WGSL must not contain any atomic operations: {wgsl}"
    );
}

#[test]
fn c11_lex_digraphs_wgsl_contains_no_atomics() {
    let program = c11_lex_digraphs("tok_types", "tok_starts", "tok_lens", 4096);
    let wgsl = emit_wgsl(&program);
    let wgsl_lower = wgsl.to_lowercase();
    let has_atomic_op = [
        "atomicload",
        "atomicstore",
        "atomicadd",
        "atomicsub",
        "atomicmax",
        "atomicmin",
        "atomicand",
        "atomicor",
        "atomicxor",
        "atomicexchange",
        "atomiccompareexchangeweak",
    ]
    .iter()
    .any(|op| wgsl_lower.contains(op));
    assert!(
        !has_atomic_op,
        "c11_lex_digraphs WGSL must not contain any atomic operations: {wgsl}"
    );
}

// ---------------------------------------------------------------------------
// 3. ellipsis max-munch
// ---------------------------------------------------------------------------

#[test]
fn ellipsis_lexes_as_single_token() {
    let source = b"...";
    let haystack_len = source.len() as u32;
    let (tok_types, _, tok_lens, tok_count) = run_c11_lexer(source, haystack_len);
    assert_eq!(tok_count, 1, "`...` must produce exactly one token");
    assert_eq!(tok_types[0], TOK_ELLIPSIS, "`...` must lex as TOK_ELLIPSIS");
    assert_eq!(tok_lens[0], 3, "TOK_ELLIPSIS must span 3 bytes");
}

#[test]
fn four_dots_max_munch_ellipsis_plus_dot() {
    let source = b"....";
    let haystack_len = source.len() as u32;
    let (tok_types, _, tok_lens, tok_count) = run_c11_lexer(source, haystack_len);
    assert_eq!(tok_count, 2, "`....` must produce exactly two tokens");
    assert_eq!(
        tok_types[0], TOK_ELLIPSIS,
        "first three dots must be TOK_ELLIPSIS"
    );
    assert_eq!(tok_lens[0], 3);
    assert_eq!(tok_types[1], TOK_DOT, "remaining dot must be TOK_DOT");
    assert_eq!(tok_lens[1], 1);
}

#[test]
fn ellipsis_host_lexer_max_munch() {
    let kinds = lex_c11_max_munch_kinds(b"...").expect("must lex");
    let non_ws: Vec<u32> = kinds
        .into_iter()
        .filter(|k| *k != TOK_WHITESPACE && *k != TOK_COMMENT)
        .collect();
    assert_eq!(non_ws, vec![TOK_ELLIPSIS]);
}

// ---------------------------------------------------------------------------
// 4. keyword promotion for GNU builtins
// ---------------------------------------------------------------------------

#[test]
fn gnu_builtin_keywords_promote_via_cpu_oracle() {
    let source = b"__builtin_constant_p __builtin_choose_expr __builtin_types_compatible_p";
    let raw_types = [TOK_IDENTIFIER, TOK_IDENTIFIER, TOK_IDENTIFIER];
    let starts = [0u32, 21, 43];
    let lens = [20u32, 21, 28];
    let promoted = reference_c_keyword_types(&raw_types, &starts, &lens, source);
    assert_eq!(
        promoted,
        vec![
            TOK_BUILTIN_CONSTANT_P,
            TOK_BUILTIN_CHOOSE_EXPR,
            TOK_BUILTIN_TYPES_COMPATIBLE_P,
        ]
    );
}

#[test]
fn gnu_builtin_keywords_promote_via_gpu_pass() {
    let source = b"__builtin_constant_p __builtin_choose_expr __builtin_types_compatible_p";
    let raw_types = vec![TOK_IDENTIFIER, TOK_IDENTIFIER, TOK_IDENTIFIER];
    let starts = vec![0u32, 21, 43];
    let lens = vec![20u32, 21, 28];
    let keyword_map = c_keyword_map_words();
    let program = c_keyword(
        "tok_types",
        "tok_starts",
        "tok_lens",
        "counts",
        "haystack",
        "keyword_map",
        raw_types.len() as u32,
        C_KEYWORDS.len() as u32,
        source.len() as u32,
    );
    let inputs = [
        Value::from(bytes(&raw_types)),
        Value::from(bytes(&starts)),
        Value::from(bytes(&lens)),
        Value::from(bytes(&[raw_types.len() as u32])),
        Value::from(bytes(&haystack_words(source))),
        Value::from(bytes(&keyword_map)),
    ];
    let outputs = vyre_reference::reference_eval(&program, &inputs)
        .expect("c_keyword must execute under the reference oracle");
    let promoted = words_from_bytes(&outputs[0].to_bytes());
    let promoted = promoted[..raw_types.len()].to_vec();
    assert_eq!(
        promoted,
        vec![
            TOK_BUILTIN_CONSTANT_P,
            TOK_BUILTIN_CHOOSE_EXPR,
            TOK_BUILTIN_TYPES_COMPATIBLE_P,
        ],
        "GPU keyword pass must promote GNU builtins identically to CPU oracle"
    );
}

// ---------------------------------------------------------------------------
// 5. token count bounds
// ---------------------------------------------------------------------------

#[test]
fn token_count_never_exceeds_haystack_len() {
    let source = b"int main(void) { return 42; }";
    let haystack_len = source.len() as u32;
    let (_, _, _, count) = run_c11_lexer(source, haystack_len);
    assert!(
        count <= haystack_len,
        "token count {count} must not exceed haystack length {haystack_len}"
    );
}

#[test]
fn empty_source_emits_zero_tokens() {
    // The GPU lexer Program rejects with_count(0), so we exercise the
    // empty-source boundary through the host reference lexer.
    let kinds = lex_c11_max_munch_kinds(b"").expect("empty source must lex");
    assert!(kinds.is_empty(), "empty source must produce zero tokens");
}

#[test]
fn whitespace_only_source_emits_zero_tokens() {
    let source = b" ";
    let (_, _, _, count) = run_c11_lexer(source, 1);
    assert_eq!(count, 0, "whitespace-only source must emit zero tokens");
}

#[test]
fn single_byte_source_emits_at_most_one_token() {
    let source = b"+";
    let (_, _, _, count) = run_c11_lexer(source, 1);
    assert!(
        count <= 1,
        "single-byte source must emit at most one token, got {count}"
    );
}

#[test]
fn token_count_bounded_by_ast_max_tok_scan() {
    let source = b"a + b ; c";
    let haystack_len = source.len() as u32;
    let (_, _, _, count) = run_c11_lexer(source, haystack_len);
    assert!(
        count <= C11_AST_MAX_TOK_SCAN,
        "token count {count} must not exceed C11_AST_MAX_TOK_SCAN ({C11_AST_MAX_TOK_SCAN})"
    );
    assert!(
        count <= haystack_len,
        "token count {count} must also not exceed haystack length {haystack_len}"
    );
}

#[test]
fn dense_token_stream_count_bound() {
    // Every byte is a distinct single-character token.
    let source = b"+-*/(){}[];,.";
    let haystack_len = source.len() as u32;
    let (_, _, _, count) = run_c11_lexer(source, haystack_len);
    assert_eq!(
        count, haystack_len,
        "dense punctuation source must emit one token per byte"
    );
    assert!(
        count <= C11_AST_MAX_TOK_SCAN,
        "dense token count {count} must respect C11_AST_MAX_TOK_SCAN"
    );
}
